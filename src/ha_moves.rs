//! Durable copy/verify/delete moves. Recovery never repeats an HA mutation.
use super::*;

impl HaStore {
  pub(super) fn ensure_not_pending(&self, list: &str) -> Result<(), AppError> {
    if self.metadata.pending.as_ref().is_some_and(|p| {
      p.target == list || split_key(&p.source).is_ok_and(|(source, _)| source == list)
    }) {
      return Err(AppError::Conflict(
        "A move involving this list needs review. Reconcile or abandon it before another write."
          .into(),
      ));
    }
    Ok(())
  }

  pub(super) fn pending_move(&self) -> Value {
    self.metadata.pending.as_ref().map(|p| json!({"operation_id":p.operation_id,"task_id":p.metadata.id,"source_list":split_key(&p.source).map(|p| p.0).unwrap_or(""),"target_list":p.target,"copy_identified":p.target_uid.is_some()})).unwrap_or(Value::Null)
  }

  pub(super) async fn move_task(
    &mut self,
    id: u64,
    text: &str,
    target: &str,
    before: BTreeMap<String, Item>,
  ) -> Result<Entry, AppError> {
    if self.metadata.pending.is_some() {
      return Err(invalid("Resolve the pending HA move first"));
    }
    let (source, metadata) = self
      .metadata
      .items
      .iter()
      .find(|(_, m)| m.id == id)
      .map(|(k, m)| (k.clone(), m.clone()))
      .ok_or(AppError::NotFound)?;
    let original = before.get(&source).cloned().ok_or(AppError::NotFound)?;
    let task = parse_text(text)?;
    let (summary, due) = summary_due(&task)?;
    let probe = Task {
      completed: false,
      legacy_prefix: None,
      completion_date: None,
      creation_date: None,
      description: summary.clone(),
    };
    if Task::parse(&probe.to_string()) != Some(probe) {
      return Err(invalid("Invalid HA move summary"));
    }
    self.entity = target.to_owned();
    let features = self.features().await?;
    if features & WRITE_FEATURES != WRITE_FEATURES
      || original.description.is_some() && features & 64 == 0
      || due
        .as_ref()
        .is_some_and(|d| features & if date(d) { 16 } else { 32 } == 0)
    {
      return Err(invalid(
        "Target HA list cannot preserve all moved task fields",
      ));
    }
    self.entity = split_key(&source)?.0.to_owned();
    if self.features().await? & 2 == 0 {
      return Err(invalid("Source HA list cannot delete tasks"));
    }
    let expected = Item {
      uid: String::new(),
      summary: summary.clone(),
      status: if task.completed {
        "completed"
      } else {
        "needs_action"
      }
      .into(),
      due: due.clone(),
      description: original.description.clone(),
    };
    let mut saved_metadata = metadata;
    if task.creation_date.is_some() {
      saved_metadata.creation = task.creation_date.map(|d| d.to_string());
    }
    saved_metadata.completion = task.completion_date.map(|d| d.to_string());
    saved_metadata.legacy = task.legacy_prefix;
    saved_metadata.completed = task.completed;
    let operation_id = self
      .metadata
      .move_sequence
      .checked_add(1)
      .filter(|id| *id <= MAX_ID)
      .ok_or_else(metadata_error)?;
    let journal = MoveJournal {
      operation_id,
      source: source.clone(),
      target: target.into(),
      target_uid: None,
      before_uids: before.keys().cloned().collect(),
      original,
      expected,
      metadata: saved_metadata,
    };
    let mut next = self.metadata.clone();
    next.move_sequence = operation_id;
    next.pending = Some(journal.clone());
    self.save(next)?;
    self.entity = target.to_owned();
    let result = self.copy_verify_delete(journal).await;
    if result.is_err() {
      self.interrupted();
    }
    result
  }

  async fn copy_verify_delete(&mut self, mut journal: MoveJournal) -> Result<Entry, AppError> {
    let mut data = json!({"item":journal.expected.summary});
    if let Some(description) = &journal.expected.description {
      data["description"] = json!(description);
    }
    if let Some(due) = &journal.expected.due {
      data[if date(due) {
        "due_date"
      } else {
        "due_datetime"
      }] = json!(due);
    }
    self
      .service("add_item", data)
      .await
      .map_err(|_| uncertain())?;
    let after = self.fetch().await?;
    let candidates: Vec<_> = after
      .values()
      .filter(|item| {
        split_key(&item.uid).is_ok_and(|(list, _)| list == journal.target)
          && !journal.before_uids.contains(&item.uid)
          && item.summary == journal.expected.summary
          && same_due(&item.due, &journal.expected.due)
          && item.description == journal.expected.description
          && item.status == "needs_action"
      })
      .collect();
    if candidates.len() != 1 {
      return Err(uncertain());
    }
    let key = candidates[0].uid.clone();
    journal.target_uid = Some(key.clone());
    let mut next = self.metadata.clone();
    next.pending = Some(journal.clone());
    self.save(next)?;
    // Record the UID before any follow-up write, including completion status.
    if journal.expected.status == "completed" {
      self.guard_uid(&after, &key)?;
      self
        .service("update_item", json!({"item": key, "status":"completed"}))
        .await
        .map_err(|_| uncertain())?;
    }
    let verified = self.fetch().await?;
    if verified.get(&journal.source) != Some(&journal.original)
      || !verified
        .get(&key)
        .is_some_and(|item| moved_content_matches(item, &journal.expected))
    {
      return Err(uncertain());
    }
    self.entity = split_key(&journal.source)?.0.to_owned();
    self.guard_uid(&verified, &journal.source)?;
    self
      .service("remove_item", json!({"item":journal.source}))
      .await
      .map_err(|_| uncertain())?;
    let after = self.fetch().await?;
    if after.contains_key(&journal.source)
      || !after
        .get(&key)
        .is_some_and(|item| moved_content_matches(item, &journal.expected))
    {
      return Err(uncertain());
    }
    self.finish_move(&journal, &after)?;
    self.publish(after.clone(), None);
    self
      .entries(&after)
      .into_iter()
      .find(|e| e.id == journal.metadata.id)
      .ok_or_else(uncertain)
  }

  fn guard_uid(&self, snapshot: &BTreeMap<String, Item>, key: &str) -> Result<(), AppError> {
    let (list, uid) = split_key(key)?;
    if snapshot.values().any(|i| {
      i.uid != key && split_key(&i.uid).is_ok_and(|(other, _)| other == list) && i.summary == uid
    }) {
      return Err(invalid(
        "HA UID collides with another summary; resolve in HA first",
      ));
    }
    Ok(())
  }

  fn finish_move(
    &mut self,
    journal: &MoveJournal,
    snapshot: &BTreeMap<String, Item>,
  ) -> Result<(), AppError> {
    self.observe(snapshot)?;
    let mut next = self.metadata.clone();
    next.items.remove(&journal.source);
    next.items.insert(
      journal.target_uid.clone().ok_or_else(uncertain)?,
      journal.metadata.clone(),
    );
    next.pending = None;
    self.save(next)
  }

  pub async fn recover_move(&mut self, abandon: bool, confirmed: bool) -> Result<Value, AppError> {
    if !confirmed {
      return Err(invalid("Explicit move recovery confirmation is required"));
    }
    let journal = self.metadata.pending.clone().ok_or(AppError::NotFound)?;
    let snapshot = self.fetch().await?;
    if abandon {
      // No HA writes: remaining copies receive separate identities via observe.
      self.observe(&snapshot)?;
      let mut next = self.metadata.clone();
      next.pending = None;
      self.save(next)?;
    } else {
      let key = journal.target_uid.as_ref().ok_or_else(|| AppError::Conflict("Copy UID is uncertain. Review both lists in HA, then abandon this move; no automatic replay is allowed.".into()))?;
      if snapshot.contains_key(&journal.source)
        || !snapshot
          .get(key)
          .is_some_and(|i| moved_content_matches(i, &journal.expected))
      {
        return Err(AppError::Conflict("Move is not proven complete. Review both lists, then abandon without deleting either task.".into()));
      }
      self.finish_move(&journal, &snapshot)?;
    }
    self.publish(snapshot, None);
    self.notify(None);
    self.lists().await
  }

  pub fn check_recovery(&self, expected: &str) -> Result<(), AppError> {
    if expected.parse::<u64>().ok().is_none_or(|id| {
      self
        .metadata
        .pending
        .as_ref()
        .is_none_or(|p| p.operation_id != id)
    }) {
      return Err(AppError::Conflict(
        "Pending move changed. Refresh and confirm the current operation.".into(),
      ));
    }
    Ok(())
  }
}

fn moved_content_matches(a: &Item, b: &Item) -> bool {
  a.summary == b.summary
    && a.status == b.status
    && a.description == b.description
    && same_due(&a.due, &b.due)
}

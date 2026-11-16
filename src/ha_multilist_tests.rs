use super::*;

// Each writer fails only the selected journal transition, after replacement but
// before directory synchronization. Earlier observations still use real syncs.
#[cfg(unix)]
fn fail_journal_directory_sync<const PHASE: u8>(
  path: &std::path::Path,
  text: &str,
) -> std::io::Result<()> {
  let next: Sidecar = serde_json::from_str(text).unwrap();
  let fail = match PHASE {
    0 => next
      .pending
      .as_ref()
      .is_some_and(|p| p.target_uid.is_none()),
    1 => next
      .pending
      .as_ref()
      .is_some_and(|p| p.target_uid.is_some()),
    2 => next.pending.is_none() && next.move_sequence > 0,
    _ => unreachable!(),
  };
  atomicfile::write_durable_with(path, text, |stage| {
    if fail && stage == atomicfile::DurableStage::SyncDirectory {
      Err(std::io::Error::other(
        "injected journal directory sync failure",
      ))
    } else {
      Ok(())
    }
  })
}

#[cfg(unix)]
#[tokio::test]
async fn failed_intent_directory_sync_never_admits_copy_and_restart_can_abandon() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  f.set(json!({"writes":[]})).await;
  store.persist_metadata = fail_journal_directory_sync::<0>;
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      move_route(&entry, "todo.target"),
      false,
      None
    )
    .await
    .is_err());
  assert!(f.state().await["writes"].as_array().unwrap().is_empty());
  let (mut restarted, _) = f.store();
  assert!(
    restarted.metadata.pending.is_some(),
    "post-rename intent may already exist on disk"
  );
  restarted.recover_move(true, true).await.unwrap();
  assert_eq!(restarted.list().await.unwrap()[0].id, entry.id);
  assert!(f.state().await["writes"].as_array().unwrap().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn failed_target_directory_sync_retains_source_and_distinct_copies_without_replay() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  f.set(json!({"writes":[]})).await;
  store.persist_metadata = fail_journal_directory_sync::<1>;
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      move_route(&entry, "todo.target"),
      false,
      None
    )
    .await
    .is_err());
  let state = f.state().await;
  assert_eq!(
    state["writes"].as_array().unwrap().len(),
    1,
    "copy only; source deletion is not admitted"
  );
  assert_eq!(state["items"].as_array().unwrap().len(), 1);
  assert!(
    store
      .metadata
      .pending
      .as_ref()
      .unwrap()
      .target_uid
      .is_none(),
    "memory is not advanced on failure"
  );
  let (mut restarted, _) = f.store();
  assert!(restarted
    .metadata
    .pending
    .as_ref()
    .unwrap()
    .target_uid
    .is_some());
  assert!(restarted
    .mutate(Mutation::Delete(entry.id), false, None)
    .await
    .is_err());
  assert!(restarted.recover_move(false, true).await.is_err());
  restarted.recover_move(true, true).await.unwrap();
  let entries = restarted.list().await.unwrap();
  assert_eq!(entries.len(), 2);
  assert_ne!(entries[0].id, entries[1].id);
  assert!(entries.iter().any(|e| e.id == entry.id));
  assert_eq!(f.state().await["writes"], state["writes"]);
}

#[cfg(unix)]
#[tokio::test]
async fn failed_completion_directory_sync_keeps_memory_blocked_and_restart_keeps_identity() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  store.persist_metadata = fail_journal_directory_sync::<2>;
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      move_route(&entry, "todo.target"),
      false,
      None
    )
    .await
    .is_err());
  let state = f.state().await;
  assert!(state["items"].as_array().unwrap().is_empty());
  assert!(
    store.metadata.pending.is_some(),
    "failed completion keeps conservative in-memory admission"
  );
  assert!(store.ensure_not_pending("todo.target").is_err());
  let (mut restarted, _) = f.store();
  assert!(
    restarted.metadata.pending.is_none(),
    "post-rename disk can already contain completion"
  );
  let moved = restarted.list().await.unwrap().remove(0);
  assert_eq!(moved.id, entry.id);
  assert_eq!(moved.list_id.as_deref(), Some("todo.target"));
  assert_eq!(f.state().await["writes"], state["writes"]);
}

#[cfg(unix)]
#[tokio::test]
async fn recovery_directory_sync_failures_never_replay_and_preserve_identity_after_restart() {
  for abandon in [true, false] {
    let f = Fixture::new();
    f.set(json!({"lists":{"todo.target":[]}})).await;
    let (mut store, _) = f.store();
    let entry = store
      .mutate(Mutation::Create("task".into()), false, None)
      .await
      .unwrap()
      .unwrap();
    f.set(json!({"mode": if abandon { "move_fail_delete" } else { "move_delete_then_fail" }}))
      .await;
    assert!(store
      .mutate_routed(
        Mutation::Update(entry.id, entry.task.to_string()),
        move_route(&entry, "todo.target"),
        false,
        None
      )
      .await
      .is_err());
    f.set(json!({"mode":""})).await;
    let writes = f.state().await["writes"].clone();
    store.persist_metadata = fail_journal_directory_sync::<2>;
    assert!(store.recover_move(abandon, true).await.is_err());
    assert!(store.metadata.pending.is_some());
    let (mut restarted, _) = f.store();
    assert!(restarted.metadata.pending.is_none());
    let entries = restarted.list().await.unwrap();
    let ids: BTreeSet<_> = entries.iter().map(|entry| entry.id).collect();
    assert_eq!(ids.len(), entries.len());
    assert!(entries.iter().any(|e| e.id == entry.id));
    assert_eq!(entries.len(), if abandon { 2 } else { 1 });
    assert_eq!(f.state().await["writes"], writes);
  }
}

#[tokio::test]
async fn canceled_copy_keeps_durable_intent_and_never_deletes_source_on_restart() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  f.set(json!({"mode":"pause_write", "writes":[]})).await;
  let handle = tokio::spawn(async move {
    store
      .mutate_routed(
        Mutation::Update(entry.id, entry.task.to_string()),
        move_route(&entry, "todo.target"),
        false,
        None,
      )
      .await
  });
  let started: Value = f
    .client
    .get(format!("{}/barrier/wait", f.config.ha_url))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  assert_eq!(started["started"], true);
  handle.abort();
  assert!(handle.await.unwrap_err().is_cancelled());
  f.client
    .get(format!("{}/barrier/release", f.config.ha_url))
    .send()
    .await
    .unwrap();
  f.set(json!({"mode":""})).await;
  let writes = f.state().await["writes"].clone();
  let (mut restarted, _) = f.store();
  assert!(restarted.metadata.pending.is_some());
  assert!(restarted.recover_move(false, true).await.is_err());
  assert_eq!(f.state().await["items"].as_array().unwrap().len(), 1);
  assert_eq!(f.state().await["writes"], writes);
}

#[tokio::test]
async fn metadata_failure_after_source_deletion_can_be_reconciled_without_replaying_delete() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  f.set(json!({"mode":"move_delete_block_metadata", "writes":[]}))
    .await;
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      move_route(&entry, "todo.target"),
      false,
      None
    )
    .await
    .is_err());
  let writes = f.state().await["writes"].clone();
  std::fs::remove_dir(f.dir.join("metadata.json.tmp")).unwrap();
  f.set(json!({"mode":""})).await;
  let (mut restarted, _) = f.store();
  restarted.recover_move(false, true).await.unwrap();
  assert_eq!(restarted.list().await.unwrap()[0].id, entry.id);
  assert_eq!(f.state().await["writes"], writes);
}

fn route(list: &str) -> Routing {
  Routing {
    list_id: Some(list.into()),
    ..Routing::default()
  }
}
fn move_route(entry: &Entry, target: &str) -> Routing {
  Routing {
    list_id: Some(target.into()),
    confirm_move: true,
    expected_source: entry.list_id.clone(),
    expected_raw: Some(entry.task.to_string()),
  }
}

#[tokio::test]
async fn lists_qualify_duplicate_uids_and_preserve_global_ids_across_restart() {
  let f = Fixture::new();
  f.set(json!({"items":[item("same", "legacy +work")], "lists":{"todo.other":[item("same", "legacy +work")], "todo.empty":[]}, "names":{"todo.fixture":"Personal tasks", "todo.other":"Personal tasks"}})).await;
  let (mut store, _) = f.store();
  let entries = store.list().await.unwrap();
  assert_eq!(entries.len(), 2);
  assert_ne!(
    entries[0].id, entries[1].id,
    "same UID in distinct lists must not alias"
  );
  let catalog = store.lists().await.unwrap();
  assert_eq!(
    catalog["lists"].as_array().unwrap().len(),
    3,
    "empty lists are lists too"
  );
  assert_eq!(
    catalog["can_create"], false,
    "administration must be opt-in"
  );
  let created = store
    .mutate_routed(
      Mutation::Create("new +literal".into()),
      route("todo.other"),
      false,
      None,
    )
    .await
    .unwrap()
    .unwrap();
  assert_eq!(created.list_id.as_deref(), Some("todo.other"));
  assert_eq!(created.task.description, "new +literal");
  let updated = store
    .mutate(
      Mutation::Update(created.id, "changed +literal".into()),
      false,
      None,
    )
    .await
    .unwrap()
    .unwrap();
  assert_eq!(
    updated.list_id, created.list_id,
    "old clients cannot move tasks through summary edits"
  );
  let (mut restarted, _) = f.store();
  let after = restarted.list().await.unwrap();
  assert!(after
    .iter()
    .any(|e| e.id == created.id && e.list_id.as_deref() == Some("todo.other")));
}

#[tokio::test]
async fn schema_one_migrates_atomically_without_changing_existing_ids() {
  let f = Fixture::new();
  f.set(
    json!({"items":[item("old", "old item")], "lists":{"todo.other":[item("old", "other item")]}}),
  )
  .await;
  std::fs::write(&f.config.ha_metadata_file, json!({"schema":1,"source":format!("{}/|todo.fixture",f.config.ha_url),"next_id":43,"items":{"old":{"id":42,"creation":"2026-01-01","completion":null,"legacy":"A","completed":false}}}).to_string()).unwrap();
  let (mut store, _) = f.store();
  let entries = store.list().await.unwrap();
  let old = entries
    .iter()
    .find(|e| e.list_id.as_deref() == Some("todo.fixture"))
    .unwrap();
  assert_eq!(old.id, 42);
  assert_eq!(old.task.creation_date.unwrap().to_string(), "2026-01-01");
  assert_eq!(
    entries
      .iter()
      .find(|e| e.list_id.as_deref() == Some("todo.other"))
      .unwrap()
      .id,
    43
  );
  let map: Value =
    serde_json::from_str(&std::fs::read_to_string(&f.config.ha_metadata_file).unwrap()).unwrap();
  assert_eq!(map["schema"], 2);
  let mut wrong = f.config.clone();
  wrong.ha_url = "https://elsewhere.test".into();
  assert!(HaStore::new(&wrong, broadcast::channel(8).0).is_err());
}

#[tokio::test]
async fn migration_persistence_failure_preserves_schema_one_and_never_writes_to_ha() {
  let f = Fixture::new();
  let original = json!({"schema":1,"source":format!("{}/|todo.fixture",f.config.ha_url),
    "next_id":2,"items":{"old":{"id":1,"creation":null,"completion":null,"legacy":null,"completed":false}}}).to_string();
  std::fs::write(&f.config.ha_metadata_file, &original).unwrap();
  std::fs::create_dir(f.dir.join("metadata.json.tmp")).unwrap();
  assert!(HaStore::new(&f.config, broadcast::channel(8).0).is_err());
  assert_eq!(
    std::fs::read_to_string(&f.config.ha_metadata_file).unwrap(),
    original
  );
  assert!(f.state().await["writes"].as_array().unwrap().is_empty());
  std::fs::remove_dir(f.dir.join("metadata.json.tmp")).unwrap();
  let (store, _) = f.store();
  assert_eq!(store.metadata.schema, 2);
  assert_eq!(store.metadata.items["todo.fixture\nold"].id, 1);
}

#[tokio::test]
async fn administrative_creation_rename_and_empty_only_deletion_use_verified_wss() {
  let mut f = Fixture::new();
  f.config.ha_manage_lists = true;
  let (mut store, mut events) = f.store();
  assert_eq!(store.lists().await.unwrap()["can_create"], true);
  store.poll().await.unwrap();
  while events.try_recv().is_ok() {}
  let result = store
    .manage_list("create", "", "Shared tasks", true, "")
    .await
    .unwrap();
  let list = result["lists"]
    .as_array()
    .unwrap()
    .iter()
    .find(|l| l["name"] == "Shared tasks")
    .unwrap();
  let id = list["id"].as_str().unwrap();
  assert_eq!(list["can_delete"], true);
  assert!(
    events.try_recv().is_ok(),
    "empty catalog changes must notify"
  );
  store
    .manage_list("rename", id, "New display name", true, "Shared tasks")
    .await
    .unwrap();
  assert!(store
    .manage_list("rename", id, "Stale name", true, "Shared tasks")
    .await
    .is_err());
  assert!(store
    .manage_list("delete", "todo.fixture", "", true, "todo.fixture")
    .await
    .is_err());
  f.set(json!({"lists":{id:[{"uid":"done", "summary":"completed task", "status":"completed"}]}}))
    .await;
  assert!(
    store
      .manage_list("delete", id, "", true, "New display name")
      .await
      .is_err(),
    "completed tasks prevent deletion"
  );
  f.set(json!({"lists":{id:[]}})).await;
  store
    .manage_list("delete", id, "", true, "New display name")
    .await
    .unwrap();
  assert!(f.state().await["lists"].get(id).is_none());
}

#[tokio::test]
async fn administration_permissions_provider_and_tls_guards_do_not_disable_task_reads() {
  let mut f = Fixture::new();
  f.set(json!({"lists":{"todo.remote":[]}, "platforms":{"todo.remote":"other_provider"}}))
    .await;
  let (mut store, _) = f.store();
  assert!(store
    .manage_list("create", "", "Forbidden", true, "")
    .await
    .is_err());
  f.config.ha_manage_lists = true;
  let (mut store, _) = f.store();
  let catalog = store.lists().await.unwrap();
  assert_eq!(
    catalog["lists"]
      .as_array()
      .unwrap()
      .iter()
      .find(|l| l["id"] == "todo.remote")
      .unwrap()["can_delete"],
    false
  );
  assert!(store
    .manage_list("delete", "todo.remote", "", true, "todo.remote")
    .await
    .is_err());
  f.set(json!({"admin":false})).await;
  assert_eq!(store.lists().await.unwrap()["can_create"], false);
  assert!(store.list().await.is_ok());
  let mut bad = f.config.clone();
  bad.ha_ca_file = None;
  let mut untrusted = HaStore::new(&bad, broadcast::channel(8).0).unwrap();
  assert!(untrusted.lists().await.is_err());
}

#[tokio::test]
async fn confirmed_move_preserves_description_status_due_dates_and_numeric_identity() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(
      Mutation::Create("x 2026-01-02 2026-01-01 task +legacy due:2026-11-16T23:00:00-05:00".into()),
      false,
      None,
    )
    .await
    .unwrap()
    .unwrap();
  let mut state = f.state().await;
  state["items"][0]["description"] = json!("**Native description**\nwith lines");
  f.set(json!({"items":state["items"]})).await;
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      route("todo.target"),
      false,
      None
    )
    .await
    .is_err());
  let moved = store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      move_route(&entry, "todo.target"),
      false,
      None,
    )
    .await
    .unwrap()
    .unwrap();
  assert_eq!(moved.id, entry.id);
  assert_eq!(moved.list_id.as_deref(), Some("todo.target"));
  assert_eq!(moved.task, entry.task);
  let state = f.state().await;
  assert!(state["items"].as_array().unwrap().is_empty());
  assert_eq!(
    state["lists"]["todo.target"][0]["description"],
    "**Native description**\nwith lines"
  );
  assert_eq!(state["lists"]["todo.target"][0]["status"], "completed");
  assert_eq!(
    state["lists"]["todo.target"][0]["due"],
    "2026-11-16T23:00:00-05:00"
  );
  assert!(store.metadata.pending.is_none());
  let (mut restarted, _) = f.store();
  assert_eq!(restarted.list().await.unwrap()[0].id, entry.id);
}

#[tokio::test]
async fn failed_move_stages_never_replay_or_delete_unverified_source_and_can_be_abandoned() {
  for mode in [
    "ambiguous_create",
    "write_then_fail",
    "move_fail_status",
    "move_status_then_fail",
    "move_fail_delete",
    "move_change_source",
    "block_metadata",
  ] {
    let f = Fixture::new();
    f.set(json!({"lists":{"todo.target":[]}})).await;
    let (mut store, _) = f.store();
    let entry = store
      .mutate(Mutation::Create("x 2026-01-02 task".into()), false, None)
      .await
      .unwrap()
      .unwrap();
    f.set(json!({"mode":mode,"writes":[]})).await;
    assert!(
      store
        .mutate_routed(
          Mutation::Update(entry.id, entry.task.to_string()),
          move_route(&entry, "todo.target"),
          false,
          None
        )
        .await
        .is_err(),
      "{mode}"
    );
    let before = f.state().await;
    assert_eq!(
      before["items"].as_array().unwrap().len(),
      1,
      "source retained for {mode}"
    );
    f.set(json!({"mode":""})).await;
    if mode == "block_metadata" {
      std::fs::remove_dir(f.dir.join("metadata.json.tmp")).unwrap();
    }
    let (mut restarted, _) = f.store();
    assert!(restarted.metadata.pending.is_some());
    assert!(
      restarted.check_recovery("0").is_err(),
      "stale recovery must not release a different journal"
    );
    let operation_id = restarted.metadata.pending.as_ref().unwrap().operation_id;
    assert!(restarted.check_recovery(&operation_id.to_string()).is_ok());
    assert!(restarted
      .mutate(Mutation::Delete(entry.id), false, None)
      .await
      .is_err());
    assert!(restarted.recover_move(false, true).await.is_err());
    assert_eq!(
      f.state().await["writes"],
      before["writes"],
      "recovery must not replay HA writes for {mode}"
    );
    restarted.recover_move(true, true).await.unwrap();
    let entries = restarted.list().await.unwrap();
    let ids: BTreeSet<_> = entries.iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), entries.len());
    assert!(entries.iter().any(|e| e.id == entry.id));
  }
}

#[tokio::test]
async fn lost_source_delete_response_reconciles_identity_without_repeating_writes() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  f.set(json!({"mode":"move_delete_then_fail", "writes":[]}))
    .await;
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      move_route(&entry, "todo.target"),
      false,
      None
    )
    .await
    .is_err());
  f.set(json!({"mode":""})).await;
  let writes = f.state().await["writes"].clone();
  let (mut restarted, _) = f.store();
  restarted.recover_move(false, true).await.unwrap();
  let entries = restarted.list().await.unwrap();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].id, entry.id);
  assert_eq!(f.state().await["writes"], writes);
}

#[tokio::test]
async fn move_journal_save_failure_happens_before_copy_and_stale_confirmation_has_no_writes() {
  let f = Fixture::new();
  f.set(json!({"lists":{"todo.target":[]}})).await;
  let (mut store, _) = f.store();
  let entry = store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  let mut routing = move_route(&entry, "todo.target");
  routing.expected_raw = Some("stale".into());
  f.set(json!({"writes":[]})).await;
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      routing,
      false,
      None
    )
    .await
    .is_err());
  std::fs::create_dir(f.dir.join("metadata.json.tmp")).unwrap();
  assert!(store
    .mutate_routed(
      Mutation::Update(entry.id, entry.task.to_string()),
      move_route(&entry, "todo.target"),
      false,
      None
    )
    .await
    .is_err());
  assert!(f.state().await["writes"].as_array().unwrap().is_empty());
}

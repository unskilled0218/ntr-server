//! Catalog and administrative list management. No task summary parsing here.
use super::*;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
  connect_async_tls_with_config,
  tungstenite::{protocol::WebSocketConfig, Message},
  Connector,
};

impl HaStore {
  pub(super) async fn discover(&self) -> Result<Vec<HaList>, AppError> {
    let states = self.request("api/states", None).await?;
    let states = states.as_array().ok_or_else(upstream)?;
    let mut lists = Vec::new();
    for state in states {
      let Some(id) = state["entity_id"].as_str().filter(|id| valid_entity(id)) else {
        continue;
      };
      if state["state"] == "unavailable" || state["state"] == "unknown" {
        return Err(AppError::Unavailable(
          "A visible HA list is unavailable; task snapshot was not replaced".into(),
        ));
      }
      if state["attributes"]["supported_features"].as_u64().is_none() {
        return Err(upstream());
      }
      lists.push(HaList {
        id: id.to_owned(),
        name: state["attributes"]["friendly_name"]
          .as_str()
          .unwrap_or(id)
          .to_owned(),
        is_default: id == self.default_entity,
        can_rename: false,
        can_delete: false,
      });
    }
    lists.sort_by(|a, b| a.id.cmp(&b.id));
    if !lists.iter().any(|l| l.is_default) || lists.windows(2).any(|l| l[0].id == l[1].id) {
      return Err(upstream());
    }
    Ok(lists)
  }

  // Each command uses a bounded, short-lived verified WSS connection. The token
  // never enters URLs, error messages, or downstream API payloads.
  async fn ws(&self, command: Value) -> Result<Value, AppError> {
    tokio::time::timeout(Duration::from_secs(10), async {
      let mut url = self.base.join("api/websocket").map_err(|_| upstream())?;
      url.set_scheme("wss").map_err(|_| upstream())?;
      let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_RESPONSE))
        .max_frame_size(Some(MAX_RESPONSE));
      let (mut socket, _) = connect_async_tls_with_config(
        url.as_str(),
        Some(config),
        false,
        Some(Connector::Rustls(self.tls.clone())),
      )
      .await
      .map_err(|_| {
        AppError::Unavailable("HA WSS connection failed; check CA trust and availability".into())
      })?;
      let result = async {
        async fn receive<S>(
          socket: &mut tokio_tungstenite::WebSocketStream<S>,
        ) -> Result<Value, AppError>
        where
          S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
        {
          match socket.next().await {
            Some(Ok(Message::Text(text))) => serde_json::from_str(&text).map_err(|_| upstream()),
            _ => Err(upstream()),
          }
        }
        if receive(&mut socket).await?["type"] != "auth_required" {
          return Err(upstream());
        }
        socket
          .send(Message::Text(
            json!({"type": "auth", "access_token": self.token})
              .to_string()
              .into(),
          ))
          .await
          .map_err(|_| upstream())?;
        if receive(&mut socket).await?["type"] != "auth_ok" {
          return Err(invalid("HA rejected WSS authentication"));
        }
        let mut command = command;
        command["id"] = json!(1);
        socket
          .send(Message::Text(command.to_string().into()))
          .await
          .map_err(|_| upstream())?;
        let reply = receive(&mut socket).await?;
        if reply["id"] != 1 || reply["type"] != "result" || reply["success"] != true {
          return Err(invalid(
            "HA rejected list administration; administrator permissions may be required",
          ));
        }
        Ok(reply["result"].clone())
      }
      .await;
      let _ = socket.close(None).await;
      result
    })
    .await
    .map_err(|_| AppError::Timeout)?
  }

  async fn administration(&self) -> Result<bool, AppError> {
    if !self.manage_lists {
      return Ok(false);
    }
    Ok(self.ws(json!({"type":"auth/current_user"})).await?["is_admin"] == true)
  }

  async fn registry(&self) -> Result<Vec<Value>, AppError> {
    serde_json::from_value(
      self
        .ws(json!({"type":"config/entity_registry/list"}))
        .await?,
    )
    .map_err(|_| upstream())
  }

  async fn local_entry(&self, id: &str) -> Result<String, AppError> {
    let registry = self.registry().await?;
    Self::local_entry_from(&registry, id)
  }

  fn local_entry_from(registry: &[Value], id: &str) -> Result<String, AppError> {
    let entity = registry
      .iter()
      .find(|e| e["entity_id"] == id)
      .ok_or_else(upstream)?;
    if entity["platform"] != "local_todo" {
      return Err(invalid("Only HA Local To-do lists can be deleted in ntr"));
    }
    let entry = entity["config_entry_id"].as_str().ok_or_else(upstream)?;
    if registry
      .iter()
      .filter(|e| e["config_entry_id"] == entry)
      .count()
      != 1
    {
      return Err(invalid(
        "Config entry owns multiple entities; deletion blocked",
      ));
    }
    Ok(entry.to_owned())
  }

  pub async fn lists(&mut self) -> Result<Value, AppError> {
    let mut lists = self.discover().await?;
    let admin = self.administration().await?;
    if admin {
      let registry = self.registry().await?;
      for list in &mut lists {
        list.can_rename = registry.iter().any(|entity| entity["entity_id"] == list.id);
        list.can_delete = !list.is_default && Self::local_entry_from(&registry, &list.id).is_ok();
      }
    }
    Ok(json!({"lists": lists, "can_create": admin, "pending_move": self.pending_move()}))
  }

  pub async fn manage_list(
    &mut self,
    action: &str,
    id: &str,
    name: &str,
    confirmed: bool,
    expected_name: &str,
  ) -> Result<Value, AppError> {
    if !confirmed {
      return Err(invalid("Explicit list mutation confirmation is required"));
    }
    if !self.administration().await? {
      return Err(invalid(
        "Enable HA_MANAGE_LISTS and use an HA administrator token for list management",
      ));
    }
    let catalog = self.discover().await?;
    if action != "create" {
      let list = catalog
        .iter()
        .find(|l| l.id == id)
        .ok_or(AppError::NotFound)?;
      if list.name != expected_name {
        return Err(AppError::Conflict(
          "HA list changed; reopen confirmation".into(),
        ));
      }
      self.ensure_not_pending(id)?;
    }
    if action != "delete"
      && (name.trim().is_empty()
        || name.len() > 200
        || name.contains(['\n', '\r'])
        || catalog.iter().any(|l| l.id != id && l.name == name))
    {
      return Err(invalid(
        "Use a unique nonempty list name of at most 200 bytes on one line",
      ));
    }
    let result = match action {
      "create" => self.create_list(name).await,
      "rename" if confirmed => {
        self
          .ws(json!({"type":"config/entity_registry/update", "entity_id": id, "name": name}))
          .await?;
        let after = self.discover().await?;
        if !after.iter().any(|l| l.id == id && l.name == name) {
          return Err(uncertain());
        }
        Ok(())
      }
      "delete" if confirmed => {
        if id == self.default_entity {
          return Err(invalid("The default HA list cannot be deleted"));
        }
        let entry = self.local_entry(id).await?;
        let snapshot = self.fetch().await?;
        if snapshot
          .keys()
          .any(|key| split_key(key).is_ok_and(|(list, _)| list == id))
        {
          return Err(invalid(
            "Only empty lists can be deleted, including completed tasks",
          ));
        }
        // HA has no atomic empty-check-and-delete. Exclusive external access is required.
        self.delete_entry(&entry).await?;
        if self.discover().await?.iter().any(|l| l.id == id) {
          return Err(uncertain());
        }
        Ok(())
      }
      _ => Err(invalid("Explicit list mutation confirmation is required")),
    };
    self.interrupted();
    result?;
    self.lists().await
  }

  async fn create_list(&self, name: &str) -> Result<(), AppError> {
    let flow = self
      .request(
        "api/config/config_entries/flow",
        Some(json!({"handler":"local_todo", "show_advanced_options":false})),
      )
      .await?;
    let id = flow["flow_id"]
      .as_str()
      .filter(|s| s.bytes().all(|c| c.is_ascii_alphanumeric()))
      .ok_or_else(upstream)?;
    if flow["type"] != "form" || flow["step_id"] != "user" {
      return Err(upstream());
    }
    let result = self
      .request(
        &format!("api/config/config_entries/flow/{id}"),
        Some(json!({"todo_list_name":name})),
      )
      .await
      .map_err(|_| uncertain())?;
    if result["type"] != "create_entry" {
      return Err(invalid(
        "HA could not create list; check duplicate Local To-do names",
      ));
    }
    let entry = result["result"]["entry_id"]
      .as_str()
      .ok_or_else(uncertain)?;
    let registry = self.registry().await?;
    let entities: Vec<_> = registry
      .iter()
      .filter(|e| e["config_entry_id"] == entry && e["platform"] == "local_todo")
      .collect();
    if entities.len() != 1 {
      return Err(uncertain());
    }
    let id = entities[0]["entity_id"].as_str().ok_or_else(uncertain)?;
    if !self.discover().await?.iter().any(|l| l.id == id) {
      return Err(uncertain());
    }
    Ok(())
  }

  async fn delete_entry(&self, entry: &str) -> Result<(), AppError> {
    if !entry.bytes().all(|c| c.is_ascii_alphanumeric()) {
      return Err(upstream());
    }
    let response = self
      .client
      .delete(
        self
          .base
          .join(&format!("api/config/config_entries/entry/{entry}"))
          .map_err(|_| upstream())?,
      )
      .send()
      .await
      .map_err(|_| uncertain())?;
    if !response.status().is_success() {
      return Err(uncertain());
    }
    Ok(())
  }
}

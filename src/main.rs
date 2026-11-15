//! Binary entry point: load config, open the store, serve the API.

use my_ntr_server::api;
use my_ntr_server::config::Config;
use my_ntr_server::watch;
use std::time::Duration;

/// How often the watcher re-reads the files to pick up out-of-band edits (those made to
/// the file by something other than this server; API mutations announce themselves). It
/// bounds how long another client waits for such an edit; tiny files make a 1s poll cheap.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let config = Config::load().map_err(|error| error.to_string())?;
  let state = api::AppState::from_config(config.clone())?;

  watch::spawn(state.clone(), WATCH_INTERVAL);

  let addr = format!("{}:{}", config.host, config.port);
  let listener = tokio::net::TcpListener::bind(&addr).await?;
  println!(
    "my-ntr-server listening on http://{} (task backend: {}, note: {}, labels: {})",
    addr,
    config.task_backend,
    config.note_file.display(),
    config.labels_file.display()
  );
  axum::serve(listener, api::router(state)).await?;
  Ok(())
}

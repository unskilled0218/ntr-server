//! Binary entry point: load config, open the store, serve the API and WebUI.

use my_ntr_server::api;
use my_ntr_server::config::Config;
use my_ntr_server::note::NoteStore;
use my_ntr_server::store::Store;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let config = Config::from_env();
  let store = Store::load(&config.file)?;
  let note = NoteStore::load(&config.note_file)?;
  let state = api::AppState::new(store, note, config.clone());

  let addr = format!("{}:{}", config.host, config.port);
  let listener = tokio::net::TcpListener::bind(&addr).await?;
  println!(
    "my-ntr-server listening on http://{} (todo: {}, note: {})",
    addr,
    config.file.display(),
    config.note_file.display()
  );
  axum::serve(listener, api::router(state)).await?;
  Ok(())
}

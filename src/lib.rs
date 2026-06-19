//! my-ntr-server: a userless todo server using the todo.txt format,
//! exposing a JSON HTTP API and serving a small self-contained WebUI.

pub mod api;
pub mod config;
pub mod store;
pub mod task;

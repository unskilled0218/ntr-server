//! my-ntr-server: a userless todo server using the todo.txt format,
//! exposing a JSON HTTP API.

pub mod api;
pub mod atomicfile;
pub mod config;
pub mod homeassistant;
pub mod labelstore;
pub mod note;
pub mod store;
pub mod task;
pub mod task_backend;
pub mod watch;

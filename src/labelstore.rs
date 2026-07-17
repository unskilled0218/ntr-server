use crate::atomicfile;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Labels {
  pub project: Vec<String>,
  pub context: Vec<String>,
}

pub struct LabelStore {
  path: PathBuf,
  labels: Labels,
}

impl LabelStore {
  pub fn load(path: impl Into<PathBuf>) -> io::Result<Self> {
    let path = path.into();
    let labels = match fs::read_to_string(&path) {
      Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
      Err(e) if e.kind() == io::ErrorKind::NotFound => Labels::default(),
      Err(e) => return Err(e),
    };
    Ok(Self { path, labels })
  }

  pub fn get(&self) -> Labels {
    self.labels.clone()
  }

  pub fn replace(&mut self, labels: Labels) -> io::Result<()> {
    let text = serde_json::to_string(&labels)
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    atomicfile::write(&self.path, &text)?;
    self.labels = labels;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn temp_path(tag: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ntr-labels-{tag}-{}.json", std::process::id()));
    let _ = fs::remove_file(&path);
    path
  }

  #[test]
  fn missing_file_is_empty() {
    assert_eq!(
      LabelStore::load(temp_path("missing")).unwrap().get(),
      Labels::default()
    );
  }

  #[test]
  fn replace_then_get_round_trips() {
    let path = temp_path("roundtrip");
    let expected = Labels {
      project: vec!["work".into()],
      context: vec!["phone".into()],
    };
    let mut store = LabelStore::load(&path).unwrap();
    store.replace(expected.clone()).unwrap();
    assert_eq!(store.get(), expected);
    assert_eq!(LabelStore::load(path).unwrap().get(), expected);
  }

  #[test]
  fn malformed_json_is_empty() {
    let path = temp_path("malformed");
    fs::write(&path, "not json").unwrap();
    assert_eq!(LabelStore::load(path).unwrap().get(), Labels::default());
  }
}

//! The todo.txt task model: parsing, serialization, and completion handling.
//!
//! Format reference: https://github.com/todotxt/todo.txt

use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::fmt;

/// A single todo.txt task.
///
/// `description` holds the free-text remainder verbatim (including any
/// `+project`, `@context`, and `key:value` tokens), so it stays the
/// authoritative source for those derived fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
  pub completed: bool,
  pub priority: Option<char>,
  pub completion_date: Option<NaiveDate>,
  pub creation_date: Option<NaiveDate>,
  pub description: String,
}

/// Fields derived from a task's description, parsed in a single pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Derived {
  pub projects: Vec<String>,
  pub contexts: Vec<String>,
  pub tags: BTreeMap<String, String>,
}

impl Task {
  /// Parse one todo.txt line. Returns `None` for blank/whitespace-only lines.
  pub fn parse(line: &str) -> Option<Task> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      return None;
    }

    let mut rest = trimmed;
    let mut completed = false;
    if let Some(r) = rest.strip_prefix("x ") {
      completed = true;
      rest = r.trim_start();
    }

    let mut priority = None;
    if let Some((p, r)) = take_priority(rest) {
      priority = Some(p);
      rest = r;
    }

    let mut completion_date = None;
    let mut creation_date = None;
    if completed {
      if let Some((d, r)) = take_date(rest) {
        completion_date = Some(d);
        rest = r;
        if let Some((d2, r2)) = take_date(rest) {
          creation_date = Some(d2);
          rest = r2;
        }
      }
    } else if let Some((d, r)) = take_date(rest) {
      creation_date = Some(d);
      rest = r;
    }

    Some(Task {
      completed,
      priority,
      completion_date,
      creation_date,
      description: rest.to_string(),
    })
  }

  /// Parse projects, contexts, and key/value tags from the description in one pass.
  pub fn derived(&self) -> Derived {
    let mut d = Derived::default();
    for tok in self.description.split_whitespace() {
      if let Some(p) = tok.strip_prefix('+') {
        if !p.is_empty() && !d.projects.iter().any(|x| x == p) {
          d.projects.push(p.to_string());
        }
      } else if let Some(c) = tok.strip_prefix('@') {
        if !c.is_empty() && !d.contexts.iter().any(|x| x == c) {
          d.contexts.push(c.to_string());
        }
      } else if let Some((k, v)) = tok.split_once(':') {
        if !k.is_empty() && !v.is_empty() && !k.starts_with('+') && !k.starts_with('@') {
          d.tags.insert(k.to_string(), v.to_string());
        }
      }
    }
    d
  }

  /// Mark the task complete. The priority (if any) is preserved as a `pri:X`
  /// tag so it can be restored on un-completion (the todo.txt spec drops the
  /// leading priority from completed lines).
  pub fn complete(&mut self, today: NaiveDate) {
    if self.completed {
      return;
    }
    self.completed = true;
    self.completion_date = Some(today);
    if let Some(p) = self.priority.take() {
      if !self.description.is_empty() {
        self.description.push(' ');
      }
      self.description.push_str(&format!("pri:{}", p));
    }
  }

  /// Mark the task incomplete, restoring a `pri:X` tag back into the priority
  /// field if one is present.
  pub fn uncomplete(&mut self) {
    if !self.completed {
      return;
    }
    self.completed = false;
    self.completion_date = None;
    if let Some(p) = self.extract_priority_tag() {
      self.priority = Some(p);
    }
  }

  /// Remove a `pri:X` token from the description and return its priority.
  fn extract_priority_tag(&mut self) -> Option<char> {
    let mut found = None;
    let kept: Vec<&str> = self
      .description
      .split_whitespace()
      .filter(|tok| {
        if found.is_none() {
          if let Some(rest) = tok.strip_prefix("pri:") {
            let mut chars = rest.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
              if c.is_ascii_uppercase() {
                found = Some(c);
                return false;
              }
            }
          }
        }
        true
      })
      .collect();
    if found.is_some() {
      self.description = kept.join(" ");
    }
    found
  }
}

impl fmt::Display for Task {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut parts: Vec<String> = Vec::new();
    if self.completed {
      parts.push("x".to_string());
    }
    if let Some(p) = self.priority {
      parts.push(format!("({})", p));
    }
    if self.completed {
      // A creation date is only meaningful on a completed line when a
      // completion date precedes it, otherwise it would round-trip as the
      // completion date. Emit creation only alongside completion.
      if let Some(d) = self.completion_date {
        parts.push(d.to_string());
        if let Some(c) = self.creation_date {
          parts.push(c.to_string());
        }
      }
    } else if let Some(d) = self.creation_date {
      parts.push(d.to_string());
    }
    if !self.description.is_empty() {
      parts.push(self.description.clone());
    }
    write!(f, "{}", parts.join(" "))
  }
}

/// Take a leading `(A)`..`(Z)` priority token, returning it and the remainder.
fn take_priority(s: &str) -> Option<(char, &str)> {
  let bytes = s.as_bytes();
  if bytes.len() >= 3 && bytes[0] == b'(' && bytes[2] == b')' && bytes[1].is_ascii_uppercase() {
    let after = &s[3..];
    if after.is_empty() || after.starts_with(' ') {
      return Some((bytes[1] as char, after.trim_start()));
    }
  }
  None
}

/// Take a leading `YYYY-MM-DD` token, returning it and the remainder. Tokens
/// that merely look like dates but do not parse are left in place.
fn take_date(s: &str) -> Option<(NaiveDate, &str)> {
  let end = s.find(' ').unwrap_or(s.len());
  let token = &s[..end];
  let date = NaiveDate::parse_from_str(token, "%Y-%m-%d").ok()?;
  Some((date, s[end..].trim_start()))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
  }

  fn roundtrip(line: &str) {
    let task = Task::parse(line).unwrap();
    assert_eq!(task.to_string(), line, "round-trip mismatch");
  }

  #[test]
  fn parses_plain_task() {
    let t = Task::parse("buy milk").unwrap();
    assert!(!t.completed);
    assert_eq!(t.priority, None);
    assert_eq!(t.creation_date, None);
    assert_eq!(t.description, "buy milk");
  }

  #[test]
  fn parses_priority_and_creation_date() {
    let t = Task::parse("(A) 2026-06-19 thank Mom @phone").unwrap();
    assert_eq!(t.priority, Some('A'));
    assert_eq!(t.creation_date, Some(date("2026-06-19")));
    assert_eq!(t.description, "thank Mom @phone");
  }

  #[test]
  fn parses_completed_with_two_dates() {
    let t = Task::parse("x 2026-06-19 2026-06-01 measure space +chapel").unwrap();
    assert!(t.completed);
    assert_eq!(t.completion_date, Some(date("2026-06-19")));
    assert_eq!(t.creation_date, Some(date("2026-06-01")));
    assert_eq!(t.description, "measure space +chapel");
  }

  #[test]
  fn blank_line_is_none() {
    assert!(Task::parse("   ").is_none());
    assert!(Task::parse("").is_none());
  }

  #[test]
  fn invalid_date_stays_as_text() {
    let t = Task::parse("2026-13-99 not a date").unwrap();
    assert_eq!(t.creation_date, None);
    assert_eq!(t.description, "2026-13-99 not a date");
  }

  #[test]
  fn derives_projects_contexts_tags() {
    let t = Task::parse("call +work @office due:2026-06-30 +work").unwrap();
    let d = t.derived();
    assert_eq!(d.projects, vec!["work"]);
    assert_eq!(d.contexts, vec!["office"]);
    assert_eq!(d.tags.get("due").map(String::as_str), Some("2026-06-30"));
  }

  #[test]
  fn complete_preserves_priority_as_tag() {
    let mut t = Task::parse("(A) 2026-06-01 write report").unwrap();
    t.complete(date("2026-06-19"));
    assert!(t.completed);
    assert_eq!(t.priority, None);
    assert_eq!(t.completion_date, Some(date("2026-06-19")));
    assert!(t.description.contains("pri:A"));

    t.uncomplete();
    assert!(!t.completed);
    assert_eq!(t.priority, Some('A'));
    assert!(!t.description.contains("pri:A"));
    assert_eq!(t.description, "write report");
  }

  #[test]
  fn round_trips_representative_lines() {
    roundtrip("buy milk");
    roundtrip("(B) call the bank @phone");
    roundtrip("2026-06-19 plain with creation date");
    roundtrip("(A) 2026-06-19 thank Mom +family @phone due:2026-06-30");
    roundtrip("x 2026-06-19 2026-06-01 finished task +proj");
    roundtrip("x 2026-06-19 finished without creation date");
  }
}

# Brooks-Lint Review — Code (REVIEW.md step 1)

**Mode:** PR Review
**Scope:** branch changes vs main (`main...HEAD`) — src/task.rs, src/store.rs, src/config.rs, src/api.rs, src/main.rs, tests/api.rs, web/*
**Health Score:** 98/100

Implementation is clean, layered, and well-tested; two minor in-module duplications in `store.rs` are worth tidying.

## Findings

### Green Suggestion

**Knowledge Duplication — File-rendering logic exists in two places**
Symptom: `Store::raw()` and the free function `write_file()` in `src/store.rs` both build the file text with the same loop (`task.to_string()` + `'\n'` per entry).
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself; Fowler — Refactoring (Duplicate Code).
Consequence: The todo.txt "render whole document" decision lives in two spots; a future change to line termination or trailing-newline handling must be made in both, and they can drift.
Remedy: Extract a single `render(entries: &[Entry]) -> String` and have both `raw()` and `write_file()` call it.

**Knowledge Duplication — Repeated clone/find/save mutation boilerplate**
Symptom: `update`, `complete`, and `uncomplete` in `src/store.rs` each repeat: clone `entries`, `iter_mut().find(id).ok_or(NotFound)`, mutate, `self.save(...)`.
Source: Fowler — Refactoring (Duplicate Code); McConnell — Code Complete, Ch. 7.
Consequence: The "locate by id under write-then-swap" decision is copied three times; a change to lookup or persistence semantics must touch all three identically.
Remedy: Extract a private `mutate(&mut self, id, f: impl FnOnce(&mut Task)) -> Result<(), StoreError>` helper and express the three operations in terms of it.

## Step 7 — Quick Test Check

Tests accompany the new behavior (14 unit + 7 integration), names express scenario and outcome, and there is no mock usage. All three signals clean — no test findings.

## Summary

No correctness or architecture issues; dependencies flow cleanly (task -> store -> api -> main, acyclic) and the domain model carries behavior rather than being anemic. The two suggestions are localized `store.rs` cleanups. Per REVIEW.md they will be implemented; expected post-fix score 100.

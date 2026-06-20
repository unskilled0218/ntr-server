# Brooks-Lint Review

**Mode:** PR Review
**Scope:** plans/new.md (design plan for my-ntr-server)
**Health Score:** 93/100

A clear, well-scoped plan; the main risk is using the todo.txt line position as the public API identifier, which leaks a storage detail into the contract and creates a concurrent-client hazard.


## Findings

### Yellow Warning

**Change Propagation — Line-index IDs leak storage layout into the API contract**
Symptom: Section 4 and Section 5 define the public resource id as the 1-based line index of the task in todo.txt. `DELETE /api/tasks/:id` renumbers every later task, and the plan's only mitigation is documentation ("clients read-then-act").
Source: Ousterhout — A Philosophy of Software Design, Ch. 5 (Information Hiding and Leakage); Winters et al. — Software Engineering at Google, Ch. 1 (Hyrum's Law).
Consequence: This is a correctness hazard, not just an ergonomic one. With two API clients (the WebUI plus an external program — an explicit requirement), a read-then-act sequence has a time-of-check/time-of-use gap: client A fetches id 3, client B deletes id 1, and client A's next `PUT`/`DELETE`/`complete` on id 3 now silently mutates the wrong task. Documentation cannot close this gap.
Remedy: Either (a) make the API id stable for a task's lifetime by holding an in-memory `next_id` counter and a `id -> task` map alongside the Vec (the file stays pure todo.txt; ids are assigned at load by position but never reused within a process run), or (b) add an optimistic-concurrency guard so mutations are rejected when the target line changed. Pick one in the plan and specify it.

### Green Suggestion

**Accidental Complexity — Hand-rolled CLI/env parsing**
Symptom: Section 7 proposes parsing `--file/--host/--port/--add-creation-date` by hand over `std::env::args` "with env fallbacks; no extra clap dependency unless needed".
Source: Hunt & Thomas — The Pragmatic Programmer, Topic 4 (Good-Enough Software); Fowler — Refactoring (Speculative Generality, inverted — here it is hand-rolled scaffolding).
Consequence: Manual flag parsing with env fallbacks and a boolean flag tends to grow edge cases (`--flag=value` vs `--flag value`, `--no-...` negation, error messages), reimplementing a solved problem and adding untested code paths.
Remedy: Either keep config strictly env-var-only (trivial and dependency-free) or pull in a small parser; avoid the middle path of a bespoke arg parser. State the chosen approach.

**Cognitive Overload — Repeated re-parsing of `description` for derived fields**
Symptom: Section 3 derives `projects()`, `contexts()`, and `tags()` by re-parsing the `description` string on every call, and Section 5 serializes all three into every JSON task response.
Source: McConnell — Code Complete, Ch. 7 (High-Quality Routines).
Consequence: Minor: correctness is fine, but the same scan runs three-plus times per task per list response; for a list endpoint this is repeated work and three parsing code paths to keep consistent.
Remedy: Parse the description once into a small struct (projects, contexts, tags) when building the response DTO, rather than three independent scans. Low priority.


## Summary

The plan is solid and testable; the highest-leverage fix is to resolve the line-index ID hazard (Section 4/5) before implementation, because it is a correctness issue for the dual-client requirement, not a documentation gap. The two suggestions are minor and can be folded into the same revision. Recommended fix order: line-index IDs, then config parsing, then derived-field parsing.


# Brooks-Lint Review — Pass 2 (after revision)

**Mode:** PR Review
**Scope:** plans/new.md (revised)
**Health Score:** 99/100

All Pass-1 findings are resolved; one minor residual remains around id rotation on bulk replace.

## Findings

### Green Suggestion

**Change Propagation — IDs rotate on `PUT /api/raw`**
Symptom: Section 4 now uses process-stable ids (good), but a whole-file replace via `PUT /api/raw` reassigns ids from load order, invalidating ids held by other clients.
Source: Winters et al. — Software Engineering at Google, Ch. 1 (Hyrum's Law).
Consequence: An external program that cached ids and then issues (or races) a raw replace will find its ids point elsewhere. This is far narrower than the Pass-1 hazard — it only occurs on an explicit bulk operation, which is inherently destructive — so it is a Suggestion, not a Warning.
Remedy: Already mitigated by documenting `PUT /api/raw` as a bulk operation that rotates ids. No further action required for v1; acceptable as documented.

## Summary

The line-index hazard (Pass-1 Warning) is resolved by switching to process-stable in-memory ids that 404 rather than silently alias; config is now env-only, removing the bespoke-parser risk; and derived fields are parsed once via `Task::derived()`. The remaining item is a documented, narrow edge case on bulk replace. Score 99/100 — over the 95 gate; cleared to implement.


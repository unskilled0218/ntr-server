# Markdown note

- [Storage and API](#storage-and-api)

## Storage and API

The server keeps one independent Markdown note in `NOTE_FILE` (default `Note.md`). A missing file reads as empty. `GET /api/note` reads it; `PUT /api/note` replaces it using atomic file replacement.

There are no per-task notes or revision history. Saves are last-write-wins, not transactional conflict protection. [Client approval](../../app/docs/live-sync.md) protects observed drafts but cannot recover overwritten versions.

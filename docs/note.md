# Note (Note.md)

Alongside the task file the server keeps a single free-form Markdown note in `NOTE_FILE` (default `Note.md`). It is userless and unstructured — one block of text, the scratchpad counterpart to the structured todo.txt — stored in its own file, cached in memory, and written atomically (temp file + rename) just like the task store, so the file and memory never diverge. A missing file is treated as an empty note. There is no per-task note and no versioning; `PUT /api/note` is last-write-wins, the same as `PUT /api/raw` for the task file — a plain overwrite.

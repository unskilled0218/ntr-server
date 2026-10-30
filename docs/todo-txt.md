# todo.txt support

- [todo.txt support](#todotxt-support)
- [Legacy data and client compatibility](#legacy-data-and-client-compatibility)

Tasks use the plain [todo.txt](https://github.com/todotxt/todo.txt) format as their canonical store.

Supports completion (`x`), completion/creation dates, `+projects`, `@contexts`, and `key:value` tags. Priority is no longer a task feature or DTO field. No priority selection, sorting, filtering, or lifecycle conversion remains.

## Legacy data and client compatibility

An existing complete leading uppercase `(A)`–`(Z)` token is retained internally as hidden legacy data, before dates on an active line or immediately after `x ` on a completed line. Parsing still recognizes the lifecycle dates after that token. Completion and reopening keep the token in the same position relative to the dates; they never move it into or out of `pri:X`. Every `pri:` occurrence stays ordinary description text, including duplicates and order. Other tags, labels, and metadata remain unchanged. Reading does not migrate files, and whole-store writes preserve this data on unrelated tasks too.

Client `0.15.18` and server `0.2.8` provide this preservation guarantee together. The client uses `raw` to retain hidden tokens during edits and validates it against the DTO's completion state, dates, and description; unsafe or missing raw text fails visibly. Newly typed descriptions beginning with a complete `(A)`–`(Z)` token, including `(A)` alone, are rejected in the client because the file format cannot distinguish them from legacy data. Embedded tokens and malformed lookalikes remain ordinary text. Raw API input still accepts complete legacy lines for import and round-trip editing.

New-client edits work with valid older-server DTOs, including extra JSON fields. Older-server completion and reopening are not covered: they may convert leading legacy tokens to `pri:X` and consume ordinary `pri:X` tokens when reopening. Upgrade the server first if exact preservation matters. Older clients with this server are unsupported for priority-dependent behavior; any HTTP `priority` query returns a clear 400 rather than silently filtering or ignoring it.

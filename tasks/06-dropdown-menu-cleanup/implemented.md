# Implemented: dropdown menu cleanup (branch `dropdown-menu-cleanup`)

Frontend-only change to the add form's three option dropdowns. The
project/context "add" entry moved from the bottom to the top of its option list,
and the empty/no-selection placeholder in all three dropdowns (Priority,
Projects, Contexts) now reads `None` instead of repeating the field caption. No
server, API, or `Cargo.toml` change.

## What changed

- `web/app.js`
  - Added `EMPTY_LABEL = "None"` at module scope: the single source for the
    visible text of the empty (`value=""`) no-selection option shared by all three
    dropdowns. Used by both `initPriorityOptions` and `populateLabelSelect`.
  - Removed the now-dead `plural` field from both `KINDS` entries. It had a single
    reader — the project/context placeholder text — which is now `EMPTY_LABEL`.
    `singular` (add-entry text, modal title) and `prefix` (`composeText`) remain.
  - `initPriorityOptions`: the first option's text is `EMPTY_LABEL` ("None")
    instead of "Priority"; `value=""` unchanged, so priority stays optional.
  - `populateLabelSelect`: option order is now the flagged add `<option>` first,
    then the `EMPTY_LABEL` option, then the sorted labels (previously: placeholder,
    labels, add entry last). `select.value` is still set explicitly afterwards, so
    the add entry being first never becomes the default selection — the control
    shows `None`. The add entry is still recognised by its `data-add` flag, not by
    value or position, so detection is unchanged.
  - Updated the explanatory comments on `KINDS`/`ADD_VALUE`,
    `initPriorityOptions`, and `populateLabelSelect` to match the new order and
    label.

- `README.md`
  - WebUI section: each project/context dropdown now *begins* with the
    `+ Add project...` / `+ Add context...` entry (was "ends with"), and the
    no-selection option is described as the `None` option.

- No change to `web/index.html` (the `<label>` captions stay and the dropdowns are
  populated by JS), `web/style.css`, or any Rust source.

## Behaviour preserved

- `value=""` is still the no-selection state for all three selects. `composeText`
  still omits empty fields; `resetAddForm` still returns each select to `None`;
  `populateLabelSelect`'s selection-preservation fallback still lands on the empty
  option. Only the visible label and the add-entry position changed.
- The staging flow, the modal, `parseLabelInput`, the priority A–C range, the due
  control, inline edit, the tag filter, and `renderMeta` are untouched.

## Verification

- `cargo fmt --check` — clean.
- `cargo build` — compiles (web assets embedded via `include_str!`, so the edited
  `app.js` is revalidated into the binary).
- `cargo test` — 7 passed, 0 failed.
- `cargo clippy --all-targets` — clean.
- Frontend has no JS test harness (deliberate no-build-step decision, consistent
  with prior WebUI features), so the UI behaviour was verified by reading the
  changed code paths: option order, explicit `select.value` set, and the unchanged
  `data-add` detection.

## brooks-review

`reviews/dropdown-menu-cleanup.md`: plan reached 100/100 after one iteration (the
first pass at 99/100 flagged the `"None"` literal duplicated across two call
sites; resolved by introducing `EMPTY_LABEL`).

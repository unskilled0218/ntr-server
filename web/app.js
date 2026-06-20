"use strict";

// Everything that differs between the two label kinds lives here, so the prefix
// and user-facing word are defined once instead of being scattered across the
// markup and the compose/populate logic. The select itself holds its options and
// current value, so no separate per-kind selection state is needed.
const KINDS = {
  project: { prefix: "+", singular: "project" },
  context: { prefix: "@", singular: "context" },
};

// The "add" entry pinned at the top of each label select. It is recognised by
// its data-add flag (set in populateLabelSelect), never by this value, so a real
// label can never be mistaken for it. The value is a validator-forbidden token
// (`+` is rejected in a real label name) purely as defence-in-depth.
const ADD_VALUE = "+add";

// Visible text of the empty / no-selection option shared by all three add-form
// dropdowns (priority, project, context). value="" is the real no-selection
// state; this is only its label, kept in one place so the wording never drifts
// between the selects.
const EMPTY_LABEL = "None";

// How long after the last keystroke the note auto-saves. One named source so the
// debounce length is not a magic number sprinkled through the save logic.
const NOTE_SAVE_DELAY_MS = 800;

const state = {
  status: "all", // all | active | completed (-> completed query param)
  project: "", // "" = any project, else an exact project name
  context: "", // "" = any context, else an exact context name
  sort: "file", // key into SORTS
};

// Status filter options keyed by their <select> value. `completed` is the query
// param the option maps to (null = omit it), kept beside the label so buildQuery
// never re-decides what a status means (same shape as SORTS: label + behavior).
const STATUSES = {
  all: { label: "All", completed: null },
  active: { label: "Active", completed: false },
  completed: { label: "Completed", completed: true },
};

// Client-side sort orders. compare:null keeps the server's file order. Every
// comparator sorts ascending and sends tasks missing the key to the bottom
// (cmpMissingLast); JS's stable sort then keeps file order among ties.
const SORTS = {
  file: { label: "File order", compare: null },
  priority: { label: "Priority", compare: (a, b) => cmpMissingLast(a.priority, b.priority) },
  due: { label: "Due date", compare: (a, b) => cmpMissingLast(a.tags.due, b.tags.due) },
  created: { label: "Created", compare: (a, b) => cmpMissingLast(a.creation_date, b.creation_date) },
  alpha: {
    label: "A-Z",
    compare: (a, b) => a.description.localeCompare(b.description, undefined, { sensitivity: "base" }),
  },
};

// Labels created in the modal but not yet attached to any task. todo.txt has no
// label store, so a brand-new label lives here until a task using it is saved;
// the rendered options are (server-derived labels) ∪ staged. One Set per kind.
const staged = { project: new Set(), context: new Set() };

// The current real selection of each label select, remembered so that choosing
// the "add" entry can restore the prior pick instead of leaving the sentinel
// selected. Never holds the sentinel.
const priorLabel = { project: "", context: "" };

// The kind whose "add" entry opened the modal, so the submit handler knows which
// select to stage the new label into.
let modalKind = null;

const els = {
  form: document.getElementById("add-form"),
  input: document.getElementById("add-input"),
  priority: document.getElementById("add-priority"),
  due: document.getElementById("add-due"),
  // Label selects keyed by kind name so the compose/reset/load loops can read
  // els[kind] without a per-kind branch.
  project: document.getElementById("add-project"),
  context: document.getElementById("add-context"),
  list: document.getElementById("tasks"),
  empty: document.getElementById("empty"),
  error: document.getElementById("error"),
  noteInput: document.getElementById("note-input"),
  noteStatus: document.getElementById("note-status"),
  status: document.getElementById("filter-status"),
  sort: document.getElementById("filter-sort"),
  modal: document.getElementById("label-modal"),
  modalForm: document.getElementById("label-form"),
  modalTitle: document.getElementById("label-modal-title"),
  modalInput: document.getElementById("label-input"),
  modalError: document.getElementById("label-error"),
  modalCancel: document.getElementById("label-cancel"),
  main: document.querySelector("main"),
  panes: document.querySelector(".panes"),
  nav: document.querySelector(".bottom-nav"),
  navButtons: Array.from(document.querySelectorAll(".bottom-nav button")),
};

// Filter label selects keyed by kind name, parallel to how els[kind] keys the
// add-form label selects, so the populate/wire loops can read filterEls[kind]
// without a per-kind branch.
const filterEls = {
  project: document.getElementById("filter-project"),
  context: document.getElementById("filter-context"),
};

function showError(message) {
  els.error.textContent = message;
  els.error.hidden = !message;
}

// On a non-2xx response, surface the API's JSON {error} message (falling back to
// the status text) by throwing it. Shared by the JSON api() helper and the
// plain-text note fetches so the error-extraction logic lives in one place.
async function throwIfNotOk(res) {
  if (res.ok) return;
  let detail = res.statusText;
  try {
    const data = await res.json();
    if (data && data.error) detail = data.error;
  } catch (_) {
    /* non-JSON error body */
  }
  throw new Error(detail);
}

async function api(method, path, body) {
  const opts = { method, headers: {} };
  if (body !== undefined) {
    opts.headers["Content-Type"] = "application/json";
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(path, opts);
  await throwIfNotOk(res);
  if (res.status === 204) return null;
  return res.json();
}

// GET the note's Markdown text.
async function fetchNote() {
  const res = await fetch("/api/note");
  await throwIfNotOk(res);
  return res.text();
}

// PUT the note's Markdown text (text body, 204 on success). `keepalive` lets the
// request outlive the page for the pagehide flush; it defaults off for normal saves.
async function putNote(text, { keepalive = false } = {}) {
  const res = await fetch("/api/note", {
    method: "PUT",
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
    body: text,
    keepalive,
  });
  await throwIfNotOk(res);
}

function buildQuery() {
  const params = new URLSearchParams();
  const { completed } = STATUSES[state.status];
  if (completed !== null) params.set("completed", String(completed));
  if (state.project) params.set("project", state.project);
  if (state.context) params.set("context", state.context);
  const qs = params.toString();
  return qs ? `/api/tasks?${qs}` : "/api/tasks";
}

// Compare two optional sort keys ascending, with absent values (null/undefined)
// always ordering after present ones — so "no priority" / "no due date" sink to
// the bottom. ISO date strings and single-char priorities compare correctly with
// <, so no parsing is needed.
function cmpMissingLast(a, b) {
  if (a == null && b == null) return 0;
  if (a == null) return 1;
  if (b == null) return -1;
  return a < b ? -1 : a > b ? 1 : 0;
}

// Return the tasks in the order selected by the Sort dropdown. file order is a
// no-op (the server already returns file order); other keys sort a copy so the
// fetched array is never mutated.
function sortTasks(tasks) {
  const compare = SORTS[state.sort].compare;
  return compare ? [...tasks].sort(compare) : tasks;
}

function renderMeta(task) {
  const bits = [];
  if (task.creation_date) bits.push(`created ${task.creation_date}`);
  if (task.completion_date) bits.push(`done ${task.completion_date}`);
  if (task.projects.length) bits.push(task.projects.map((p) => `+${p}`).join(" "));
  if (task.contexts.length) bits.push(task.contexts.map((c) => `@${c}`).join(" "));
  return bits.join(" · ");
}

function taskElement(task) {
  const li = document.createElement("li");
  li.className = "task" + (task.completed ? " completed" : "");

  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.checked = task.completed;
  checkbox.addEventListener("change", () => toggleComplete(task));

  const bodyWrap = document.createElement("div");
  bodyWrap.className = "body";

  const text = document.createElement("span");
  text.className = "text";
  if (task.priority) {
    const pri = document.createElement("span");
    pri.className = "priority";
    pri.textContent = `(${task.priority})`;
    text.appendChild(pri);
  }
  text.appendChild(document.createTextNode(task.description));

  const meta = document.createElement("span");
  meta.className = "meta";
  meta.textContent = renderMeta(task);

  bodyWrap.appendChild(text);
  if (meta.textContent) bodyWrap.appendChild(meta);

  const actions = document.createElement("div");
  actions.className = "task-actions";

  const editBtn = document.createElement("button");
  editBtn.type = "button";
  editBtn.textContent = "Edit";
  editBtn.addEventListener("click", () => editTask(task));

  const delBtn = document.createElement("button");
  delBtn.type = "button";
  delBtn.textContent = "Delete";
  delBtn.addEventListener("click", () => deleteTask(task));

  actions.appendChild(editBtn);
  actions.appendChild(delBtn);

  li.appendChild(checkbox);
  li.appendChild(bodyWrap);
  li.appendChild(actions);
  return li;
}

// Most recent fetch, kept so a sort change can re-render without refetching (sort
// is client-side; only filter changes need a new query).
let lastTasks = [];

function render() {
  els.list.replaceChildren(...sortTasks(lastTasks).map(taskElement));
  els.empty.hidden = lastTasks.length !== 0;
}

async function refresh() {
  showError("");
  try {
    lastTasks = await api("GET", buildQuery());
    render();
  } catch (err) {
    showError(`Could not load tasks: ${err.message}`);
  }
}

async function toggleComplete(task) {
  const action = task.completed ? "uncomplete" : "complete";
  try {
    await api("POST", `/api/tasks/${task.id}/${action}`);
    await refresh();
  } catch (err) {
    showError(err.message);
    await refresh();
  }
}

async function deleteTask(task) {
  try {
    await api("DELETE", `/api/tasks/${task.id}`);
    await refresh();
  } catch (err) {
    showError(err.message);
  }
}

async function editTask(task) {
  const next = window.prompt("Edit task (todo.txt syntax):", task.raw);
  if (next === null) return;
  const text = next.trim();
  if (!text) return;
  try {
    await api("PUT", `/api/tasks/${task.id}`, { text });
    await refresh();
    await loadLabels();
  } catch (err) {
    showError(err.message);
  }
}

// Populate the priority select once: a "None" no-selection option, then A..C.
// Only A-C are offered in the picker by product decision; the server still
// accepts and displays any (A)..(Z) priority on tasks created or edited as raw
// todo.txt.
function initPriorityOptions() {
  els.priority.add(new Option(EMPTY_LABEL, ""));
  const first = "A".charCodeAt(0);
  const last = "C".charCodeAt(0);
  for (let code = first; code <= last; code += 1) {
    const letter = String.fromCharCode(code);
    els.priority.add(new Option(letter, letter));
  }
}

// Populate the Status and Sort dropdowns from their config maps and seed each
// filter label select with just its "all" option, so none of the filter selects
// is an empty <select> before the async loadLabels fills in the real labels.
function initFilterOptions() {
  for (const [value, s] of Object.entries(STATUSES)) els.status.add(new Option(s.label, value));
  for (const [key, s] of Object.entries(SORTS)) els.sort.add(new Option(s.label, key));
  els.status.value = state.status;
  els.sort.value = state.sort;
  for (const kind of Object.keys(KINDS)) populateFilterSelect(kind, new Set());
}

// Rebuild one label select's option list: the "add" entry pinned first, then the
// "None" no-selection option, then the sorted label names. The current pick is
// preserved across reloads only while it still exists, otherwise it falls back to
// the empty option. The add entry is flagged with dataset.add so the change
// handler recognises it without relying on its value or position; priorLabel[kind]
// is kept in sync with the surviving selection. select.value is always set
// explicitly below, so the add entry being first never makes it the default pick.
function populateLabelSelect(kind, names) {
  const select = els[kind];
  const previous = select.value;
  select.replaceChildren();
  const addOption = new Option(`+ Add ${KINDS[kind].singular}...`, ADD_VALUE);
  addOption.dataset.add = "1";
  select.add(addOption);
  select.add(new Option(EMPTY_LABEL, ""));
  for (const name of [...names].sort()) select.add(new Option(name, name));
  select.value = names.has(previous) ? previous : "";
  priorLabel[kind] = select.value;
}

// Rebuild one filter label select from the labels currently in the store. Unlike
// the add-form select it has no "add" entry and no staged labels (you can only
// filter by a label some task actually has) — just an "all" option, then the
// sorted names. The active choice is kept across reloads while it still exists; if
// its label is gone the select falls back to "all" and the matching filter state is
// cleared so the dropdown and the query never disagree.
function populateFilterSelect(kind, names) {
  const select = filterEls[kind];
  const previous = select.value;
  select.replaceChildren();
  select.add(new Option(`All ${KINDS[kind].singular}s`, ""));
  for (const name of [...names].sort()) select.add(new Option(name, name));
  const kept = names.has(previous) ? previous : "";
  select.value = kept;
  state[kind] = kept;
}

// Refresh both label selects from the labels currently in the store, unioned with
// any client-staged labels not yet on a task. Options are best-effort: a failure
// here leaves the form usable, the selects just keep their current options. A
// select shows only the placeholder + add entry until a label of that kind exists.
async function loadLabels() {
  let server;
  try {
    const tasks = await api("GET", "/api/tasks");
    server = { project: new Set(), context: new Set() };
    for (const task of tasks) {
      for (const p of task.projects) server.project.add(p);
      for (const c of task.contexts) server.context.add(c);
    }
  } catch (err) {
    showError(`Could not load label suggestions: ${err.message}`);
    return;
  }
  for (const kind of Object.keys(KINDS)) {
    populateLabelSelect(kind, new Set([...server[kind], ...staged[kind]]));
    populateFilterSelect(kind, server[kind]);
  }
}

// Validate a label name typed in the modal. Pure: no DOM, no side effects, so it
// is trivially testable. Returns { name } on success or { error } on rejection.
// Strips leading +/@ sigils a user may type, then forbids whitespace and any
// +/@/: in the bare name (a key:value name would compose into a tag, not a label).
function parseLabelInput(raw) {
  const name = raw.trim().replace(/^[+@]+/, "");
  if (!name) return { error: "Name is required." };
  if (/\s/.test(name)) return { error: "No spaces allowed in a project or context." };
  if (/[+@:]/.test(name)) return { error: "A project or context cannot contain + @ or :" };
  return { name };
}

// The "add" entry of a label select was chosen: restore the prior real selection
// (so the sentinel is never a stuck value) and open the modal for that kind. Any
// other change is a normal pick, remembered as the new prior value. Recognising
// the add entry by its dataset flag — not its value — means a real label sharing
// the sentinel's value string can never be misread as the add entry.
function onLabelSelectChange(kind) {
  const select = els[kind];
  if (select.selectedOptions[0]?.dataset.add) {
    select.value = priorLabel[kind];
    openLabelModal(kind);
    return;
  }
  priorLabel[kind] = select.value;
}

// Open the shared modal for one kind. Native showModal gives a focus trap,
// backdrop, and Escape-to-dismiss; dismissing changes nothing because the select
// was already restored to its prior value before this ran.
function openLabelModal(kind) {
  modalKind = kind;
  els.modalTitle.textContent = `Add ${KINDS[kind].singular}`;
  els.modalInput.value = "";
  els.modalError.hidden = true;
  els.modalError.textContent = "";
  els.modal.showModal();
  els.modalInput.focus();
}

// Modal submit: validate, then stage the new label, render it as an option, and
// select it for the kind that opened the modal. An invalid value shows an inline
// error and keeps the dialog open; an existing name is just selected, no error.
function confirmLabel() {
  const kind = modalKind;
  const result = parseLabelInput(els.modalInput.value);
  if (result.error) {
    els.modalError.textContent = result.error;
    els.modalError.hidden = false;
    return;
  }
  staged[kind].add(result.name);
  const names = new Set([...Array.from(els[kind].options).map((o) => o.value), result.name]);
  names.delete("");
  names.delete(ADD_VALUE);
  populateLabelSelect(kind, names);
  els[kind].value = result.name;
  priorLabel[kind] = result.name;
  els.modal.close();
}

// Build a todo.txt line from the form controls. Pure: reads values, returns a
// string. Priority must lead the line; the rest of the order is cosmetic. Each
// label select contributes at most one label (picker-only single-select).
function composeText() {
  const parts = [];
  if (els.priority.value) parts.push(`(${els.priority.value})`);
  const desc = els.input.value.trim();
  if (desc) parts.push(desc);
  for (const kind of Object.keys(KINDS))
    if (els[kind].value) parts.push(`${KINDS[kind].prefix}${els[kind].value}`);
  if (els.due.value) parts.push(`due:${els.due.value}`);
  return parts.join(" ").trim();
}

function resetAddForm() {
  els.input.value = "";
  els.priority.value = "";
  els.due.value = "";
  for (const kind of Object.keys(KINDS)) {
    els[kind].value = "";
    priorLabel[kind] = "";
  }
}

// The note text as last successfully persisted. The textarea is "dirty" (unsaved)
// when it differs from this; autosave's job is to drive it back to equal.
let savedNote = "";
// Pending debounce timer id (null when none is armed).
let noteSaveTimer = null;
// True while a PUT is in flight, so a second flush never starts an overlapping,
// possibly out-of-order, write (single-flight).
let savingNote = false;

// The one definition of "the note has unsaved edits": the editor's text differs from the
// last persisted text. Every save path (the flush guard, the post-save re-check, the
// page-close beacon) asks this, so the dirty rule has a single home.
function noteDirty() {
  return readNoteText(els.noteInput) !== savedNote;
}

// Single writer for the note status line; `isError` toggles the error styling so a
// failed load/save shows red in the same slot as the editing/saving/saved text.
function setNoteStatus(message, isError) {
  els.noteStatus.textContent = message;
  els.noteStatus.classList.toggle("is-error", Boolean(isError));
}

// (Re)arm the debounce: a save fires NOTE_SAVE_DELAY_MS after the last keystroke.
function scheduleNoteSave() {
  clearTimeout(noteSaveTimer);
  noteSaveTimer = setTimeout(flushNote, NOTE_SAVE_DELAY_MS);
}

// Persist the note now if it is dirty and no save is already running. Cancels any
// pending debounce (this IS the save it was waiting for). On completion it re-checks
// for edits made during the PUT and re-arms the debounce, so the newest text is never
// left unsaved. Used by the debounce, by blur, and after a save completes.
async function flushNote() {
  clearTimeout(noteSaveTimer);
  noteSaveTimer = null;
  if (savingNote) return; // single-flight; the in-flight save's re-check will catch up
  if (!noteDirty()) return; // nothing changed
  const text = readNoteText(els.noteInput);
  savingNote = true;
  setNoteStatus("Saving…", false);
  try {
    await putNote(text);
    savedNote = text;
    setNoteStatus("Saved", false);
    // The user may have typed during the PUT; persist that newest text too. (flushNote
    // clears the pending timer before the single-flight return, so a blur/debounce that
    // fired mid-save left nothing armed — this re-check is what re-arms it.) Only after
    // a success: a failed save waits for the next edit to retry rather than looping on
    // a timer against a failing server.
    if (noteDirty()) scheduleNoteSave();
  } catch (err) {
    setNoteStatus(`Could not save note: ${err.message}`, true);
  } finally {
    savingNote = false;
  }
}

// Best-effort save when the page is going away (tab/window close, navigation).
// Cannot await on unload, so it uses keepalive and ignores the result; the normal
// path will have saved everything earlier, this only covers the last unsaved edit.
function flushNoteBeacon() {
  if (!noteDirty()) return;
  putNote(readNoteText(els.noteInput), { keepalive: true }).catch(() => {});
}

async function loadNote() {
  try {
    const text = await fetchNote();
    savedNote = text;
    setEditorText(text, null); // render the loaded note in place; no caret to restore yet
    setNoteStatus("", false); // freshly loaded: clean, no message
  } catch (err) {
    setNoteStatus(`Could not load note: ${err.message}`, true);
  }
}

// --- In-place live Markdown editor (single contenteditable surface) ---
// The note is stored and served as raw Markdown and edited in place: the editor IS the
// preview. highlightMarkdown wraps each character of the source in styling spans WITHOUT
// adding, removing, or reordering a character, so the editor's text content stays exactly
// the raw Markdown (markers kept, just dimmed). Everything below highlightMarkdown is pure
// (string in, HTML string out); the text/caret helpers are the only DOM-touching code.

// Block line-patterns named once so each "what marks this block" decision has a single
// home. None carry the `g` flag, so reusing them with `.test`/`.match` is stateless.
const LIST_ITEM = /^(\s*)([-*+]|\d+\.)(\s+)(.*)$/; // leading indent, bullet, gap, text
const ORDERED_ITEM = /^\s*\d+\.\s+/; //              an ordered list item specifically
const FENCE = /^```/; //                             a fenced-code delimiter line
const QUOTE_MARKER = /^(>\s?)(.*)$/; //              a blockquote line: marker, then text
const HEADING = /^(#{1,6}\s+)(.*)$/; //              heading: hashes+gap marker, then text

// Escape the HTML-significant characters so note text can never inject markup when it
// becomes innerHTML. Because the browser decodes these entities back in textContent
// (&gt; -> >), escaping preserves the character count — the line invariant still holds.
function escapeHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// Classify the block a line begins — the single source of "what kind of line is this",
// reused by highlightMarkdown. LIST_ITEM/QUOTE_MARKER/HEADING also capture their parts,
// but a `.test` here only asks the yes/no question.
function blockKind(line) {
  if (line.trim() === "") return "blank";
  if (FENCE.test(line)) return "fence";
  if (HEADING.test(line)) return "heading";
  if (/^(---|\*\*\*|___)\s*$/.test(line)) return "hr";
  if (QUOTE_MARKER.test(line)) return "quote"; // ^>-anchored, so this implies startsWith(">")
  if (LIST_ITEM.test(line)) return "list";
  return "para";
}

// A dimmed, kept marker (e.g. `#`, `**`, `>`); the one place marker spans are built.
const mark = (s) => `<span class="md-mark">${s}</span>`;

// A replace callback that wraps one emphasis run: the fixed marker re-emitted as dimmed
// marks around a styled span, so textContent === the source run. The four bold/italic
// branches of formatInline share this; the link branch keeps its own (different) shape.
const emph = (marker, cls) => (_, t) => `${mark(marker)}<span class="${cls}">${t}</span>${mark(marker)}`;

// Wrap emphasis and links on one already-escaped, non-code run, re-emitting every original
// marker character inside an .md-mark span so textContent === the source run. Order mirrors
// task 10's formatText (links, then bold, then italic) so ** is not eaten by the single-*
// rule; every branch preserves characters, so a mis-paired marker is at worst cosmetic.
function formatInline(run) {
  return run
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      (_, t, u) => `${mark("[")}<span class="md-link">${t}</span>${mark("](")}<span class="md-url">${u}</span>${mark(")")}`,
    )
    .replace(/\*\*([^*]+)\*\*/g, emph("**", "md-strong"))
    .replace(/__([^_]+)__/g, emph("__", "md-strong"))
    .replace(/\*([^*]+)\*/g, emph("*", "md-em"))
    .replace(/_([^_]+)_/g, emph("_", "md-em"));
}

// Inline highlight of one already-escaped run: partition `code` spans (odd segments) from
// the rest. Code keeps its backticks (dimmed) and is not emphasis-formatted; non-code
// segments go to formatInline. Mirrors task 10's renderInline split, markers kept.
function highlightInline(escaped) {
  return escaped
    .split(/(`[^`]+`)/)
    .map((part, i) =>
      i % 2 ? `${mark("`")}<span class="md-code-inline">${part.slice(1, -1)}</span>${mark("`")}` : formatInline(part),
    )
    .join("");
}

// One renderer per line kind -> that source line's HTML. Keyed dispatch (like task 10's
// TAKE and tasks 11–12's NOTE_MODES) so adding a kind is one entry, not a new branch. Each
// div's textContent equals the source line exactly (markers kept), so the lines join back
// to the raw Markdown; an empty line becomes <br> so it keeps height and stays selectable.
// highlightMarkdown only ever produces these seven kinds (it precomputes fenced-code state,
// so code lines arrive as "code" and are literal), making the table total.
const LINE = {
  blank: () => `<div class="md-line"><br></div>`,
  code: (line) => {
    const body = escapeHtml(line);
    return `<div class="md-line md-code">${body === "" ? "<br>" : body}</div>`;
  },
  hr: (line) => `<div class="md-line md-hr">${mark(escapeHtml(line))}</div>`,
  heading: (line) => {
    const [, marker, text] = line.match(HEADING);
    return `<div class="md-line md-h${marker.trim().length}">${mark(escapeHtml(marker))}${highlightInline(escapeHtml(text))}</div>`;
  },
  quote: (line) => {
    const [, marker, text] = line.match(QUOTE_MARKER);
    return `<div class="md-line md-quote">${mark(escapeHtml(marker))}${highlightInline(escapeHtml(text))}</div>`;
  },
  list: (line) => {
    const [, indent, bullet, gap, text] = line.match(LIST_ITEM);
    return `<div class="md-line md-li">${escapeHtml(indent)}${mark(escapeHtml(bullet + gap))}${highlightInline(escapeHtml(text))}</div>`;
  },
  para: (line) => `<div class="md-line">${highlightInline(escapeHtml(line))}</div>`,
};

// Render the raw Markdown to the editor's HTML: one block <div> per source line, every
// character preserved. Fenced-code state is tracked across lines so a fence body styles as
// code regardless of its content. Supported subset matches task 10 (headings, bold/italic,
// inline and fenced code, one-level lists, blockquotes, rules, links); anything else is
// shown as its literal (dimmed where it looks like a marker) text. Used only by setEditorText.
function highlightMarkdown(src) {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  let inFence = false;
  return lines
    .map((line) => {
      const fence = FENCE.test(line);
      const kind = inFence || fence ? "code" : blockKind(line);
      if (fence) inFence = !inFence;
      return LINE[kind](line);
    })
    .join("");
}

// THE LINE INVARIANT (one home for the "lines join with a single \n" decision): the
// editor's logical text is each direct-child line's textContent joined by exactly one
// "\n". readNoteText performs that join; getCaret/setCaret count this same per-line length
// via lineLen, so they are exact inverses of the join and cannot drift. Change the
// separator here and the caret math follows from lineLen — there is no second copy.
const lineLen = (line) => line.textContent.length + 1; // +1 = the joining "\n"

// The editor's logical text — the raw Markdown, since highlighting only wraps characters,
// never changes them. What autosave reads and what the caret offsets count against.
function readNoteText(root) {
  return Array.from(root.children)
    .map((line) => line.textContent)
    .join("\n");
}

// Characters before the caret, counting one "\n" per line boundary — the inverse of the
// readNoteText join, so an offset survives a re-render unchanged. null when the caret is
// not in the editor (nothing to restore).
function getCaret(root) {
  const sel = window.getSelection();
  if (!sel.rangeCount) return null;
  const range = sel.getRangeAt(0);
  if (!root.contains(range.startContainer)) return null;
  let offset = 0;
  for (const line of root.children) {
    if (line === range.startContainer || line.contains(range.startContainer)) {
      const within = document.createRange();
      within.setStart(line, 0);
      within.setEnd(range.startContainer, range.startOffset);
      return offset + within.toString().length; // no block boundaries inside a line
    }
    offset += lineLen(line); // same per-line length the join implies (THE LINE INVARIANT)
  }
  return offset;
}

// Put the caret at character offset `pos` within `line`, walking its descendant text nodes.
// An empty line (just <br>) has no text node, so the caret collapses to the line itself.
function placeInLine(line, pos) {
  const sel = window.getSelection();
  const range = document.createRange();
  const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
  let acc = 0;
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (acc + node.length >= pos) {
      range.setStart(node, pos - acc);
      range.collapse(true);
      sel.removeAllRanges();
      sel.addRange(range);
      return;
    }
    acc += node.length;
  }
  range.selectNodeContents(line);
  range.collapse(false); // end of an empty/short line
  sel.removeAllRanges();
  sel.addRange(range);
}

// Place the caret at logical offset `pos`, finding its line then the spot within it.
function setCaret(root, pos) {
  for (const line of root.children) {
    if (pos <= line.textContent.length) return placeInLine(line, pos);
    pos -= lineLen(line); // step over this line and its joining "\n" (THE LINE INVARIANT)
  }
  if (root.lastElementChild) placeInLine(root.lastElementChild, root.lastElementChild.textContent.length);
}

// The single write path: render `text` into the editor and (optionally) restore the caret
// to character offset `caret`. Load, every keystroke, Enter, and paste all go through here,
// so "what the editor shows" and "where the caret is" have one home. The is-empty class
// drives the CSS placeholder (a contenteditable is never CSS :empty once it holds a line).
function setEditorText(text, caret) {
  els.noteInput.innerHTML = highlightMarkdown(text);
  els.noteInput.classList.toggle("is-empty", text === "");
  if (caret != null) setCaret(els.noteInput, caret);
}

// Re-highlight in place, preserving the caret: read the current text, re-render, restore.
function rerenderNote() {
  setEditorText(readNoteText(els.noteInput), getCaret(els.noteInput));
}

// Insert `insert` at the caret, operating on the logical text so the DOM stays flat (one
// div per line). Used by Enter ("\n") and paste (clipboard text/plain), the two structural
// edits the browser would otherwise turn into nested blocks.
function spliceNote(insert) {
  const text = readNoteText(els.noteInput);
  const at = getCaret(els.noteInput) ?? text.length;
  setEditorText(text.slice(0, at) + insert + text.slice(at), at + insert.length);
  setNoteStatus("Editing…", false);
  scheduleNoteSave();
}

els.form.addEventListener("submit", async (e) => {
  e.preventDefault();
  // A description is the one required field; without it an add is a no-op.
  if (!els.input.value.trim()) return;
  const text = composeText();
  try {
    await api("POST", "/api/tasks", { text });
    resetAddForm();
    await refresh();
    await loadLabels();
  } catch (err) {
    showError(err.message);
  }
});

for (const kind of Object.keys(KINDS))
  els[kind].addEventListener("change", () => onLabelSelectChange(kind));

els.modalForm.addEventListener("submit", (e) => {
  e.preventDefault();
  confirmLabel();
});
els.modalCancel.addEventListener("click", () => els.modal.close());

// Status and the label filters change the server query, so they refetch; the sort
// is client-side, so it only re-renders the already-fetched list.
els.status.addEventListener("change", () => {
  state.status = els.status.value;
  refresh();
});
for (const kind of Object.keys(KINDS))
  filterEls[kind].addEventListener("change", () => {
    state[kind] = filterEls[kind].value;
    refresh();
  });
els.sort.addEventListener("change", () => {
  state.sort = els.sort.value;
  render();
});

// True between compositionstart/end so we don't re-render mid-IME and drop or duplicate
// the characters being composed (CJK, accents). The highlight refreshes once on commit.
let composingNote = false;
els.noteInput.addEventListener("compositionstart", () => {
  composingNote = true;
});
els.noteInput.addEventListener("compositionend", () => {
  composingNote = false;
  rerenderNote();
});

// Autosave: typing shows "Editing…" and (re)arms the debounce; leaving the field flushes
// at once; closing the page flushes best-effort so the last edit survives. Each keystroke
// also re-highlights in place (caret preserved), so the editor renders as you type.
els.noteInput.addEventListener("input", () => {
  setNoteStatus("Editing…", false);
  scheduleNoteSave();
  if (!composingNote) rerenderNote();
});

// Enter and paste are reimplemented on the logical text so the DOM stays flat (one div per
// line): Enter splices a "\n"; paste splices the plain-text clipboard, which also keeps any
// pasted rich HTML out of the editor. Ordinary typing/deletion is left to the browser
// (native caret, IME, undo) and re-highlighted by the input handler above.
els.noteInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    spliceNote("\n");
  }
});
els.noteInput.addEventListener("paste", (e) => {
  e.preventDefault();
  spliceNote(e.clipboardData.getData("text/plain"));
});
els.noteInput.addEventListener("blur", flushNote);
window.addEventListener("pagehide", flushNoteBeacon);

// --- Mobile view switching (bottom nav + swipe) -------------------------------------------
// The two panes are an ordered list of views: the nav-button order, the swipe direction, and
// the default view all derive from this one array. Add a view here + a matching <button> and
// nothing else changes. (THE VIEW ORDER — one home.)
const VIEWS = ["note", "todo"];

// The single place the active view changes: record it on <main> (CSS reads data-view to show
// one pane on mobile) and reflect it on the nav buttons. Both the nav click and the swipe
// call this — one switch, two inputs.
function setView(view) {
  els.main.dataset.view = view;
  for (const btn of els.navButtons) {
    const active = btn.dataset.view === view;
    btn.classList.toggle("is-active", active);
    if (active) btn.setAttribute("aria-current", "page");
    else btn.removeAttribute("aria-current");
  }
}

for (const btn of els.navButtons) {
  btn.addEventListener("click", () => setView(btn.dataset.view));
}

const SWIPE_MIN = 50; // px of horizontal travel before a drag counts as a view swipe

// "Are we in the mobile single-pane layout?" — read CSS's own decision instead of re-encoding
// the 720px breakpoint here, so the breakpoint lives in exactly one place (the @media rule).
// The bottom bar is display:none on desktop and shown on mobile, so its computed display IS
// the layout mode.
const onMobile = () => getComputedStyle(els.nav).display !== "none";

let swipeStartX = null;
let swipeStartY = null;
els.panes.addEventListener(
  "touchstart",
  (e) => {
    // Only single-finger gestures while the single-pane layout is active; else ignore.
    if (!onMobile() || e.touches.length !== 1) {
      swipeStartX = null;
      return;
    }
    swipeStartX = e.touches[0].clientX;
    swipeStartY = e.touches[0].clientY;
  },
  { passive: true },
);
els.panes.addEventListener(
  "touchend",
  (e) => {
    if (swipeStartX === null) return;
    const dx = e.changedTouches[0].clientX - swipeStartX;
    const dy = e.changedTouches[0].clientY - swipeStartY;
    swipeStartX = null;
    // Ignore taps, short drags, and vertical-dominant drags (those scroll the pane).
    if (Math.abs(dx) < SWIPE_MIN || Math.abs(dx) <= Math.abs(dy)) return;
    const cur = VIEWS.indexOf(els.main.dataset.view);
    const next = cur + (dx < 0 ? 1 : -1); // swipe left → next view, right → previous
    if (next >= 0 && next < VIEWS.length) setView(VIEWS[next]);
  },
  { passive: true },
);

initPriorityOptions();
initFilterOptions();
// Adopt the view declared in the markup (<main data-view="note">) so first paint is correct
// without JS; this only syncs the nav highlight. VIEWS[0] is a defensive fallback.
setView(els.main.dataset.view || VIEWS[0]);
refresh();
loadLabels();
loadNote();

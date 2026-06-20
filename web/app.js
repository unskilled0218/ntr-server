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

const state = {
  filter: "all", // all | active | completed
  tag: "",
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
  tagFilter: document.getElementById("tag-filter"),
  filterButtons: Array.from(document.querySelectorAll("#filters button[data-filter]")),
  modal: document.getElementById("label-modal"),
  modalForm: document.getElementById("label-form"),
  modalTitle: document.getElementById("label-modal-title"),
  modalInput: document.getElementById("label-input"),
  modalError: document.getElementById("label-error"),
  modalCancel: document.getElementById("label-cancel"),
};

function showError(message) {
  els.error.textContent = message;
  els.error.hidden = !message;
}

async function api(method, path, body) {
  const opts = { method, headers: {} };
  if (body !== undefined) {
    opts.headers["Content-Type"] = "application/json";
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(path, opts);
  if (!res.ok) {
    let detail = res.statusText;
    try {
      const data = await res.json();
      if (data && data.error) detail = data.error;
    } catch (_) {
      /* non-JSON error body */
    }
    throw new Error(detail);
  }
  if (res.status === 204) return null;
  return res.json();
}

function buildQuery() {
  const params = new URLSearchParams();
  if (state.filter === "active") params.set("completed", "false");
  if (state.filter === "completed") params.set("completed", "true");
  const tag = state.tag.trim();
  if (tag.startsWith("+")) params.set("project", tag.slice(1));
  else if (tag.startsWith("@")) params.set("context", tag.slice(1));
  const qs = params.toString();
  return qs ? `/api/tasks?${qs}` : "/api/tasks";
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

async function refresh() {
  showError("");
  try {
    const tasks = await api("GET", buildQuery());
    els.list.replaceChildren(...tasks.map(taskElement));
    els.empty.hidden = tasks.length !== 0;
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
  for (const kind of Object.keys(KINDS))
    populateLabelSelect(kind, new Set([...server[kind], ...staged[kind]]));
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

els.filterButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    state.filter = btn.dataset.filter;
    els.filterButtons.forEach((b) => b.classList.toggle("active", b === btn));
    refresh();
  });
});

let tagTimer = null;
els.tagFilter.addEventListener("input", () => {
  state.tag = els.tagFilter.value;
  clearTimeout(tagTimer);
  tagTimer = setTimeout(refresh, 200);
});

initPriorityOptions();
refresh();
loadLabels();

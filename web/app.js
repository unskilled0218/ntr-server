"use strict";

// Everything that differs between the two label kinds lives here, so prefixes
// and user-facing words are defined once instead of being scattered across the
// markup and the compose/render logic.
const KINDS = {
  project: { prefix: "+", singular: "project", plural: "Projects" },
  context: { prefix: "@", singular: "context", plural: "Contexts" },
};

const state = {
  filter: "all", // all | active | completed
  tag: "",
};

// Per-kind label sets. `known` is what the dropdown offers (server-derived plus
// staged), `selected` is the current pick, `staged` is labels created via the
// modal that no task uses yet (see loadLabels).
const labels = {
  project: { known: new Set(), selected: new Set(), staged: new Set() },
  context: { known: new Set(), selected: new Set(), staged: new Set() },
};

const els = {
  form: document.getElementById("add-form"),
  input: document.getElementById("add-input"),
  priority: document.getElementById("add-priority"),
  due: document.getElementById("add-due"),
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

// Filled by initDropdowns(): kind -> { render } controller.
const dropdowns = {};

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

// Populate the priority select once: a blank placeholder, then A..C. Only A-C
// are offered in the picker by product decision; the server still accepts and
// displays any (A)..(Z) priority on tasks created or edited as raw todo.txt.
function initPriorityOptions() {
  els.priority.add(new Option("Priority", ""));
  const first = "A".charCodeAt(0);
  const last = "C".charCodeAt(0);
  for (let code = first; code <= last; code += 1) {
    const letter = String.fromCharCode(code);
    els.priority.add(new Option(letter, letter));
  }
}

// Wire one project/context dropdown: a toggle button revealing a panel of
// checkbox rows plus an "Add" button. Returns a controller exposing render().
function setupDropdown(kind, root) {
  const meta = KINDS[kind];
  const toggle = root.querySelector(".dropdown-toggle");
  const panel = root.querySelector(".dropdown-panel");
  const options = root.querySelector(".dropdown-options");
  const addBtn = root.querySelector(".dropdown-add");
  const selected = labels[kind].selected;

  function refreshToggle() {
    toggle.textContent = selected.size
      ? `${meta.plural} (${selected.size})`
      : `Select ${meta.plural.toLowerCase()}`;
  }

  function render() {
    const rows = Array.from(labels[kind].known)
      .sort()
      .map((name) => {
        const row = document.createElement("label");
        row.className = "dropdown-option";
        const box = document.createElement("input");
        box.type = "checkbox";
        box.checked = selected.has(name);
        box.addEventListener("change", () => {
          if (box.checked) selected.add(name);
          else selected.delete(name);
          refreshToggle();
        });
        row.appendChild(box);
        row.appendChild(document.createTextNode(name));
        return row;
      });
    options.replaceChildren(...rows);
    refreshToggle();
  }

  function setOpen(open) {
    panel.hidden = !open;
    toggle.setAttribute("aria-expanded", String(open));
  }

  toggle.addEventListener("click", () => {
    const willOpen = panel.hidden;
    closeAllDropdowns();
    if (willOpen) setOpen(true);
  });
  addBtn.addEventListener("click", () => openLabelModal(kind));

  render();
  return { render, setOpen };
}

function closeAllDropdowns() {
  for (const kind of Object.keys(dropdowns)) dropdowns[kind].setOpen(false);
}

function initDropdowns() {
  for (const root of document.querySelectorAll(".dropdown")) {
    const kind = root.dataset.kind;
    dropdowns[kind] = setupDropdown(kind, root);
  }
  document.addEventListener("click", (e) => {
    if (!e.target.closest(".dropdown")) closeAllDropdowns();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !els.modal.open) closeAllDropdowns();
  });
}

// --- Add-label modal (shared between both kinds) ---

let modalKind = null;

function openLabelModal(kind) {
  modalKind = kind;
  els.modalTitle.textContent = `Add ${KINDS[kind].singular}`;
  els.modalInput.value = "";
  els.modalError.hidden = true;
  els.modalError.textContent = "";
  els.modal.showModal();
  els.modalInput.focus();
}

// Normalize and validate a typed label. Returns { name } on success or
// { error } describing why it was rejected. A leading +/@ the user types is
// stripped; todo.txt labels are single whitespace-free tokens.
function parseLabelInput(raw) {
  const name = raw.trim().replace(/^[+@]/, "");
  if (!name) return { error: "Name is required." };
  if (/\s/.test(name)) return { error: "No spaces allowed in a project or context." };
  return { name };
}

function confirmLabel() {
  const kind = modalKind;
  const { name, error } = parseLabelInput(els.modalInput.value);
  if (error) {
    els.modalError.textContent = error;
    els.modalError.hidden = false;
    return;
  }
  labels[kind].staged.add(name);
  labels[kind].known.add(name);
  labels[kind].selected.add(name);
  dropdowns[kind].render();
  els.modal.close();
}

// Rebuild each kind's known set from the labels currently in the store, merging
// in any staged labels, and re-render the dropdowns. Selections that no longer
// exist are dropped. Suggestions are best-effort: a failure here leaves the
// form usable, it just offers no server-derived options.
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
    const known = new Set([...server[kind], ...labels[kind].staged]);
    labels[kind].known = known;
    for (const name of [...labels[kind].selected])
      if (!known.has(name)) labels[kind].selected.delete(name);
    dropdowns[kind].render();
  }
}

// Build a todo.txt line from the form controls. Pure: reads values, returns a
// string. Priority must lead the line; the rest of the order is cosmetic.
function composeText() {
  const parts = [];
  if (els.priority.value) parts.push(`(${els.priority.value})`);
  const desc = els.input.value.trim();
  if (desc) parts.push(desc);
  for (const kind of Object.keys(KINDS))
    for (const name of labels[kind].selected) parts.push(`${KINDS[kind].prefix}${name}`);
  if (els.due.value) parts.push(`due:${els.due.value}`);
  return parts.join(" ").trim();
}

function resetAddForm() {
  els.input.value = "";
  els.priority.value = "";
  els.due.value = "";
  for (const kind of Object.keys(KINDS)) {
    labels[kind].selected.clear();
    dropdowns[kind].render();
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
initDropdowns();
refresh();
loadLabels();

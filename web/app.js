"use strict";

// Everything that differs between the two label kinds lives here, so the prefix
// and user-facing word are defined once instead of being scattered across the
// markup and the compose/populate logic. The select itself holds its options and
// current value, so no separate per-kind selection state is needed.
const KINDS = {
  project: { prefix: "+", plural: "Projects" },
  context: { prefix: "@", plural: "Contexts" },
};

const state = {
  filter: "all", // all | active | completed
  tag: "",
};

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

// Rebuild one label select's option list: a blank placeholder (text = the field
// caption, mirroring the Priority select) followed by the sorted label names.
// The current pick is preserved across reloads only while it still exists,
// otherwise it falls back to the placeholder.
function populateLabelSelect(kind, names) {
  const select = els[kind];
  const previous = select.value;
  select.replaceChildren();
  select.add(new Option(KINDS[kind].plural, ""));
  for (const name of [...names].sort()) select.add(new Option(name, name));
  select.value = names.has(previous) ? previous : "";
}

// Refresh both label selects from the labels currently in the store. Options are
// best-effort: a failure here leaves the form usable, the selects just keep their
// current options. A select stays empty (placeholder only) until the file holds
// at least one label of that kind.
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
  for (const kind of Object.keys(KINDS)) populateLabelSelect(kind, server[kind]);
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
  for (const kind of Object.keys(KINDS)) els[kind].value = "";
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

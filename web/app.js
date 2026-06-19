"use strict";

const state = {
  filter: "all", // all | active | completed
  tag: "",
};

const els = {
  form: document.getElementById("add-form"),
  input: document.getElementById("add-input"),
  priority: document.getElementById("add-priority"),
  due: document.getElementById("add-due"),
  tags: document.getElementById("add-tags"),
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
    await loadTagOptions();
  } catch (err) {
    showError(err.message);
  }
}

// Populate the priority select once: a blank placeholder, then A..Z.
function initPriorityOptions() {
  const placeholder = new Option("Priority", "");
  els.priority.add(placeholder);
  for (let code = 65; code <= 90; code += 1) {
    const letter = String.fromCharCode(code);
    els.priority.add(new Option(letter, letter));
  }
}

// Rebuild the projects/contexts select from the labels currently in the store,
// preserving any still-valid selection. Suggestions are best-effort: a failure
// here leaves the form usable, it just offers no options.
async function loadTagOptions() {
  let tokens;
  try {
    const tasks = await api("GET", "/api/tasks");
    const set = new Set();
    for (const task of tasks) {
      for (const p of task.projects) set.add(`+${p}`);
      for (const c of task.contexts) set.add(`@${c}`);
    }
    tokens = Array.from(set).sort();
  } catch (err) {
    showError(`Could not load tag suggestions: ${err.message}`);
    return;
  }
  const selected = new Set(selectedTagTokens());
  els.tags.replaceChildren(
    ...tokens.map((tok) => {
      const opt = new Option(tok, tok);
      opt.selected = selected.has(tok);
      return opt;
    }),
  );
}

// Selected option values from the projects/contexts select.
function selectedTagTokens() {
  return Array.from(els.tags.selectedOptions, (opt) => opt.value);
}

// Build a todo.txt line from the form controls. Pure: reads values, returns a
// string. Priority must lead the line; the rest of the order is cosmetic.
function composeText() {
  const parts = [];
  if (els.priority.value) parts.push(`(${els.priority.value})`);
  const desc = els.input.value.trim();
  if (desc) parts.push(desc);
  for (const tok of selectedTagTokens()) parts.push(tok);
  if (els.due.value) parts.push(`due:${els.due.value}`);
  return parts.join(" ").trim();
}

function resetAddForm() {
  els.input.value = "";
  els.priority.value = "";
  els.due.value = "";
  for (const opt of els.tags.options) opt.selected = false;
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
    await loadTagOptions();
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
loadTagOptions();

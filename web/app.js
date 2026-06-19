"use strict";

const state = {
  filter: "all", // all | active | completed
  tag: "",
};

const els = {
  form: document.getElementById("add-form"),
  input: document.getElementById("add-input"),
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
  } catch (err) {
    showError(err.message);
  }
}

els.form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const text = els.input.value.trim();
  if (!text) return;
  try {
    await api("POST", "/api/tasks", { text });
    els.input.value = "";
    await refresh();
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

refresh();

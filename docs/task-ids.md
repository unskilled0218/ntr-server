# Task IDs

Ids are assigned at load time and are stable for the lifetime of the running process: deleting a task does not renumber the others, and a deleted id returns 404 rather than silently aliasing another task. Ids are not written into the todo.txt file (it stays portable to other todo.txt tools) and are reassigned on restart or on a `PUT /api/raw` bulk replace.

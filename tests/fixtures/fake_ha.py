"""Disposable TLS HA service fixture. Requires Python 3 and OpenSSL, no live HA."""

import json
import base64
import hashlib
import struct
import pathlib
import ssl
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

root = pathlib.Path(sys.argv[1])
subprocess.run([
  "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
  "-keyout", str(root / "ca-key.pem"), "-out", str(root / "ca.pem"),
  "-subj", "/CN=ntr disposable test CA", "-addext", "basicConstraints=critical,CA:TRUE",
], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
subprocess.run([
  "openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes",
  "-keyout", str(root / "key.pem"), "-out", str(root / "server.csr"),
  "-subj", "/CN=localhost",
], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
(root / "server.ext").write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n")
subprocess.run([
  "openssl", "x509", "-req", "-in", str(root / "server.csr"),
  "-CA", str(root / "ca.pem"), "-CAkey", str(root / "ca-key.pem"),
  "-CAcreateserial", "-days", "1", "-out", str(root / "server.pem"),
  "-extfile", str(root / "server.ext"),
], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
state = {"items": [], "next": 1, "features": 127, "writes": [], "mode": "", "on_write": [], "lists": {}, "names": {}, "admin": True}
lock = threading.RLock()
write_started = threading.Event()
write_released = threading.Event()


class Handler(BaseHTTPRequestHandler):
  def log_message(self, *_):
    pass

  def reply(self, value, status=200):
    data = json.dumps(value).encode()
    self.send_response(status)
    self.send_header("Content-Type", "application/json")
    self.send_header("Content-Length", str(len(data)))
    self.end_headers()
    try:
      self.wfile.write(data)
    except (BrokenPipeError, ConnectionResetError, ssl.SSLEOFError):
      pass  # Expected when testing a committed write whose client timed out.

  def do_GET(self):
    if self.path == "/api/websocket":
      return self.websocket()
    if self.path == "/barrier/wait":
      return self.reply({"started": write_started.wait(5)})
    if self.path == "/barrier/release":
      write_released.set()
      return self.reply({})
    with lock:
      if self.path == "/control":
        return self.reply(state)
      if self.path == "/redirect-target":
        state["redirect_hits"] = state.get("redirect_hits", 0) + 1
        return self.reply({"state":"0", "attributes":{"supported_features":127}})
      if self.headers.get("Authorization") != "Bearer fixture-secret":
        return self.reply({"secret": "fixture-secret"}, 401)
      if state["mode"] == "redirect":
        self.send_response(302)
        self.send_header("Location", "/redirect-target")
        self.end_headers()
        return
      if state["mode"] == "unavailable":
        return self.reply({}, 503)
      if self.path.startswith("/api/states/"):
        return self.reply({"state": state.get("entity_state", "0"), "attributes": {"supported_features": state["features"]}})
      if self.path == "/api/states":
        return self.reply([{"entity_id": entity, "state": state.get("entity_state", "0"), "attributes": {"supported_features": state["features"], "friendly_name": state["names"].get(entity, entity)}} for entity in ["todo.fixture", *state["lists"]]])
      self.reply({}, 404)

  def websocket(self):
    self.protocol_version = "HTTP/1.1"
    key = self.headers.get("Sec-WebSocket-Key", "")
    accept = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
    self.send_response(101)
    self.send_header("Upgrade", "websocket")
    self.send_header("Connection", "Upgrade")
    self.send_header("Sec-WebSocket-Accept", accept)
    self.end_headers()
    def send(value):
      data = json.dumps(value).encode()
      self.wfile.write(b"\x81" + (bytes([len(data)]) if len(data) < 126 else b"\x7e" + struct.pack("!H", len(data))) + data)
      self.wfile.flush()
    def receive():
      first = self.rfile.read(2)
      if len(first) != 2 or first[0] & 15 == 8:
        return None
      size = first[1] & 127
      if size == 126: size = struct.unpack("!H", self.rfile.read(2))[0]
      elif size == 127: size = struct.unpack("!Q", self.rfile.read(8))[0]
      if size > 4 * 1024 * 1024: return None
      mask = self.rfile.read(4) if first[1] & 128 else b""
      data = self.rfile.read(size)
      if mask: data = bytes(c ^ mask[i % 4] for i, c in enumerate(data))
      return json.loads(data)
    try:
      send({"type":"auth_required", "ha_version":"2026.11.0"})
      auth = receive()
      if not auth or auth.get("access_token") != "fixture-secret":
        return send({"type":"auth_invalid"})
      send({"type":"auth_ok", "ha_version":"2026.11.0"})
      command = receive()
      if not command: return
      with lock:
        kind = command["type"]
        success, result = True, None
        if kind == "auth/current_user": result = {"is_admin":state["admin"]}
        elif not state["admin"]: success = False
        elif kind == "config/entity_registry/list":
          result = [{"entity_id":e, "platform":state.get("platforms", {}).get(e, "local_todo"), "config_entry_id": "entry" + e.split(".")[1].replace("_", "")} for e in ["todo.fixture", *state["lists"]]]
        elif kind == "config/entity_registry/update":
          state["names"][command["entity_id"]] = command["name"]
          result = {"entity_entry": {"entity_id":command["entity_id"], "name":command["name"]}}
          state["writes"].append({"action":"rename_list", "body":command})
        else: success = False
        send({"id": command["id"], "type":"result", "success":success, "result":result})
      receive()  # Consume close frame; do not leave a persistent test socket.
    except (BrokenPipeError, ConnectionResetError, ssl.SSLEOFError):
      pass

  def do_DELETE(self):
    with lock:
      if self.headers.get("Authorization") != "Bearer fixture-secret" or not state["admin"]: return self.reply({}, 401)
      entry = self.path.rsplit("/", 1)[-1]
      entity = next((e for e in state["lists"] if "entry" + e.split(".")[1].replace("_", "") == entry), None)
      if not entity: return self.reply({}, 404)
      del state["lists"][entity]
      state["writes"].append({"action":"delete_list", "body":{"entity_id":entity}})
      return self.reply({"require_restart":False})

  def do_POST(self):
    body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
    with lock:
      if self.path == "/control":
        state.update(body)
        return self.reply(state)
      if self.headers.get("Authorization") != "Bearer fixture-secret":
        return self.reply({"secret": "fixture-secret"}, 401)
      if self.path.startswith("/api/config/config_entries/flow"):
        if not state["admin"]: return self.reply({}, 401)
        if "todo_list_name" not in body: return self.reply({"type":"form", "step_id":"user", "flow_id":"flow1"})
        name = body["todo_list_name"]
        entity = "todo." + "".join(c.lower() if c.isalnum() else "_" for c in name)
        if entity in state["lists"]: return self.reply({"type":"abort", "reason":"already_configured"})
        state["lists"][entity] = []
        state["names"][entity] = name
        state["writes"].append({"action":"create_list", "body":body})
        return self.reply({"type":"create_entry", "result":{"entry_id":"entry" + entity.split(".")[1].replace("_", "")}})
      entity = body.get("entity_id")
      items = state["items"] if entity == "todo.fixture" else state["lists"].get(entity)
      if items is None: return self.reply({}, 404)
      action = urlsplit(self.path).path.rsplit("/", 1)[-1]
      if action == "get_items":
        if state["mode"] in ("malformed", "fail_readback"):
          return self.reply({"secret": "fixture-secret"}, 500 if state["mode"] == "fail_readback" else 200)
        return self.reply({"changed_states": [], "service_response": {entity: {"items": items}}})
      state["writes"].append({"action": action, "body": body})
      if state["mode"] == "move_fail_status" and action == "update_item": return self.reply({}, 500)
      if state["mode"] == "move_fail_delete" and action == "remove_item": return self.reply({}, 500)
      if action == "add_item":
        item = {"uid": f"uid-{state['next']}", "summary": body["item"], "status": "needs_action"}
        state["next"] += 1
        for key in ("due_date", "due_datetime"):
          if key in body:
            item["due"] = body[key]
        if "description" in body: item["description"] = body["description"]
        items.append(item)
        if state["mode"] == "move_change_source" and state["items"]:
          state["items"][0]["summary"] = "concurrent source edit"
        if state["mode"] == "ambiguous_create":
          items.append({**item, "uid": f"uid-{state['next']}"})
          state["next"] += 1
      else:
        values = body["item"] if isinstance(body["item"], list) else [body["item"]]
        found = next((i for i in items if i["uid"] in values or i["summary"] in values), None)
        if found is None:
          return self.reply({}, 404)
        if action == "remove_item":
          items.remove(found)
        elif action == "update_item":
          if not any(k in body for k in ("rename", "status", "due_date", "due_datetime", "description")):
            return self.reply({}, 400)
          if "rename" in body:
            found["summary"] = body["rename"]
          for key in ("status", "description"):
            if key in body:
              found[key] = body[key]
          for key in ("due_date", "due_datetime"):
            if key in body:
              found["due"] = body[key]
      for change in state["on_write"]:
        target = next(i for i in state["items"] if i["uid"] == change["uid"])
        target.update(change)
      state["on_write"] = []
      if state["mode"] == "write_then_fail":
        state["mode"] = "fail_readback"
      if state["mode"] == "move_delete_then_fail" and action == "remove_item":
        state["mode"] = "fail_readback"
      if state["mode"] == "move_status_then_fail" and action == "update_item":
        state["mode"] = "fail_readback"
      if state["mode"] == "move_delete_block_metadata" and action == "remove_item":
        (root / "metadata.json.tmp").mkdir()
      if state["mode"] == "block_metadata":
        (root / "metadata.json.tmp").mkdir()
      if state["mode"] == "write_then_timeout":
        threading.Event().wait(1)
      if state["mode"] == "pause_write":
        write_started.set()
        if not write_released.wait(5):
          return self.reply({}, 500)
      self.reply([])


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root / "server.pem", root / "key.pem")
server.socket = context.wrap_socket(server.socket, server_side=True)
print(server.server_port, flush=True)
server.serve_forever()

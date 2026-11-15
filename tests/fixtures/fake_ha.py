"""Disposable TLS HA service fixture. Requires Python 3 and OpenSSL, no live HA."""

import json
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
state = {"items": [], "next": 1, "features": 127, "writes": [], "mode": "", "on_write": []}
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
      self.reply({}, 404)

  def do_POST(self):
    body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
    with lock:
      if self.path == "/control":
        state.update(body)
        return self.reply(state)
      if self.headers.get("Authorization") != "Bearer fixture-secret":
        return self.reply({"secret": "fixture-secret"}, 401)
      action = urlsplit(self.path).path.rsplit("/", 1)[-1]
      if action == "get_items":
        if state["mode"] in ("malformed", "fail_readback"):
          return self.reply({"secret": "fixture-secret"}, 500 if state["mode"] == "fail_readback" else 200)
        return self.reply({"changed_states": [], "service_response": {"todo.fixture": {"items": state["items"]}}})
      state["writes"].append({"action": action, "body": body})
      if body.get("entity_id") != "todo.fixture":
        return self.reply({}, 400)
      if action == "add_item":
        item = {"uid": f"uid-{state['next']}", "summary": body["item"], "status": "needs_action"}
        state["next"] += 1
        for key in ("due_date", "due_datetime"):
          if key in body:
            item["due"] = body[key]
        state["items"].append(item)
        if state["mode"] == "ambiguous_create":
          state["items"].append({**item, "uid": f"uid-{state['next']}"})
          state["next"] += 1
      else:
        values = body["item"] if isinstance(body["item"], list) else [body["item"]]
        found = next((i for i in state["items"] if i["uid"] in values or i["summary"] in values), None)
        if found is None:
          return self.reply({}, 404)
        if action == "remove_item":
          state["items"].remove(found)
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

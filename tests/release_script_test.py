"""Behavioral release tests using real Git/Bash and disposable engine seams."""

import json
import hashlib
import io
import os
from pathlib import Path
import shutil
import selectors
import subprocess
import sys
import tempfile
import tarfile
import time
import unittest


SERVER = Path(__file__).resolve().parent.parent
TOKEN = "fixture-token-never-print"

# The fake tools expose the actual CLI JSON contracts, not implementation methods.
TOOLS = r'''
import json
import base64
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
from urllib.parse import urlsplit, parse_qs, unquote

tool = Path(sys.argv[0]).name
args = sys.argv[1:]
state_path = Path(os.environ["FIXTURE_STATE"])
state = json.loads(state_path.read_text())
scenario = state.get("scenario", {})
input_text = sys.stdin.read() if tool == "curl" or (tool == "docker" and args[:1] == ["login"]) else ""
docker_config = os.environ.get("DOCKER_CONFIG")
event = {"tool": tool, "args": args, "stdin": input_text, "docker_config": docker_config,
         "buildx_config": os.environ.get("BUILDX_CONFIG"), "builder": os.environ.get("BUILDX_BUILDER"),
         "docker_context": os.environ.get("DOCKER_CONTEXT"), "docker_host": os.environ.get("DOCKER_HOST"),
         "token_env": os.environ.get("FORGEJO_TOKEN")}
with open(os.environ["FIXTURE_EVENTS"], "a") as output:
  output.write(json.dumps(event) + "\n")

index = os.environ["FIXTURE_INDEX"]
amd64 = os.environ["FIXTURE_AMD64"]
arm64 = os.environ["FIXTURE_ARM64"]
other = os.environ["FIXTURE_ATTESTATION"]

def fail(message, code=1):
  print(message, file=sys.stderr)
  sys.exit(code)

def emit(value):
  print(json.dumps(value))
  sys.exit(0)

if tool == "skopeo":
  if scenario.get("export_failure"):
    fail("archive export failed fixture-token-never-print")
  if args[:4] != ["copy", "--all", "--preserve-digests", "--src-authfile"]:
    fail("archive export must preserve all platform digests")
  if args[-2] != "docker://" + state["image"] + "@" + index:
    fail("archive export is not digest pinned")
  config = json.loads(Path(args[4]).read_text())
  if state["image"].split("/")[0] not in config.get("auths", {}):
    fail("missing temporary registry login")
  Path(args[-1].removeprefix("oci-archive:")).write_bytes(
    b"corrupted" if scenario.get("archive_corrupt") else Path(os.environ["FIXTURE_ARCHIVE"]).read_bytes()
  )
  sys.exit(0)

if tool == "curl":
  url = args[-1]
  parsed = urlsplit(url)
  path = parsed.path
  output = Path(args[args.index("--output") + 1])
  status = "200"
  value = {}
  if scenario.get("api_denied"):
    status = "403"
  elif "/git/commits/" in path:
    if unquote(path.split("/git/commits/", 1)[1]).startswith("refs/tags/") and not state.get("release"):
      status = "404"
      if scenario.get("tag_conflict"):
        status = "200"
        value = {"sha": "0" * 40}
    else:
      value = {"sha": "0" * 40 if scenario.get("remote_revision_conflict") else os.environ["FIXTURE_REVISION"]}
  elif path.startswith("/attachments/"):
    asset = next(a for a in state["release"]["assets"] if a["browser_download_url"] == url)
    content = base64.b64decode(state["uploads"][asset["name"]])
    if scenario.get("checksum_conflict") and asset["name"].endswith(".sha256"):
      content = b"wrong checksum"
    output.write_bytes(content)
    print("200", end="")
    sys.exit(0)
  elif "/releases/tags/" in path:
    value = state.get("release", {})
    status = "200" if value else "404"
    if scenario.get("release_conflict"):
      value = {"id": 1, "tag_name": "v0.2.2", "body": "different revision", "assets": []}
      status = "200"
  elif path.endswith("/releases"):
    value = json.loads(Path(args[args.index("--data-binary") + 1][1:]).read_text())
    value.update({"id": 1, "assets": []})
    state["release"] = value
    status = "201"
  elif path.endswith("/assets"):
    name = parse_qs(parsed.query)["name"][0]
    if scenario.get("upload_failure") == name:
      status = "500"
    else:
      attachment = args[args.index("--form") + 1].split("=@", 1)[1].strip('"')
      value = {"id": len(state["release"]["assets"]) + 1, "name": name,
               "browser_download_url": "https://" + parsed.netloc + "/attachments/" + name}
      state["release"]["assets"].append(value)
      state.setdefault("uploads", {})[name] = base64.b64encode(Path(attachment).read_bytes()).decode()
      status = "201"
  else:
    fail("unexpected Forgejo request " + url)
  state_path.write_text(json.dumps(state))
  output.write_text(json.dumps(value))
  print(status, end="")
  sys.exit(0)

if tool == "cargo":
  if args[:1] == ["metadata"]:
    packages = [{"name": "unrelated", "version": "9.0.0"},
                {"name": "my-ntr-server", "version": scenario.get("version", "0.2.2")}]
    emit({"packages": packages})
  if args == ["test", "--locked"]:
    if scenario.get("test_failure"):
      fail("fixture test failed fixture-token-never-print")
    if scenario.get("change_checkout"):
      Path("src/main.rs").write_text("changed during tests")
    print("fixture cargo tests passed")
    sys.exit(0)

if tool != "docker":
  fail("unexpected tool")
if args == ["info"]:
  if scenario.get("daemon_failure"):
    fail("engine unavailable")
  if docker_config != os.environ["ORIGINAL_DOCKER_CONFIG"]:
    config = json.loads((Path(docker_config) / "config.json").read_text())
    if config.get("auths") != {"unused.invalid": {}} or "credsStore" in config:
      fail("copied unrelated auth configuration")
    if str(Path(os.environ["ORIGINAL_DOCKER_CONFIG"]) / "cli-plugins") not in config["cliPluginsExtraDirs"]:
      fail("lost user plugin directory")
    if config.get("proxies") != {"default": {"httpProxy": "http://proxy.invalid:8080"}}:
      fail("lost build proxy settings")
    for context in ["remote-engine", "arm-node"]:
      if not (Path(docker_config) / (context + ".imported")).exists():
        fail("lost named context")
  sys.exit(0)
if args == ["context", "show"]:
  print("remote-engine")
  sys.exit(0)
if args[:2] == ["context", "export"]:
  Path(args[3]).write_text("fixture context TLS data")
  sys.exit(0)
if args[:2] == ["context", "import"]:
  (Path(docker_config) / (args[2] + ".imported")).write_text(Path(args[3]).read_text())
  sys.exit(0)
if args[:2] == ["buildx", "version"]:
  if scenario.get("buildx_missing"):
    fail("buildx plugin unavailable")
  print("github.com/docker/buildx fixture-contract-version")
  sys.exit(0)
if args[:2] == ["buildx", "inspect"]:
  if args != ["buildx", "inspect", "ntr-release", "--bootstrap"]:
    fail("invalid inspect flags or wrong builder")
  sys.exit(0)
if args[:2] == ["buildx", "create"]:
  if scenario.get("create_failure"):
    fail("builder creation failed")
  if args[args.index("--name") + 1] != "ntr-release" or args[args.index("--driver") + 1] != "docker-container" or "--use" in args:
    fail("incorrect automatic builder creation")
  state["builder_created"] = True
  state_path.write_text(json.dumps(state))
  print("ntr-release")
  sys.exit(0)
if args[:3] == ["buildx", "ls", "--format"]:
  if args[3] != '{{if eq .Name .Builder.Name}}{"Current":{{json .Builder.Current}},"Builder":{{json .Builder}}}{{end}}':
    fail("invalid builder JSON template")
  if docker_config != os.environ["ORIGINAL_DOCKER_CONFIG"]:
    if os.environ.get("BUILDX_CONFIG") != os.environ["EXPECTED_BUILDX_CONFIG"]:
      fail("lost Buildx state path")
  platforms = ["linux/amd64", "linux/arm64/v8"]
  if scenario.get("platform_missing"):
    platforms = ["linux/amd64"]
  name = "changed-builder" if scenario.get("builder_changed") and docker_config != os.environ["ORIGINAL_DOCKER_CONFIG"] else "ntr-release"
  if scenario.get("builder_missing") and not state.get("builder_created"):
    emit({"Current": True, "Builder": {"Name": "default", "Driver": "docker", "Nodes": []}})
  emit({"Current": False, "Builder": {"Name": name, "Driver": scenario.get("driver", "docker-container"),
        "Nodes": [{"Name": "node0", "Endpoint": "remote-engine", "Status": "running", "Platforms": platforms},
                  {"Name": "node1", "Endpoint": "arm-node", "Status": "running", "Platforms": []}]}})
if args[:1] == ["login"]:
  if scenario.get("login_failure"):
    fail("login rejected fixture-token-never-print")
  config_path = Path(docker_config) / "config.json"
  config = json.loads(config_path.read_text())
  config["auths"] = {args[1]: {"auth": "fixture-auth"}}
  config_path.write_text(json.dumps(config))
  sys.exit(0)
if args[:2] == ["buildx", "build"]:
  if scenario.get("live_progress"):
    print("fixture build stdout", flush=True)
    sys.stderr.write("fixture-token-")
    sys.stderr.flush()
    sys.stderr.write("never-print\nfixture build waiting\n")
    sys.stderr.flush()
    gate = Path(os.environ["FIXTURE_BUILD_GATE"])
    deadline = time.monotonic() + 10
    while not gate.exists():
      if time.monotonic() > deadline:
        fail("build progress was not streamed before completion")
      time.sleep(0.01)
  if scenario.get("terminate_build"):
    os.kill(os.getppid(), signal.SIGTERM)
    sys.exit(0)
  if scenario.get("build_failure"):
    fail("arm64 build failed fixture-token-never-print")
  if "--load" in args or args[args.index("--platform") + 1] != "linux/amd64,linux/arm64":
    fail("incorrect platform/export request")
  if args[args.index("--builder") + 1] != "ntr-release":
    fail("wrong build instance")
  if scenario.get("push_failure"):
    fail("registry export failed")
  state["published"] = True
  state["image"] = args[args.index("--tag") + 1].rsplit(":", 1)[0]
  state_path.write_text(json.dumps(state))
  if not scenario.get("metadata_missing"):
    value = {"containerimage.digest": other if scenario.get("metadata_wrong") else index}
    if scenario.get("metadata_invalid"):
      value = {"containerimage.digest": "invalid"}
    Path(args[args.index("--metadata-file") + 1]).write_text(json.dumps(value))
  sys.exit(0)
if args[:3] != ["buildx", "imagetools", "inspect"]:
  fail("unexpected Docker command: " + str(args))

reference = args[3]
template = args[args.index("--format") + 1]
if template == "{{json .Image}}":
  architecture = "amd64" if reference.endswith(amd64) else "arm64"
  if scenario.get("config_architecture") and architecture == "arm64":
    architecture = "amd64"
  revision = "0" * 40 if scenario.get("revision_conflict") else os.environ["FIXTURE_REVISION"]
  version = "9.9.9" if scenario.get("version_conflict") else scenario.get("version", "0.2.2")
  emit({"os": "linux", "architecture": architecture, "config": {"Labels": {
    "org.opencontainers.image.version": version, "org.opencontainers.image.revision": revision}}})

if not state.get("published") and not scenario.get("existing"):
  message = scenario.get("lookup_error", "ERROR: " + reference + ": not found")
  fail(message, scenario.get("lookup_code", 1))
if scenario.get("existing"):
  state["image"] = reference.split("@", 1)[0].rsplit(":", 1)[0] if "@" not in reference else reference.split("@", 1)[0]
  state_path.write_text(json.dumps(state))
if scenario.get("manifest_json"):
  print("not JSON")
  sys.exit(0)
if "@" in reference and reference.split("@", 1)[1] != index:
  fail("immutable image not found")
media = "application/vnd.oci.image.manifest.v1+json"
descriptors = [{"mediaType": media, "digest": amd64, "platform": {"os": "linux", "architecture": "amd64"}},
               {"mediaType": media, "digest": arm64, "platform": {"os": "linux", "architecture": "arm64", "variant": "v8"}},
               {"mediaType": media, "digest": other, "platform": {"os": "unknown", "architecture": "unknown"},
                "annotations": {"vnd.docker.reference.type": "attestation-manifest"}}]
if scenario.get("index_incomplete"):
  descriptors.pop(1)
if scenario.get("index_duplicate"):
  descriptors.append(descriptors[0])
if scenario.get("unmarked_attestation"):
  descriptors[-1].pop("annotations")
if scenario.get("descriptor_digest"):
  descriptors[0]["digest"] = "invalid"
if scenario.get("bad_variant"):
  descriptors[1]["platform"]["variant"] = "v9"
if scenario.get("arm_without_variant"):
  descriptors[1]["platform"].pop("variant")
digest = other if scenario.get("tag_moved") and "@" not in reference and state.get("published") else index
if scenario.get("immutable_mismatch") and "@" in reference:
  digest = other
emit({"digest": "invalid" if scenario.get("index_digest") else digest,
      "mediaType": scenario.get("index_media", "application/vnd.oci.image.index.v1+json"),
      "manifests": descriptors})
'''


class ReleaseScriptTests(unittest.TestCase):
  def setUp(self):
    self.temporary = tempfile.TemporaryDirectory(prefix="ntr-release-test-")
    self.addCleanup(self.temporary.cleanup)
    self.work = Path(self.temporary.name)
    self.root = self.work / "parent with spaces" / "server"
    (self.root / "scripts").mkdir(parents=True)
    for name in ("release.sh", "image_release.py"):
      shutil.copyfile(SERVER / "scripts" / name, self.root / "scripts" / name)
    (self.root / "Cargo.toml").write_text('[package]\nname = "my-ntr-server"\nversion = "0.2.2"\n')
    (self.root / "Dockerfile").write_text("FROM fixture\n")
    (self.root / ".gitignore").write_text("/release/\n")
    (self.root / "src").mkdir()
    (self.root / "src/main.rs").write_text("fn main() {}\n")
    self.git = shutil.which("git")
    self.git_run("init", "-q", "-b", "main")
    self.git_run("config", "user.name", "fixture-login")
    self.git_run("add", ".")
    self.git_run("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "Fixture checkout")
    self.revision = self.git_run("rev-parse", "HEAD").stdout.strip()
    blobs = {}
    def descriptor(value, media):
      content = json.dumps(value, sort_keys=True).encode()
      digest = "sha256:" + hashlib.sha256(content).hexdigest()
      blobs[digest] = content
      return {"mediaType": media, "digest": digest, "size": len(content)}
    image_type = "application/vnd.oci.image.manifest.v1+json"
    config_type = "application/vnd.oci.image.config.v1+json"
    descriptors = []
    layer = b"fixture compressed layer contents"
    layer_digest = "sha256:" + hashlib.sha256(layer).hexdigest()
    blobs[layer_digest] = layer
    for architecture in ("amd64", "arm64", "unknown"):
      config = descriptor({"os": "linux", "architecture": architecture, "config": {"Labels": {
        "org.opencontainers.image.version": "0.2.2", "org.opencontainers.image.revision": self.revision,
      }}}, config_type)
      child = descriptor({"schemaVersion": 2, "mediaType": image_type, "config": config, "layers": [
        {"mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "digest": layer_digest, "size": len(layer)},
      ]}, image_type)
      child["platform"] = {"os": "linux" if architecture != "unknown" else "unknown", "architecture": architecture}
      if architecture == "unknown":
        child["annotations"] = {"vnd.docker.reference.type": "attestation-manifest"}
      descriptors.append(child)
    self.amd64, self.arm64, self.attestation = [item["digest"] for item in descriptors]
    index = descriptor({"schemaVersion": 2, "mediaType": "application/vnd.oci.image.index.v1+json", "manifests": descriptors}, "application/vnd.oci.image.index.v1+json")
    self.index = index["digest"]
    self.archive = self.work / "fixture.tar"
    with tarfile.open(self.archive, "w") as archive:
      files = {"oci-layout": b'{"imageLayoutVersion":"1.0.0"}',
               "index.json": json.dumps({"schemaVersion": 2, "manifests": [index]}).encode(),
               **{"blobs/sha256/" + digest.split(":", 1)[1]: content for digest, content in blobs.items()}}
      for name, content in files.items():
        member = tarfile.TarInfo(name)
        member.size = len(content)
        archive.addfile(member, io.BytesIO(content))
    self.bin = self.work / "bin"
    self.bin.mkdir()
    helper = self.bin / "fake.py"
    helper.write_text(f"#!{sys.executable}\n" + TOOLS)
    helper.chmod(0o755)
    for tool in ("docker", "cargo", "curl", "skopeo"):
      (self.bin / tool).symlink_to(helper)
    for tool in ("git", "bash", "dirname", "python3"):
      (self.bin / tool).symlink_to(shutil.which(tool))
    self.state = self.work / "state.json"
    self.events = self.work / "events.jsonl"
    self.original = self.work / "original-docker"
    (self.original / "cli-plugins").mkdir(parents=True)
    (self.original / "config.json").write_text(json.dumps({
      "auths": {"unrelated.invalid": {"auth": "unrelated-secret"}},
      "credsStore": "fixture-helper", "currentContext": "remote-engine",
      "proxies": {"default": {"httpProxy": "http://proxy.invalid:8080"}},
    }))
    self.original_config = (self.original / "config.json").read_text()
    self.env = {key: value for key, value in os.environ.items() if not key.startswith(("FORGEJO_", "DOCKER_", "BUILDX_", "GIT_"))}
    self.env.update({
      "PATH": str(self.bin), "FORGEJO_TOKEN": TOKEN,
      "FIXTURE_STATE": str(self.state), "FIXTURE_EVENTS": str(self.events),
      "FIXTURE_REVISION": self.revision, "DOCKER_CONFIG": str(self.original),
      "FIXTURE_INDEX": self.index, "FIXTURE_AMD64": self.amd64, "FIXTURE_ARM64": self.arm64,
      "FIXTURE_ATTESTATION": self.attestation, "FIXTURE_ARCHIVE": str(self.archive),
      "ORIGINAL_DOCKER_CONFIG": str(self.original),
      "EXPECTED_BUILDX_CONFIG": str(self.original / "buildx"),
      "DOCKER_CONTEXT": "remote-engine", "DOCKER_HOST": "tcp://fixture-engine:2376",
    })

  def git_run(self, *args):
    return subprocess.run([self.git, *args], cwd=self.root, text=True, capture_output=True, check=True)

  def invoke(self, scenario=None, cwd=None, env=None, args=(), resume=False):
    state = json.loads(self.state.read_text()) if resume else {}
    state["scenario"] = scenario or {}
    self.state.write_text(json.dumps(state))
    self.events.write_text("")
    current_env = self.env | (env or {})
    result = subprocess.run(
      [str(self.bin / "bash"), str(self.root / "scripts/release.sh"), *args],
      cwd=cwd or self.root.parent, env=current_env, text=True, capture_output=True,
      timeout=30,
    )
    self.calls = [json.loads(line) for line in self.events.read_text().splitlines()]
    self.assertNotIn(TOKEN, result.stdout + result.stderr)
    self.assertEqual((self.original / "config.json").read_text(), self.original_config)
    for call in self.calls:
      self.assertNotIn(TOKEN, " ".join(call["args"]))
      self.assertIsNone(call["token_env"])
      isolated = call.get("docker_config")
      if isolated and isolated != str(self.original):
        self.assertFalse(Path(isolated).exists(), "Temporary registry credentials were not removed")
    return result

  def builds(self):
    return [call for call in self.calls if call["args"][:2] == ["buildx", "build"]]

  def test_publish_from_parent_server_and_unrelated_directories(self):
    for cwd in (self.root.parent, self.root, self.work):
      with self.subTest(cwd=cwd):
        result = self.invoke(cwd=cwd)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Published git.lan/ntr/ntr-server:0.2.2", result.stdout)
        self.assertIn(self.index, result.stdout)
        self.assertIn(self.amd64, result.stdout)
        self.assertIn(self.arm64, result.stdout)
        self.assertEqual(len(self.builds()), 1)
        self.assertEqual(self.builds()[0]["args"][self.builds()[0]["args"].index("--progress") + 1], "plain")
        self.assertIn("Registry login succeeded", result.stderr)
        self.assertIn("Tests passed; building and pushing linux/amd64, linux/arm64", result.stderr)
        self.assertIn("Release verified; removing temporary credentials", result.stderr)
        login = next(call for call in self.calls if call["args"][:1] == ["login"])
        self.assertEqual(login["stdin"], TOKEN + "\n")
        self.assertEqual(login["args"][login["args"].index("--username") + 1], "fixture-login")
        self.assertFalse(any(call["tool"] == "curl" and call["args"][-1].endswith("/user") for call in self.calls))
        configs = [call["args"][3] for call in self.calls if "{{json .Image}}" in call["args"]]
        self.assertEqual(configs, ["git.lan/ntr/ntr-server@" + self.amd64, "git.lan/ntr/ntr-server@" + self.arm64])

  def test_explicit_username_registry_override_and_buildx_state(self):
    state = self.work / "custom-buildx-state"
    result = self.invoke(env={
      "FORGEJO_USERNAME": "publisher", "FORGEJO_REGISTRY": "forgejo.invalid:3000",
      "FORGEJO_IMAGE": "forgejo.invalid:3000/team/server", "BUILDX_BUILDER": "other-builder",
      "BUILDX_CONFIG": str(state), "EXPECTED_BUILDX_CONFIG": str(state),
    })
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertFalse(any(call["tool"] == "curl" and call["args"][-1].endswith("/user") for call in self.calls))
    login = next(call for call in self.calls if call["args"][:1] == ["login"])
    self.assertEqual(login["args"][login["args"].index("--username") + 1], "publisher")
    self.assertIn("forgejo.invalid:3000/team/server:0.2.2", result.stdout)
    exports = [call["args"][2] for call in self.calls if call["args"][:2] == ["context", "export"]]
    self.assertEqual(exports, ["arm-node", "remote-engine"])
    self.assertTrue(all(call["docker_host"] == "tcp://fixture-engine:2376" for call in self.calls))
    self.assertTrue(all(call["builder"] == "ntr-release" for call in self.calls if call["tool"] == "docker"))

  def test_existing_complete_index_is_reused_without_build_or_test(self):
    result = self.invoke({"existing": True})
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertIn("Reused", result.stdout)
    self.assertFalse(self.builds())
    self.assertFalse(any(call["args"] == ["test", "--locked"] for call in self.calls))
    self.assertIn("Existing release verified", result.stderr)

  def test_build_progress_is_redacted_and_visible_before_build_completion(self):
    gate = self.work / "finish-build"
    self.state.write_text(json.dumps({"scenario": {"live_progress": True}}))
    self.events.write_text("")
    output = ""
    with subprocess.Popen(
      [str(self.bin / "bash"), str(self.root / "scripts/release.sh")],
      cwd=self.root.parent, env=self.env | {"FIXTURE_BUILD_GATE": str(gate)},
      stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    ) as process:
      try:
        with selectors.DefaultSelector() as selector:
          selector.register(process.stdout, selectors.EVENT_READ)
          deadline = time.monotonic() + 8
          while "fixture build waiting" not in output:
            events = selector.select(max(0, deadline - time.monotonic()))
            self.assertTrue(events, "No live build progress arrived before completion")
            chunk = os.read(process.stdout.fileno(), 65536)
            self.assertTrue(chunk, "Publisher exited before live build progress")
            output += chunk.decode()
        self.assertIsNone(process.poll(), "Build completed before progress was observed")
        self.assertIn("fixture build stdout", output)
        self.assertIn("[redacted]", output)
        self.assertNotIn(TOKEN, output)
      finally:
        gate.write_text("continue")
        remaining, _ = process.communicate(timeout=30)
        output += remaining.decode()
    self.assertEqual(process.returncode, 0, output)
    self.assertIn("Published git.lan/ntr/ntr-server:0.2.2", output)
    self.assertNotIn(TOKEN, output)

  def test_relative_docker_and_buildx_paths_use_the_callers_directory(self):
    result = self.invoke(env={
      "DOCKER_CONFIG": "../original-docker", "BUILDX_CONFIG": "../relative-buildx-state",
      "EXPECTED_BUILDX_CONFIG": str(self.work / "relative-buildx-state"),
    })
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertIn("Published", result.stdout)

  def test_arm64_without_variant_is_accepted(self):
    result = self.invoke({"arm_without_variant": True})
    self.assertEqual(result.returncode, 0, result.stderr)

  def test_missing_builder_is_created_with_existing_buildkit_configuration(self):
    home = self.work / "home"
    config = home / ".config/buildkit/buildkitd.toml"
    config.parent.mkdir(parents=True)
    config.write_text('[registry."git.lan"]\nca = ["/fixture/ca.crt"]\n')
    result = self.invoke({"builder_missing": True}, env={"HOME": str(home)})
    self.assertEqual(result.returncode, 0, result.stderr)
    creates = [call for call in self.calls if call["args"][:2] == ["buildx", "create"]]
    self.assertEqual(len(creates), 1)
    self.assertEqual(creates[0]["args"][-2:], ["--buildkitd-config", str(config)])
    result = self.invoke(resume=True, env={"HOME": str(home)})
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertFalse(any(call["args"][:2] == ["buildx", "create"] for call in self.calls))

  def test_missing_builder_uses_defaults_and_creation_errors_prevent_build(self):
    result = self.invoke({"builder_missing": True}, env={"HOME": str(self.work)})
    self.assertEqual(result.returncode, 0, result.stderr)
    create = next(call for call in self.calls if call["args"][:2] == ["buildx", "create"])
    self.assertNotIn("--buildkitd-config", create["args"])
    for scenario, env in (({"builder_missing": True, "create_failure": True}, {}),
                          ({"builder_missing": True}, {"BUILDKIT_CONFIG": str(self.work / "missing.toml")})):
      with self.subTest(scenario=scenario, env=env):
        result = self.invoke(scenario, env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.builds())

  def test_release_uploads_multiplatform_archive_and_matching_checksum(self):
    ca = self.work / "custom ca.crt"
    ca.write_text("fixture CA")
    result = self.invoke(env={"FORGEJO_CA_FILE": str(ca)})
    self.assertEqual(result.returncode, 0, result.stderr)
    destination = self.root / "release/0.2.2"
    archive = destination / "img.tar.gz"
    expected = hashlib.sha256(archive.read_bytes()).hexdigest() + "  img.tar.gz\n"
    self.assertEqual((destination / "img.tar.gz.sha256").read_text(), expected)
    with tarfile.open(archive, "r:gz") as contents:
      self.assertIn("blobs/sha256/" + self.index.split(":", 1)[1], contents.getnames())
      self.assertIn("blobs/sha256/" + self.amd64.split(":", 1)[1], contents.getnames())
      self.assertIn("blobs/sha256/" + self.arm64.split(":", 1)[1], contents.getnames())
    state = json.loads(self.state.read_text())
    self.assertEqual(state["release"]["target_commitish"], self.revision)
    self.assertEqual([asset["name"] for asset in state["release"]["assets"]], ["img.tar.gz", "img.tar.gz.sha256"])
    export = next(call for call in self.calls if call["tool"] == "skopeo")
    self.assertIn("--src-cert-dir", export["args"])
    self.assertIn("--src-tls-verify=true", export["args"])
    for call in self.calls:
      if call["tool"] == "curl":
        self.assertEqual(call["args"][call["args"].index("--cacert") + 1], str(ca))
        self.assertIn(TOKEN, call["stdin"])

  def test_complete_release_rerun_verifies_downloads_without_build_export_or_upload(self):
    result = self.invoke()
    self.assertEqual(result.returncode, 0, result.stderr)
    result = self.invoke(resume=True)
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertFalse(self.builds())
    self.assertFalse(any(call["tool"] == "skopeo" for call in self.calls))
    self.assertFalse(any("--form" in call["args"] for call in self.calls))
    self.assertEqual(sum("/attachments/" in call["args"][-1] for call in self.calls), 2)

  def test_checksum_upload_failure_is_resumed_without_replacing_archive(self):
    result = self.invoke({"upload_failure": "img.tar.gz.sha256"})
    self.assertNotEqual(result.returncode, 0)
    result = self.invoke(resume=True)
    self.assertEqual(result.returncode, 0, result.stderr)
    uploads = [call for call in self.calls if "--form" in call["args"]]
    self.assertEqual(len(uploads), 1)
    self.assertTrue(uploads[0]["args"][-1].endswith("name=img.tar.gz.sha256"))
    self.assertFalse(self.builds())

  def test_archive_api_and_identity_failures_never_report_success(self):
    for scenario in ({"export_failure": True}, {"archive_corrupt": True}, {"api_denied": True},
                     {"release_conflict": True}, {"tag_conflict": True}, {"remote_revision_conflict": True},
                     {"upload_failure": "img.tar.gz"}):
      with self.subTest(scenario=scenario):
        result = self.invoke(scenario)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("Published", result.stdout)
        if scenario.get("api_denied") or scenario.get("remote_revision_conflict"):
          self.assertFalse(self.builds())
    (self.bin / "skopeo").unlink()
    result = self.invoke()
    self.assertIn("Required command is not installed: skopeo", result.stderr)
    self.assertFalse(self.builds())

  def test_rerun_rejects_tampered_checksum_and_untrusted_attachment_host(self):
    result = self.invoke()
    self.assertEqual(result.returncode, 0, result.stderr)
    result = self.invoke({"checksum_conflict": True}, resume=True)
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("checksum does not match", result.stderr)
    self.assertFalse(any("--form" in call["args"] for call in self.calls))
    state = json.loads(self.state.read_text())
    state["release"]["assets"][0]["browser_download_url"] = "https://untrusted.invalid/image"
    self.state.write_text(json.dumps(state))
    result = self.invoke(resume=True)
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("configured HTTPS API host", result.stderr)
    self.assertFalse(any(call["args"][-1].startswith("https://untrusted.invalid") for call in self.calls))

  def test_archive_missing_or_corrupted_layer_is_never_uploaded(self):
    original = self.archive.read_bytes()
    for corrupt in (False, True):
      with self.subTest(corrupt=corrupt):
        with tarfile.open(fileobj=io.BytesIO(original), mode="r:") as source, tarfile.open(self.archive, "w") as output:
          for member in source:
            content = source.extractfile(member).read()
            if content == b"fixture compressed layer contents":
              if not corrupt:
                continue
              content = b"changed compressed layer contents"
              member.size = len(content)
            output.addfile(member, io.BytesIO(content))
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing a referenced blob" if not corrupt else "corrupted blob", result.stderr)
        self.assertFalse(any("--form" in call["args"] for call in self.calls))

  def test_invalid_api_and_ca_settings_fail_before_build(self):
    for env in ({"FORGEJO_API_URL": "http://git.lan/api/v1/repos/ntr/ntr-server"},
                {"FORGEJO_API_URL": "https://name:password@git.lan/api/v1/repos/ntr/ntr-server"},
                {"FORGEJO_CA_FILE": str(self.work / "missing-ca.crt")}):
      with self.subTest(env=env):
        result = self.invoke(env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.builds())

  def test_help_and_python_preflight_do_not_require_credentials(self):
    result = self.invoke(env={"FORGEJO_TOKEN": ""}, args=("--help",))
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertIn("Usage:", result.stdout)
    self.assertFalse(self.calls)
    result = self.invoke(args=("unknown",))
    self.assertEqual(result.returncode, 2)
    (self.bin / "python3").unlink()
    result = self.invoke()
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("Required command is not installed: python3", result.stderr)

  def test_missing_credentials_tools_dirty_and_feature_checkout(self):
    result = self.invoke(env={"FORGEJO_TOKEN": ""})
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("FORGEJO_TOKEN", result.stderr)
    (self.bin / "docker").unlink()
    result = self.invoke()
    self.assertIn("Required command is not installed: docker", result.stderr)
    (self.bin / "docker").symlink_to(self.bin / "fake.py")
    (self.root / "untracked").write_text("dirty")
    result = self.invoke()
    self.assertIn("uncommitted changes", result.stderr)
    (self.root / "untracked").unlink()
    self.git_run("switch", "-qc", "feature")
    result = self.invoke()
    self.assertIn("main checkout", result.stderr)

  def test_invalid_configuration_fails_before_docker(self):
    for env in ({"FORGEJO_IMAGE": "git.lan/ntr/server:latest"},
                {"FORGEJO_IMAGE": "elsewhere.invalid/ntr/server"},
                {"FORGEJO_REGISTRY": "https://git.lan"},
                {"FORGEJO_TOKEN": "token\nheader"}):
      with self.subTest(env=env):
        result = self.invoke(env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.calls)

  def test_builder_login_and_build_failures_do_not_succeed(self):
    scenarios = (
      {"daemon_failure": True}, {"buildx_missing": True}, {"driver": "docker"},
      {"platform_missing": True}, {"builder_changed": True}, {"login_failure": True},
      {"test_failure": True}, {"build_failure": True}, {"push_failure": True},
      {"change_checkout": True}, {"version": "not-a-tag"},
    )
    for scenario in scenarios:
      with self.subTest(scenario=scenario):
        result = self.invoke(scenario)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertNotIn("Published", result.stdout)
        if not scenario.get("build_failure") and not scenario.get("push_failure"):
          self.assertFalse(self.builds())
        if scenario.get("change_checkout"):
          self.git_run("restore", "src/main.rs")

  def test_unknown_missing_diagnostics_and_registry_errors_prevent_build(self):
    for scenario in ({"lookup_error": "permission denied"},
                     {"lookup_error": "ERROR: unrelated context: not found"},
                     {"lookup_error": "connection refused"},
                     {"lookup_code": 2}):
      with self.subTest(scenario=scenario):
        result = self.invoke(scenario)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.builds())

  def test_missing_or_invalid_git_username_requires_an_explicit_override(self):
    for username in ("", "Display Name"):
      with self.subTest(username=username):
        self.git_run("config", "user.name", username)
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("git config user.name", result.stderr)
        self.assertFalse(any(call["args"][:1] == ["login"] for call in self.calls))
        self.assertFalse(self.builds())
        result = self.invoke(env={"FORGEJO_USERNAME": "publisher"})
        self.assertEqual(result.returncode, 0, result.stderr)

  def test_malformed_incomplete_or_conflicting_indexes_are_not_overwritten(self):
    for flag in ("manifest_json", "index_incomplete", "index_duplicate", "unmarked_attestation",
                 "descriptor_digest", "bad_variant", "config_architecture", "revision_conflict",
                 "version_conflict", "index_digest", "immutable_mismatch"):
      with self.subTest(flag=flag):
        result = self.invoke({"existing": True, flag: True})
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertFalse(self.builds())
    result = self.invoke({"existing": True, "index_media": "text/plain"})
    self.assertNotEqual(result.returncode, 0)
    self.assertFalse(self.builds())

  def test_successful_push_requires_metadata_and_remote_verification(self):
    for flag in ("metadata_missing", "metadata_wrong", "metadata_invalid", "tag_moved",
                 "index_incomplete", "config_architecture", "revision_conflict"):
      with self.subTest(flag=flag):
        result = self.invoke({flag: True})
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertEqual(len(self.builds()), 1)
        self.assertNotIn("Published", result.stdout)

  def test_termination_removes_temporary_credentials_and_reports_ambiguity(self):
    result = self.invoke({"terminate_build": True})
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("terminated; inspect the registry", result.stderr)
    self.assertNotIn("Published", result.stdout)


if __name__ == "__main__":
  unittest.main()

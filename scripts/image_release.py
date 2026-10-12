"""Publish one verified amd64/arm64 server index with isolated registry credentials.

The Bash entry point locates this module. Python's standard library handles JSON
and subprocess boundaries so registry decisions never depend on shell pipelines.
"""

import json
from datetime import datetime
import gzip
import hashlib
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import tarfile
from urllib.parse import urlsplit, quote


PLATFORMS = ("linux/amd64", "linux/arm64")
RELEASE_BUILDER = "ntr-release"
INDEX_TYPES = {
  "application/vnd.oci.image.index.v1+json",
  "application/vnd.docker.distribution.manifest.list.v2+json",
}
IMAGE_TYPES = {
  "application/vnd.oci.image.manifest.v1+json",
  "application/vnd.docker.distribution.manifest.v2+json",
}
DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
BUILDER_FORMAT = '{{if eq .Name .Builder.Name}}{"Current":{{json .Builder.Current}},"Builder":{{json .Builder}}}{{end}}'


class ReleaseError(Exception):
  pass


def object_json(text, description):
  try:
    value = json.loads(text)
  except (ValueError, TypeError) as error:
    raise ReleaseError(f"Invalid JSON for {description}") from error
  if not isinstance(value, dict):
    raise ReleaseError(f"Expected a JSON object for {description}")
  return value


def valid_digest(value):
  if not isinstance(value, str) or not DIGEST.fullmatch(value):
    raise ReleaseError("Missing or invalid SHA-256 image digest")
  return value


class Commands:
  def __init__(self, root, token):
    self.root = root
    self.token = token
    self.env = dict(os.environ)
    self.env.pop("FORGEJO_TOKEN", None)

  def run(self, *args, data=None, check=True, announce=False):
    if announce:
      self.log("Running " + " ".join(args))
    if announce and data is None:
      result = self.stream(args)
    else:
      result = subprocess.run(
        args, cwd=self.root, env=self.env, input=data,
        text=True, capture_output=True,
      )
      if announce:
        for output in (result.stdout, result.stderr):
          if output:
            print(output.replace(self.token, "[redacted]"), end="", file=sys.stderr, flush=True)
    if check and result.returncode:
      detail = (result.stderr or result.stdout).strip().replace(self.token, "[redacted]")
      raise ReleaseError(f"{args[0]} {args[1] if len(args) > 1 else ''} failed: {detail}")
    return result

  def log(self, message):
    print(f"[release {datetime.now():%H:%M:%S}] {message}".replace(self.token, "[redacted]"), file=sys.stderr, flush=True)

  def stream(self, args):
    # Merge pipes so neither stream can block the child while the other is read.
    # Redact complete lines, including tokens split across separate child writes.
    with subprocess.Popen(
      args, cwd=self.root, env=self.env, stdin=subprocess.DEVNULL,
      stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1,
    ) as process:
      output = []
      try:
        for line in process.stdout:
          redacted = line.replace(self.token, "[redacted]")
          print(redacted, end="", file=sys.stderr, flush=True)
          output.append(redacted)
        return subprocess.CompletedProcess(args, process.wait(), "".join(output), "")
      except BaseException:
        process.kill()
        process.wait()
        raise


def configuration():
  token = os.environ.get("FORGEJO_TOKEN", "")
  if not token or any(ord(c) < 32 or ord(c) == 127 for c in token):
    raise ReleaseError("Set FORGEJO_TOKEN to a nonempty Forgejo package/repository-write token")
  registry = os.environ.get("FORGEJO_REGISTRY", "git.lan")
  if not re.fullmatch(r"[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?(?::[0-9]+)?", registry):
    raise ReleaseError("FORGEJO_REGISTRY must be a lowercase host[:port], without a URL scheme")
  image = os.environ.get("FORGEJO_IMAGE", f"{registry}/ntr/ntr-server")
  components = image.split("/")
  if len(components) < 3 or components[0] != registry or not all(
    re.fullmatch(r"[a-z0-9]+(?:[._-][a-z0-9]+)*", part) for part in components[1:]
  ):
    raise ReleaseError("FORGEJO_IMAGE must be registry/owner/image, without a tag or digest")
  return token, registry, image


def checkout_identity(commands):
  if commands.run("git", "branch", "--show-current").stdout.strip() != "main":
    raise ReleaseError("Image releases must run from a clean server main checkout")
  if commands.run("git", "status", "--porcelain", "--untracked-files=all").stdout.strip():
    raise ReleaseError("The server working tree has uncommitted changes; refusing to release")
  revision = commands.run("git", "rev-parse", "HEAD").stdout.strip()
  if not re.fullmatch(r"[0-9a-f]{40,64}", revision):
    raise ReleaseError("Invalid server Git revision")
  metadata = object_json(commands.run(
    "cargo", "metadata", "--locked", "--no-deps", "--format-version", "1",
  ).stdout, "Cargo metadata")
  packages = metadata.get("packages", [])
  if not isinstance(packages, list):
    raise ReleaseError("Invalid Cargo package list")
  matches = [p for p in packages if isinstance(p, dict) and p.get("name") == "my-ntr-server"]
  version = matches[0].get("version") if len(matches) == 1 else None
  if not isinstance(version, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[a-z0-9.-]+)?", version):
    raise ReleaseError("Cargo metadata must contain one my-ntr-server package with a valid version tag")
  return matches[0]["version"], revision


def login_name(commands):
  username = os.environ.get("FORGEJO_USERNAME", "")
  if not username:
    result = commands.run("git", "config", "--get", "user.name", check=False)
    if result.returncode not in (0, 1):
      raise ReleaseError("Could not read git config user.name; set FORGEJO_USERNAME")
    username = result.stdout.strip() if result.returncode == 0 else ""
  if not isinstance(username, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", username):
    raise ReleaseError("Missing or invalid Forgejo login name; set git config user.name to your Forgejo login or set FORGEJO_USERNAME")
  return username


def builder_info(commands, name, bootstrap=False, create=False):
  if bootstrap:
    commands.run("docker", "buildx", "inspect", name, "--bootstrap", announce=True)
  # `inspect` has no --format; explicitly include Current because older Buildx
  # versions omit it from the embedded Builder object's JSON representation.
  output = commands.run("docker", "buildx", "ls", "--format", BUILDER_FORMAT).stdout
  builders = [object_json(line, "Buildx builder") for line in output.splitlines() if line.strip()]
  matches = [item for item in builders if isinstance(item.get("Builder"), dict) and item["Builder"].get("Name") == name]
  # A node may share its builder's name, producing the same wrapper twice.
  matches = list({json.dumps(item, sort_keys=True): item for item in matches}.values())
  if not matches and create:
    config = Path(os.environ.get("BUILDKIT_CONFIG", "~/.config/buildkit/buildkitd.toml")).expanduser().resolve()
    if "BUILDKIT_CONFIG" in os.environ and not config.is_file():
      raise ReleaseError(f"BuildKit configuration does not exist: {config}")
    args = ["docker", "buildx", "create", "--name", name, "--driver", "docker-container"]
    if config.is_file():
      args.extend(["--buildkitd-config", str(config)])
    commands.log(f"Creating builder {name}" + (f" using {config}" if config.is_file() else " with default configuration"))
    commands.run(*args, announce=True)
    return builder_info(commands, name)
  if len(matches) != 1:
    raise ReleaseError(f"Could not identify builder {name}; create it with docker buildx create --name {name} --driver docker-container")
  info = matches[0].get("Builder")
  if not isinstance(info, dict):
    raise ReleaseError("Invalid Buildx builder object")
  if info.get("Driver") != "docker-container":
    raise ReleaseError(f"Builder {name} must use the docker-container driver; other drivers are unsupported")
  if not isinstance(info.get("Name"), str) or not info["Name"]:
    raise ReleaseError("Buildx did not return a builder name")
  return info


def isolate_docker(commands, work):
  original = Path(commands.env.get("DOCKER_CONFIG") or Path.home() / ".docker").resolve()
  state = Path(commands.env.get("BUILDX_CONFIG") or original / "buildx").resolve()
  # Subprocesses run in the server root, but relative connection paths belong
  # to the caller's directory. Normalize them before invoking Docker at all.
  commands.env["DOCKER_CONFIG"] = str(original)
  commands.env["BUILDX_CONFIG"] = str(state)
  commands.env["BUILDX_BUILDER"] = RELEASE_BUILDER
  if commands.env.get("DOCKER_CERT_PATH"):
    commands.env["DOCKER_CERT_PATH"] = str(Path(commands.env["DOCKER_CERT_PATH"]).resolve())
  config_file = original / "config.json"
  config = object_json(config_file.read_text(), "Docker configuration") if config_file.exists() else {}
  commands.log(f"Checking Docker engine and builder {RELEASE_BUILDER}")
  commands.run("docker", "buildx", "version")
  commands.run("docker", "info")
  selected = builder_info(commands, RELEASE_BUILDER, create=True)
  name = selected["Name"]
  context = commands.run("docker", "context", "show").stdout.strip()
  contexts = {context}
  nodes = selected.get("Nodes")
  if not isinstance(nodes, list) or not nodes:
    raise ReleaseError("The selected builder has no nodes")
  for node in nodes:
    endpoint = node.get("Endpoint", "") if isinstance(node, dict) else ""
    if not endpoint:
      raise ReleaseError("Builder node has no endpoint")
    if "://" not in endpoint:
      contexts.add(endpoint)
  exports = []
  for index, item in enumerate(sorted(contexts - {"default"})):
    commands.log(f"Preserving Docker context {item}")
    archive = work / f"context-{index}.dockercontext"
    commands.run("docker", "context", "export", item, str(archive))
    exports.append((item, archive))
  isolated = work / "docker"
  isolated.mkdir(mode=0o700)
  plugin_dirs = config.get("cliPluginsExtraDirs", [])
  if not isinstance(plugin_dirs, list) or not all(isinstance(p, str) for p in plugin_dirs):
    raise ReleaseError("Invalid Docker cliPluginsExtraDirs")
  proxies = config.get("proxies", {})
  if not isinstance(proxies, dict):
    raise ReleaseError("Invalid Docker proxy configuration")
  new_config = {
    "cliPluginsExtraDirs": [str(original / "cli-plugins"), *plugin_dirs],
    "currentContext": "default",
    "proxies": proxies,
    # A nonempty auth map prevents Docker from autodetecting a persistent native
    # credential helper. The placeholder contains no credentials.
    "auths": {"unused.invalid": {}},
  }
  isolated_config = isolated / "config.json"
  isolated_config.write_text(json.dumps(new_config))
  isolated_config.chmod(0o600)
  commands.env["BUILDX_CONFIG"] = str(state)
  commands.env["BUILDX_BUILDER"] = name
  commands.env["DOCKER_CONFIG"] = str(isolated)
  context_override = commands.env.pop("DOCKER_CONTEXT", None)
  for item, archive in exports:
    commands.run("docker", "context", "import", item, str(archive))
  new_config["currentContext"] = context
  isolated_config.write_text(json.dumps(new_config))
  if context_override is not None:
    commands.env["DOCKER_CONTEXT"] = context_override
  commands.run("docker", "buildx", "version")
  commands.run("docker", "info")
  commands.log(f"Bootstrapping and verifying isolated builder {name}")
  verified = builder_info(commands, name, bootstrap=True)
  if verified["Name"] != name:
    raise ReleaseError("Credential isolation changed the selected builder")
  available = set()
  verified_nodes = verified.get("Nodes")
  if not isinstance(verified_nodes, list) or not verified_nodes:
    raise ReleaseError("The isolated builder has no nodes")
  if [(n.get("Name"), n.get("Endpoint")) for n in verified_nodes if isinstance(n, dict)] != [
    (n.get("Name"), n.get("Endpoint")) for n in nodes
  ]:
    raise ReleaseError("Credential isolation changed builder node endpoints")
  for node in verified_nodes:
    if not isinstance(node, dict) or node.get("Status") != "running":
      raise ReleaseError("Every selected builder node must be running")
    platforms = node.get("Platforms", [])
    if not isinstance(platforms, list) or not all(isinstance(p, str) for p in platforms):
      raise ReleaseError("Invalid builder platform list")
    available.update(p.rstrip("*") for p in platforms)
  available = {p.strip() for p in available}
  if "linux/arm64/v8" in available:
    available.add("linux/arm64")
  if not set(PLATFORMS).issubset(available):
    raise ReleaseError("The selected builder must support both linux/amd64 and linux/arm64")
  commands.log(f"Builder {name} ready for {', '.join(PLATFORMS)}")
  return name


class Registry:
  def __init__(self, commands, image, version, revision):
    self.commands = commands
    self.image = image
    self.version = version
    self.revision = revision

  def manifest(self, reference, allow_missing=False):
    self.commands.log(f"Inspecting registry manifest {reference}")
    result = self.commands.run(
      "docker", "buildx", "imagetools", "inspect", reference,
      "--format", "{{json .Manifest}}", check=False,
    )
    if result.returncode:
      # Buildx's resolver prints this exact diagnostic for a missing manifest.
      # Never accept a generic substring or classify an auth/transport error as absence.
      missing = {f"ERROR: {reference}: not found", f"{reference}: not found"}
      if allow_missing and result.returncode == 1 and not result.stdout.strip() and result.stderr.strip() in missing:
        return None
      detail = result.stderr.strip().replace(self.commands.token, "[redacted]")
      raise ReleaseError(f"Registry lookup failed for {reference}: {detail}")
    return object_json(result.stdout, "registry manifest")

  def resolve(self, reference, allow_missing=False):
    manifest = self.manifest(reference, allow_missing)
    if manifest is None:
      return None
    digest = valid_digest(manifest.get("digest"))
    if "@" in reference and reference.rsplit("@", 1)[1] != digest:
      raise ReleaseError("Registry returned a different digest for an immutable reference")
    return digest

  def inspect(self, reference, allow_missing=False):
    digest = self.resolve(reference, allow_missing)
    if digest is None:
      return None
    manifest = self.manifest(f"{self.image}@{digest}")
    if manifest.get("mediaType") not in INDEX_TYPES or manifest.get("digest") != digest:
      raise ReleaseError("Expected a digest-matched multi-platform image index")
    descriptors = manifest.get("manifests")
    if not isinstance(descriptors, list):
      raise ReleaseError("Image index has no manifest descriptors")
    platforms = {}
    for descriptor in descriptors:
      if not isinstance(descriptor, dict):
        raise ReleaseError("Invalid image index descriptor")
      annotations = descriptor.get("annotations") or {}
      if not isinstance(annotations, dict):
        raise ReleaseError("Invalid manifest annotations")
      if annotations.get("vnd.docker.reference.type") == "attestation-manifest":
        continue
      platform = descriptor.get("platform") or {}
      architecture = platform.get("architecture") if isinstance(platform, dict) else None
      key = f"{platform.get('os')}/{architecture}" if isinstance(platform, dict) else ""
      if descriptor.get("mediaType") not in IMAGE_TYPES or key not in PLATFORMS:
        raise ReleaseError("Unexpected runnable platform or manifest media type in image index")
      variant = platform.get("variant")
      if variant not in (None, "") and not (architecture == "arm64" and variant == "v8"):
        raise ReleaseError("Unsupported platform variant in image index")
      if key in platforms:
        raise ReleaseError(f"Duplicate runnable platform: {key}")
      platforms[key] = valid_digest(descriptor.get("digest"))
    if set(platforms) != set(PLATFORMS):
      raise ReleaseError("Image index must contain both linux/amd64 and linux/arm64")
    for platform, child in platforms.items():
      self.validate_config(platform, child)
    return digest, platforms

  def validate_config(self, platform, child):
    self.commands.log(f"Verifying {platform} configuration at {child}")
    config = object_json(self.commands.run(
      "docker", "buildx", "imagetools", "inspect", f"{self.image}@{child}",
      "--format", "{{json .Image}}",
    ).stdout, "platform image configuration")
    if f"{config.get('os')}/{config.get('architecture')}" != platform:
      raise ReleaseError(f"Image configuration does not match {platform}")
    runtime = config.get("config") or {}
    labels = runtime.get("Labels") if isinstance(runtime, dict) else None
    if not isinstance(labels, dict) or labels.get("org.opencontainers.image.version") != self.version or labels.get("org.opencontainers.image.revision") != self.revision:
      raise ReleaseError(f"Existing {platform} image version/revision conflicts with this checkout; use a new server version")


def report(reference, verified, version, revision, reused=False):
  digest, platforms = verified
  print(f"{'Reused' if reused else 'Published'} {reference}")
  print(f"Index: {reference.rsplit(':', 1)[0]}@{digest}")
  for platform in PLATFORMS:
    print(f"{platform}: {platforms[platform]}")
  print(f"Version: {version}\nRevision: {revision}")


def file_sha256(path):
  digest = hashlib.sha256()
  with path.open("rb") as source:
    for chunk in iter(lambda: source.read(1024 * 1024), b""):
      digest.update(chunk)
  return digest.hexdigest()


def verify_archive(path, digest, platforms):
  # Inspect rather than extract untrusted archive paths. Verify every blob and
  # require the exact published index plus its complete descriptor graph.
  blobs = set()
  manifests = {}
  with tarfile.open(path, "r|gz") as archive:
    index = None
    for member in archive:
      if not member.isfile():
        continue
      name = member.name.removeprefix("./")
      if name == "index.json":
        index = object_json(archive.extractfile(member).read(), "OCI archive index")
      elif re.fullmatch(r"blobs/sha256/[0-9a-f]{64}", name):
        source = archive.extractfile(member)
        actual = hashlib.sha256()
        content = bytearray() if member.size <= 4 * 1024 * 1024 else None
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
          actual.update(chunk)
          if content is not None:
            content.extend(chunk)
        expected = "sha256:" + name.rsplit("/", 1)[1]
        if actual.hexdigest() != expected.split(":", 1)[1]:
          raise ReleaseError("OCI archive contains a corrupted blob")
        blobs.add(expected)
        if content is not None:
          try:
            value = json.loads(content)
            if isinstance(value, dict) and ("manifests" in value or "config" in value and "layers" in value):
              manifests[expected] = value
          except (ValueError, UnicodeError):
            pass
  entries = index.get("manifests", []) if isinstance(index, dict) else []
  if not isinstance(entries, list) or not any(isinstance(item, dict) and item.get("digest") == digest for item in entries):
    raise ReleaseError("OCI archive does not reference the published index digest")
  required = [digest]
  visited = set()
  while required:
    current = required.pop()
    if current in visited:
      continue
    visited.add(current)
    if current not in blobs:
      raise ReleaseError("OCI archive is missing a referenced blob")
    manifest = manifests.get(current)
    if manifest:
      if "manifests" in manifest:
        children = manifest["manifests"]
      else:
        layers = manifest.get("layers")
        if not isinstance(layers, list):
          raise ReleaseError("OCI archive contains invalid image layers")
        children = [manifest.get("config"), *layers]
      if not isinstance(children, list) or not all(isinstance(item, dict) for item in children):
        raise ReleaseError("OCI archive contains invalid manifest descriptors")
      required.extend(valid_digest(item.get("digest")) for item in children)
  if digest not in manifests or not set(platforms.values()).issubset(manifests):
    raise ReleaseError("OCI archive is missing required platform manifests")


class Forgejo:
  def __init__(self, commands, host, work):
    self.commands = commands
    self.work = work
    self.api = os.environ.get("FORGEJO_API_URL", f"https://{host}/api/v1/repos/ntr/ntr-server").rstrip("/")
    parsed = urlsplit(self.api)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment:
      raise ReleaseError("FORGEJO_API_URL must be an HTTPS repository API URL without credentials or query")
    self.ca = os.environ.get("FORGEJO_CA_FILE", "")
    if self.ca:
      self.ca = str(Path(self.ca).expanduser().resolve())
      if not Path(self.ca).is_file():
        raise ReleaseError(f"Forgejo CA certificate does not exist: {self.ca}")

  def fetch(self, url, output, *args):
    parsed = urlsplit(url)
    if parsed.scheme != "https" or parsed.netloc != urlsplit(self.api).netloc or parsed.username or parsed.password:
      raise ReleaseError("Forgejo download URL must use the configured HTTPS API host")
    escaped = self.commands.token.replace("\\", "\\\\").replace('"', '\\"')
    result = self.commands.run(
      "curl", "--silent", "--show-error", "--proto", "=https",
      "--connect-timeout", "15", "--max-time", "1800", "--config", "-",
      *(["--cacert", self.ca] if self.ca else []),
      "--output", str(output), "--write-out", "%{http_code}", *args, url,
      data=f'header = "Authorization: token {escaped}"\n',
    )
    return result.stdout.strip()

  def request(self, path, payload=None, attachment=None, missing=False):
    args = []
    if payload is not None:
      data = self.work / "request.json"
      data.write_text(json.dumps(payload))
      args = ["--request", "POST", "--header", "Content-Type: application/json", "--data-binary", f"@{data}"]
    elif attachment is not None:
      args = ["--form", f'attachment=@"{attachment}"']
    output = self.work / "response.json"
    status = self.fetch(self.api + path, output, *args)
    if missing and status == "404":
      return None
    if status != ("201" if payload is not None or attachment is not None else "200"):
      raise ReleaseError(f"Forgejo request {path} failed with HTTP {status}")
    return object_json(output.read_text(), "Forgejo response")

  def ensure_release(self, version, revision, image, digest):
    tag = f"v{version}"
    identity = f"Image: `{image}@{digest}`\nRevision: `{revision}`"
    self.commands.log(f"Looking up Forgejo release {tag}")
    release = self.request(f"/releases/tags/{quote(tag, safe='')}", missing=True)
    if release is None:
      commit = self.request(f"/git/commits/{quote('refs/tags/' + tag, safe='')}", missing=True)
      if commit is not None and commit.get("sha") != revision:
        raise ReleaseError("Existing Forgejo Git tag conflicts with this revision")
      release = self.request("/releases", {
        "tag_name": tag, "target_commitish": revision, "name": tag,
        "body": identity, "draft": False, "prerelease": "-" in version,
      })
    if release.get("tag_name") != tag or release.get("body") != identity or type(release.get("id")) is not int or release["id"] <= 0 or release.get("draft") is True:
      raise ReleaseError("Existing Forgejo release identity conflicts with this image/revision")
    if self.request(f"/git/commits/{quote('refs/tags/' + tag, safe='')}").get("sha") != revision:
      raise ReleaseError("Forgejo release tag does not identify the expected revision")
    return release


def release_artifacts(commands, forgejo, work, image, version, revision, verified):
  digest, platforms = verified
  release = forgejo.ensure_release(version, revision, image, digest)
  assets = release.get("assets", [])
  if not isinstance(assets, list):
    raise ReleaseError("Invalid Forgejo release assets")
  wanted = ("img.tar.gz", "img.tar.gz.sha256")
  existing = {}
  for asset in assets:
    if isinstance(asset, dict) and asset.get("name") in wanted:
      if asset["name"] in existing:
        raise ReleaseError("Duplicate Forgejo image release attachments")
      existing[asset["name"]] = asset
  archive = work / wanted[0]
  checksum = work / wanted[1]
  destination = commands.root / "release" / version
  local = destination / archive.name
  if archive.name in existing:
    commands.log("Downloading existing release archive for integrity verification")
    if forgejo.fetch(existing[archive.name].get("browser_download_url", ""), archive) != "200":
      raise ReleaseError("Could not download existing image archive")
  elif checksum.name in existing:
    raise ReleaseError("Release has a checksum without its archive; resolve the incomplete release before retrying")
  elif local.is_file():
    commands.log(f"Reusing local archive {local}")
    shutil.copyfile(local, archive)
  else:
    commands.log(f"Exporting both architectures from {image}@{digest}")
    raw = work / "img.tar"
    cert_args = []
    if forgejo.ca:
      certificates = work / "registry-certs"
      certificates.mkdir()
      shutil.copyfile(forgejo.ca, certificates / "ca.crt")
      cert_args = ["--src-cert-dir", str(certificates)]
    commands.run(
      "skopeo", "copy", "--all", "--preserve-digests", "--src-authfile",
      str(Path(commands.env["DOCKER_CONFIG"]) / "config.json"), "--src-tls-verify=true", *cert_args,
      f"docker://{image}@{digest}", f"oci-archive:{raw}", announce=True,
    )
    commands.log("Compressing OCI archive as img.tar.gz")
    with raw.open("rb") as source, archive.open("wb") as output:
      with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
        shutil.copyfileobj(source, compressed)
  commands.log("Verifying OCI archive blobs and generating SHA-256 checksum")
  verify_archive(archive, digest, platforms)
  expected_checksum = f"{file_sha256(archive)}  {archive.name}\n"
  if checksum.name in existing:
    if forgejo.fetch(existing[checksum.name].get("browser_download_url", ""), checksum) != "200" or checksum.read_text() != expected_checksum:
      raise ReleaseError("Existing release checksum does not match its image archive")
  checksum.write_text(expected_checksum)
  destination.mkdir(parents=True, exist_ok=True)
  for path in (archive, checksum):
    # Copy on the destination filesystem so replacement is atomic across /tmp mounts.
    with tempfile.NamedTemporaryFile(dir=destination, delete=False) as output:
      staged = Path(output.name)
      try:
        with path.open("rb") as source:
          shutil.copyfileobj(source, output)
        output.close()
        staged.replace(destination / path.name)
      finally:
        staged.unlink(missing_ok=True)
    if path.name not in existing:
      commands.log(f"Uploading Forgejo attachment {path.name}")
      asset = forgejo.request(f"/releases/{release['id']}/assets?name={quote(path.name)}", attachment=path)
      if asset.get("name") != path.name:
        raise ReleaseError("Forgejo returned a different attachment name")
    else:
      commands.log(f"Verified existing attachment {path.name}")
  print(f"Artifacts: {destination}")


def release(root):
  token, host, image = configuration()
  for command in ("git", "cargo", "docker", "curl", "skopeo"):
    if not shutil.which(command):
      raise ReleaseError(f"Required command is not installed: {command}")
  commands = Commands(root, token)
  commands.log("Checking clean server main checkout and Cargo version")
  version, revision = checkout_identity(commands)
  reference = f"{image}:{version}"
  commands.log(f"Release target {reference}; revision {revision}")
  with tempfile.TemporaryDirectory(prefix="ntr-image-release-") as directory:
    work = Path(directory)
    forgejo = Forgejo(commands, host, work)
    commands.log("Checking Forgejo repository access and committed source revision")
    if forgejo.request(f"/git/commits/{revision}").get("sha") != revision:
      raise ReleaseError("Forgejo repository does not contain the expected revision; push server main before releasing")
    builder = isolate_docker(commands, work)
    username = login_name(commands)
    commands.log(f"Logging in to {host} as {username}")
    commands.run("docker", "login", host, "--username", username, "--password-stdin", data=token + "\n")
    commands.log("Registry login succeeded; checking existing version tag")
    registry = Registry(commands, image, version, revision)
    existing = registry.inspect(reference, allow_missing=True)
    if existing:
      if registry.resolve(reference) != existing[0]:
        raise ReleaseError("The version tag moved while inspecting the existing release")
      release_artifacts(commands, forgejo, work, image, version, revision, existing)
      report(reference, existing, version, revision, reused=True)
      commands.log("Existing release verified; removing temporary credentials")
      return
    commands.log("Version tag is absent; running locked Cargo tests")
    commands.run("cargo", "test", "--locked", announce=True)
    # Recheck after tests: a long-running test must not label changed source as the old revision.
    if checkout_identity(commands) != (version, revision):
      raise ReleaseError("The checkout changed before the image build")
    commands.log(f"Tests passed; building and pushing {', '.join(PLATFORMS)}")
    metadata = work / "build.json"
    commands.run(
      "docker", "buildx", "build", "--builder", builder,
      "--platform", ",".join(PLATFORMS), "--progress", "plain", "--push", "--tag", reference,
      "--file", str(root / "Dockerfile"), "--metadata-file", str(metadata),
      "--build-arg", f"VERSION={version}", "--build-arg", f"REVISION={revision}",
      "--build-arg", "SOURCE=https://git.lan/ntr/ntr-server", str(root), announce=True,
    )
    if not metadata.is_file():
      raise ReleaseError("Buildx did not produce build metadata; publication outcome must be inspected")
    expected = valid_digest(object_json(metadata.read_text(), "build metadata").get("containerimage.digest"))
    commands.log(f"Build/push completed; verifying index {expected}")
    published = registry.inspect(f"{image}@{expected}")
    if registry.resolve(reference) != expected:
      raise ReleaseError("The version tag does not match the built index digest")
    release_artifacts(commands, forgejo, work, image, version, revision, published)
    report(reference, published, version, revision)
    commands.log("Release verified; removing temporary credentials")


def main():
  if len(sys.argv) > 1:
    print("Usage: scripts/release.sh\nPublish linux/amd64 and linux/arm64 plus Forgejo image archive attachments using FORGEJO_TOKEN.")
    return 0 if sys.argv[1:] == ["--help"] else 2
  try:
    release(Path(__file__).resolve().parent.parent)
    return 0
  except (ReleaseError, OSError, ValueError, EOFError, tarfile.TarError) as error:
    token = os.environ.get("FORGEJO_TOKEN", "")
    message = str(error)
    if token:
      message = message.replace(token, "[redacted]")
    print(f"Release failed: {message}", file=sys.stderr)
    return 1
  except KeyboardInterrupt:
    print("Release interrupted; inspect the registry before retrying", file=sys.stderr)
    return 130


def interrupted(signum, frame):
  raise ReleaseError("Release terminated; inspect the registry before retrying")


if __name__ == "__main__":
  signal.signal(signal.SIGTERM, interrupted)
  sys.exit(main())

"""Publish one verified amd64/arm64 server index with isolated registry credentials.

The Bash entry point locates this module. Python's standard library handles JSON
and subprocess boundaries so registry decisions never depend on shell pipelines.
"""

import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
from urllib.parse import urlsplit


PLATFORMS = ("linux/amd64", "linux/arm64")
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
      print("Running " + " ".join(args), flush=True)
    result = subprocess.run(
      args, cwd=self.root, env=self.env, input=data,
      text=True, capture_output=True,
    )
    if check and result.returncode:
      detail = (result.stderr or result.stdout).strip().replace(self.token, "[redacted]")
      raise ReleaseError(f"{args[0]} {args[1] if len(args) > 1 else ''} failed: {detail}")
    if announce and result.stdout:
      print(result.stdout.replace(self.token, "[redacted]"), end="", flush=True)
    if announce and result.stderr:
      print(result.stderr.replace(self.token, "[redacted]"), end="", file=sys.stderr)
    return result


def configuration():
  token = os.environ.get("FORGEJO_TOKEN", "")
  if not token or any(ord(c) < 32 or ord(c) == 127 for c in token):
    raise ReleaseError("Set FORGEJO_TOKEN to a nonempty Forgejo package-write token")
  registry = os.environ.get("FORGEJO_REGISTRY", "git.lan")
  if not re.fullmatch(r"[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?(?::[0-9]+)?", registry):
    raise ReleaseError("FORGEJO_REGISTRY must be a lowercase host[:port], without a URL scheme")
  image = os.environ.get("FORGEJO_IMAGE", f"{registry}/ntr/ntr-server")
  components = image.split("/")
  if len(components) < 3 or components[0] != registry or not all(
    re.fullmatch(r"[a-z0-9]+(?:[._-][a-z0-9]+)*", part) for part in components[1:]
  ):
    raise ReleaseError("FORGEJO_IMAGE must be registry/owner/image, without a tag or digest")
  api = os.environ.get("FORGEJO_API_URL", f"https://{registry}/api/v1").rstrip("/")
  parsed = urlsplit(api)
  if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment:
    raise ReleaseError("FORGEJO_API_URL must be an HTTPS API base URL without credentials or query")
  return token, registry, image, api


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


def login_name(commands, api, work):
  username = os.environ.get("FORGEJO_USERNAME", "")
  if not username:
    response = work / "user.json"
    escaped = commands.token.replace("\\", "\\\\").replace('"', '\\"')
    status = commands.run(
      "curl", "--silent", "--show-error", "--proto", "=https",
      "--connect-timeout", "15", "--max-time", "60", "--config", "-",
      "--output", str(response), "--write-out", "%{http_code}", f"{api}/user",
      data=f'header = "Authorization: token {escaped}"\n',
    ).stdout.strip()
    if status != "200":
      raise ReleaseError(f"Forgejo login lookup returned HTTP {status}; supply FORGEJO_USERNAME or a token with read:user")
    username = object_json(response.read_text(), "Forgejo user").get("login", "")
  if not isinstance(username, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", username):
    raise ReleaseError("Missing or invalid Forgejo login name; set FORGEJO_USERNAME")
  return username


def builder_info(commands, name=None, bootstrap=False):
  if bootstrap:
    commands.run("docker", "buildx", "inspect", name, "--bootstrap")
  # `inspect` has no --format; explicitly include Current because older Buildx
  # versions omit it from the embedded Builder object's JSON representation.
  output = commands.run("docker", "buildx", "ls", "--format", BUILDER_FORMAT).stdout
  builders = [object_json(line, "Buildx builder") for line in output.splitlines() if line.strip()]
  matches = [item for item in builders if isinstance(item.get("Builder"), dict) and item["Builder"].get("Name") == name] if name else [
    item for item in builders if item.get("Current") is True
  ]
  # A node may share its builder's name, producing the same wrapper twice.
  matches = list({json.dumps(item, sort_keys=True): item for item in matches}.values())
  if len(matches) != 1:
    raise ReleaseError("Could not identify one selected builder; set BUILDX_BUILDER")
  info = matches[0].get("Builder")
  if not isinstance(info, dict):
    raise ReleaseError("Invalid Buildx builder object")
  if info.get("Driver") != "docker-container":
    raise ReleaseError("Select an existing docker-container builder with BUILDX_BUILDER; other drivers are unsupported")
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
  if commands.env.get("DOCKER_CERT_PATH"):
    commands.env["DOCKER_CERT_PATH"] = str(Path(commands.env["DOCKER_CERT_PATH"]).resolve())
  config_file = original / "config.json"
  config = object_json(config_file.read_text(), "Docker configuration") if config_file.exists() else {}
  commands.run("docker", "buildx", "version")
  commands.run("docker", "info")
  selected = builder_info(commands, commands.env.get("BUILDX_BUILDER"))
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
  return name


class Registry:
  def __init__(self, commands, image, version, revision):
    self.commands = commands
    self.image = image
    self.version = version
    self.revision = revision

  def manifest(self, reference, allow_missing=False):
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


def release(root):
  token, host, image, api = configuration()
  for command in ("git", "cargo", "curl", "docker"):
    if not shutil.which(command):
      raise ReleaseError(f"Required command is not installed: {command}")
  commands = Commands(root, token)
  version, revision = checkout_identity(commands)
  reference = f"{image}:{version}"
  with tempfile.TemporaryDirectory(prefix="ntr-image-release-") as directory:
    work = Path(directory)
    builder = isolate_docker(commands, work)
    username = login_name(commands, api, work)
    commands.run("docker", "login", host, "--username", username, "--password-stdin", data=token + "\n")
    registry = Registry(commands, image, version, revision)
    existing = registry.inspect(reference, allow_missing=True)
    if existing:
      if registry.resolve(reference) != existing[0]:
        raise ReleaseError("The version tag moved while inspecting the existing release")
      report(reference, existing, version, revision, reused=True)
      return
    commands.run("cargo", "test", "--locked", announce=True)
    # Recheck after tests: a long-running test must not label changed source as the old revision.
    if checkout_identity(commands) != (version, revision):
      raise ReleaseError("The checkout changed before the image build")
    metadata = work / "build.json"
    commands.run(
      "docker", "buildx", "build", "--builder", builder,
      "--platform", ",".join(PLATFORMS), "--push", "--tag", reference,
      "--file", str(root / "Dockerfile"), "--metadata-file", str(metadata),
      "--build-arg", f"VERSION={version}", "--build-arg", f"REVISION={revision}",
      "--build-arg", "SOURCE=https://git.lan/ntr/ntr-server", str(root), announce=True,
    )
    if not metadata.is_file():
      raise ReleaseError("Buildx did not produce build metadata; publication outcome must be inspected")
    expected = valid_digest(object_json(metadata.read_text(), "build metadata").get("containerimage.digest"))
    published = registry.inspect(f"{image}@{expected}")
    if registry.resolve(reference) != expected:
      raise ReleaseError("The version tag does not match the built index digest")
    report(reference, published, version, revision)


def main():
  if len(sys.argv) > 1:
    print("Usage: scripts/release.sh\nPublish linux/amd64 and linux/arm64 using FORGEJO_TOKEN.")
    return 0 if sys.argv[1:] == ["--help"] else 2
  try:
    release(Path(__file__).resolve().parent.parent)
    return 0
  except (ReleaseError, OSError, ValueError) as error:
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

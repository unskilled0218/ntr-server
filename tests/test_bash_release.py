"""Ported release contracts from template 3c73528231ac1cbdd43600a5ca723390db804bac.

Only owned temporary Git repositories and fake Docker/curl ports are used.
"""

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
WRITES = {"build", "tag-push", "promote", "release-post", "package-link"}


def helper_module():
  spec = importlib.util.spec_from_file_location("release_data", ROOT / "scripts/release-data.py")
  module = importlib.util.module_from_spec(spec)
  spec.loader.exec_module(module)
  return module


class Fixture:
  def __enter__(self):
    self.temporary = tempfile.TemporaryDirectory(prefix="ntr-release-test-", dir="/tmp/opencode")
    self.base = Path(self.temporary.name)
    self.root = self.base / "project with spaces"
    self.root.mkdir()
    self.bin = self.base / "bin"
    self.bin.mkdir()
    (self.root / "scripts").mkdir()
    for name in ("release.sh", "release-common.sh", "release-data.py", "release.json"):
      shutil.copyfile(ROOT / "scripts" / name, self.root / "scripts" / name)
    for name in ("Dockerfile", ".dockerignore", ".gitignore"):
      shutil.copyfile(ROOT / name, self.root / name)
    (self.root / "VERSION").write_text("1.0.0\n")
    (self.root / "Cargo.toml").write_text('[package]\nname = "my-ntr-server"\nversion = "1.0.0"\n')
    (self.root / "Cargo.lock").write_text('version = 4\n[[package]]\nname = "my-ntr-server"\nversion = "1.0.0"\n')
    (self.root / "README.md").write_text("Version 1.0.0 [notes](docs/releases/1.0.0.md)\n")
    (self.root / "notes.md").write_text("# Release v1.0.0\n\nProject changes.\n")
    self.token = self.base / "token"
    self.token.write_text("SENTINEL_PRIVATE_TOKEN\n")
    self.token.chmod(0o600)
    config = json.loads((self.root / "scripts/release.json").read_text())
    config |= {"repository": "fixtures/application", "image": "git.lan/fixtures/application-hello",
               "notes": "notes.md", "token_file": str(self.token)}
    (self.root / "scripts/release.json").write_text(json.dumps(config))
    self.state_path = self.base / "state.json"
    self.state_path.write_text(json.dumps({"objects": {}, "aliases": {}, "releases": []}))
    self.environment = {k: v for k, v in os.environ.items() if not k.startswith(
      ("GIT_", "GITEA_", "FORGEJO_", "DOCKER_", "BUILDX_", "RELEASE_")) and k not in ("BASH_ENV", "ENV")}
    self.environment |= {"GIT_AUTHOR_NAME": "Fixture", "GIT_COMMITTER_NAME": "Fixture",
      "GIT_AUTHOR_EMAIL": "fixture@localhost", "GIT_COMMITTER_EMAIL": "fixture@localhost",
      "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_SYSTEM": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1",
      "BASH_TEST_STATE": str(self.base), "PATH": str(self.bin) + ":" + os.environ["PATH"],
      "TMPDIR": str(self.base), "HOME": str(self.base), "XDG_CONFIG_HOME": str(self.base / "xdg")}
    for name in ("git", "curl", "docker"):
      target = self.bin / name
      shutil.copyfile(ROOT / "tests/fixtures/bash_release_ports.py", target)
      target.chmod(0o755)
    (self.base / "system").mkdir()
    plugin = self.base / "system/docker-buildx"
    plugin.write_text("fixture system Buildx\n")
    plugin.chmod(0o755)
    self.git("init", "--quiet", "--initial-branch=main")
    self.git("init", "--bare", "--quiet", str(self.base / "remote.git"))
    self.git("remote", "add", "origin", str(self.base / "remote.git"))
    self.commit()
    return self

  def __exit__(self, *exc):
    self.temporary.cleanup()

  def git(self, *args):
    return subprocess.check_output(["/usr/bin/git", "-C", str(self.root), *args], env=self.environment,
                                   stderr=subprocess.DEVNULL).decode().strip()

  def commit(self):
    self.git("add", ".")
    self.git("commit", "--quiet", "-m", "Fixture")
    self.git("push", "--quiet", "origin", "HEAD:main")

  def state(self):
    return json.loads(self.state_path.read_text())

  def change(self, **values):
    self.state_path.write_text(json.dumps(self.state() | values))

  def events(self):
    path = self.base / "events"
    return path.read_text().splitlines() if path.exists() else []

  def clear_events(self):
    (self.base / "events").write_text("")

  def release(self, *args, cwd=None):
    result = subprocess.run(["bash", str(self.root / "scripts/release.sh"), *args], cwd=cwd or self.base,
                            env=self.environment, capture_output=True, text=True, timeout=90)
    assert "SENTINEL_PRIVATE_TOKEN" not in result.stdout + result.stderr
    assert "UNRELATED_AUTH_SENTINEL" not in result.stdout + result.stderr
    assert not list(self.base.glob("bash-release.*")), "Run-owned temporary credentials were not cleaned"
    if result.returncode and "Command failed: git" in result.stderr:
      result.stderr += "\nFixture Git diagnostics: " + repr([e for e in self.events() if e.startswith("git-command-failed:")])
    return result

  def predecessor(self):
    data = helper_module()
    body = data.canonical({"previous": "verified fixture alias"})
    oid = data.sha(body)
    state = self.state()
    state["objects"][oid] = body.decode()
    state["aliases"]["latest"] = oid
    self.change(**state, predecessor=oid)
    return oid

  def target(self):
    return {"id": 17, "name": "application", "full_name": "fixtures/application", "owner": {"login": "fixtures"}}

  def reset_association(self):
    self.change(linked_repository=None, target_reads=0, package_reads=0, version_reads=0, association_fault={}, link_fault=None)
    self.clear_events()

  def prepare_recovery(self, bump_version=True):
    self.change(gitea_version="28.2.1")
    result = self.release()
    assert result.returncode != 0 and "server association safety is unverified" in result.stderr, result.stderr
    oid, commit = self.state()["confirmed_object"], self.git("rev-parse", "HEAD")
    if bump_version:
      for name in ("VERSION", "Cargo.toml", "Cargo.lock", "README.md"):
        path = self.root / name
        path.write_text(path.read_text().replace("1.0.0", "1.0.1"))
      (self.root / "new-notes.md").write_text("# Release v1.0.1\n\nNew recovery tooling.\n")
      config = json.loads((self.root / "scripts/release.json").read_text())
      config |= {"notes": "new-notes.md", "username": "new-tool"}
      (self.root / "scripts/release.json").write_text(json.dumps(config))
    with (self.root / "scripts/release-common.sh").open("a") as script:
      script.write("\n# New fixture tool revision; historical identity must remain distinct.\n")
    self.commit()
    self.reset_association()
    self.change(gitea_version="28.2.0")
    (self.bin / "docker").unlink()
    return ("--associate-existing", "v1.0.0", "--expected-tag-oid", oid, "--expected-source", commit)

  def connection(self, context=None, host=None, configured=None, explicit_builder=False):
    directory = self.base / "original-docker"
    directory.mkdir(exist_ok=True)
    config = {"auths": {"foreign.invalid": {"auth": "UNRELATED_AUTH_SENTINEL"}},
              "credsStore": "must-not-run", "credHelpers": {"foreign.invalid": "must-not-run"},
              "proxies": {"default": {"httpProxy": "http://proxy.fixture:8080"}}}
    if configured:
      config["currentContext"] = configured
    (directory / "config.json").write_text(json.dumps(config))
    self.environment["ORIGINAL_DOCKER_CONFIG"] = str(directory)
    named = context if context and context != "default" else configured
    if named:
      meta = directory / "contexts/meta/fixture-id/meta.json"
      meta.parent.mkdir(parents=True)
      meta.write_text(json.dumps({"Name": named, "Endpoints": {"docker": {"Host": "tcp://context.fixture.invalid:2376"}}}))
      tls = directory / "contexts/tls/fixture-id/docker/ca.pem"
      tls.parent.mkdir(parents=True)
      tls.write_text("fixture-ca")
    buildx = self.base / "selected-buildx" if explicit_builder else directory / "buildx"
    buildx.mkdir(exist_ok=True)
    (buildx / "current").write_text(json.dumps({"Name": "fixture-builder"}))
    self.environment["DOCKER_CONFIG"] = str(directory)
    if context is not None:
      self.environment["DOCKER_CONTEXT"] = context
    if host:
      self.environment["DOCKER_HOST"] = host
    cert = self.base / "host-certificates"
    cert.mkdir()
    (cert / "ca.pem").write_text("fixture-host-ca")
    if host or named:
      self.environment |= {"DOCKER_TLS_VERIFY": "1", "DOCKER_CERT_PATH": str(cert)}
    if explicit_builder:
      self.environment["BUILDX_CONFIG"] = str(buildx)
    return directory


class BashReleaseTests(unittest.TestCase):
  def assert_no_writes(self, f):
    self.assertFalse(set(f.events()) & WRITES, f.events())

  def assert_no_publication_writes(self, f):
    self.assertFalse(set(f.events()) & (WRITES - {"package-link"}), f.events())
    self.assertFalse(any(e.startswith("registry:rt-") for e in f.events()))

  def test_consumer_settings_version_notes_and_runtime_contract(self):
    data = helper_module()
    config = data.settings(ROOT)
    self.assertEqual(config["version"], "0.2.9")
    self.assertEqual(config["repository"], "ntr/ntr-server")
    self.assertEqual(config["image"], "git.lan/ntr/ntr-server")
    self.assertEqual(config["notes"], "docs/releases/0.2.9.md")
    self.assertFalse((ROOT / "scripts/image_release.py").exists())
    dockerfile = (ROOT / "Dockerfile").read_text()
    for token in ("cargo build --release --locked --bin my-ntr-server", "USER 1000:1000", "EXPOSE 8080", "/data/Note.md", "/data/todo.txt", "/data/labels.json"):
      self.assertIn(token, dockerfile)
    self.assertIn("git.lan/ntr/ntr-server:latest", (ROOT / "docker-compose.yml").read_text())
    self.assertNotIn("Hello World", dockerfile)

  def test_help_validate_and_unsupported_options_do_not_use_credentials_or_commands(self):
    with Fixture() as f:
      f.token.unlink()
      for args in (("--help",), ("--validate",)):
        result = f.release(*args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(f.events(), [])
        self.assertFalse((f.root / ".template-operation.lock").exists())
      for args in (("publish",), ("resume",), ("",), ("--unknown",), ("--help", "extra"), ("--config", "missing")):
        result = f.release(*args)
        self.assertEqual(result.returncode, 2)
        self.assertIn("Usage:", result.stderr)
        self.assertEqual(f.events(), [])

  def test_deprecated_environment_refuses_before_effects_without_logging_values(self):
    with Fixture() as f:
      for key in ("FORGEJO_TOKEN", "FORGEJO_USERNAME", "FORGEJO_IMAGE", "FORGEJO_CA_FILE", "FORGEJO_API_URL"):
        f.environment[key] = "SENTINEL_PRIVATE_TOKEN"
        result = f.release()
        self.assertEqual(result.returncode, 2)
        self.assertIn("Deprecated FORGEJO_", result.stderr)
        self.assertEqual(f.events(), [])
        f.environment.pop(key)

  def test_validation_rejects_json_mirrors_readme_notes_and_paths(self):
    with Fixture() as f:
      f.token.unlink()
      files = {name: (f.root / name).read_bytes() for name in ("scripts/release.json", "Cargo.toml", "Cargo.lock", "README.md", "notes.md", "VERSION")}
      faults = [("Cargo.toml", b'[package]\nname="my-ntr-server"\nversion="9.0.0"'),
                ("Cargo.lock", b'[[package]]\nname="my-ntr-server"\nversion="9.0.0"'),
                ("README.md", b"Version 9.0.0"), ("notes.md", b"# Release v9.0.0\n\nWrong."),
                ("VERSION", b"01.0.0"), ("notes.md", b"x" * 65537), ("scripts/release.json", b'{"schema":1,"schema":1}'),
                ("scripts/release.json", b"{")]
      config = json.loads(files["scripts/release.json"])
      for override in ({"extra": True}, {"notes": "../token"}, {"platforms": ["linux/amd64"]}, {"gitea": "http://git.lan"}, {"image": "git.lan/foreign/image"}):
        faults.append(("scripts/release.json", json.dumps(config | override).encode()))
      for name, contents in faults:
        with self.subTest(name=name, contents=contents[:60]):
          (f.root / name).write_bytes(contents)
          self.assertNotEqual(f.release("--validate").returncode, 0)
          (f.root / name).write_bytes(files[name])
      self.assertEqual(f.events(), [])
      self.assertFalse((f.root / ".template-operation.lock").exists())

  def test_publish_one_build_then_completed_retry_without_staging_is_read_only(self):
    with Fixture() as f:
      result = f.release()
      self.assertEqual(result.returncode, 0, result.stderr)
      self.assertIn("Publication verified: v1.0.0", result.stdout)
      for action in WRITES:
        self.assertEqual(f.events().count(action), 1, action)
      self.assertEqual(f.state()["releases"][0]["body"], "# Release v1.0.0\n\nProject changes.\n")
      self.assertEqual(f.state()["linked_repository"], f.target())
      oid = f.git("ls-remote", "--tags", "origin", "refs/tags/v1.0.0").split()[0]
      staging = next(a for a in f.state()["aliases"] if a.startswith("rt-"))
      for aliases, cwd in (({"latest": f.state()["prepared"]}, f.root),
                           ({"latest": f.state()["prepared"], staging: "sha256:" + "0" * 64}, f.bin)):
        f.change(aliases=aliases)
        f.clear_events()
        self.assertEqual(f.release(cwd=cwd).returncode, 0)
        self.assert_no_writes(f)
        self.assertFalse(any(e.startswith("registry:rt-") for e in f.events()))
        self.assertEqual(f.git("ls-remote", "--tags", "origin", "refs/tags/v1.0.0").split()[0], oid)

  def test_follow_tags_setting_cannot_expand_publication_or_completed_retry(self):
    for lost in (False, True):
      with self.subTest(lost_tag_response=lost), Fixture() as f:
        # Source setup must finish before enabling follow-tags or creating this tag.
        f.git("config", "push.followTags", "true")
        f.git("tag", "-a", "unrequested-tag", "-m", "Keep this annotation local")
        self.assertEqual(f.git("config", "--bool", "push.followTags"), "true")
        local_tag = f.git("rev-parse", "refs/tags/unrequested-tag")
        source = f.git("rev-parse", "HEAD")
        self.assertEqual(f.git("cat-file", "-t", local_tag), "tag")
        self.assertEqual(f.git("rev-parse", "unrequested-tag^{}"), source)
        before = dict(line.split()[::-1] for line in f.git("ls-remote", "origin").splitlines())
        self.assertNotIn("refs/tags/unrequested-tag", before)
        self.assertEqual(before["refs/heads/main"], source)
        configuration = (f.root / ".git/config").read_bytes()
        f.change(lose_tag=lost)

        result = f.release()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Publication verified: v1.0.0", result.stdout)
        for action in WRITES:
          self.assertEqual(f.events().count(action), 1, action)
        after = dict(line.split()[::-1] for line in f.git("ls-remote", "origin").splitlines())
        self.assertEqual(set(after) - set(before), {"refs/tags/v1.0.0", "refs/tags/v1.0.0^{}"}, after)
        self.assertEqual({ref: after.get(ref) for ref in before}, before)
        self.assertEqual(after["refs/tags/v1.0.0^{}"], source)
        self.assertEqual(after["refs/tags/v1.0.0"], f.state()["confirmed_object"])
        self.assertEqual(f.git("rev-parse", "refs/tags/unrequested-tag"), local_tag)
        self.assertEqual(f.git("config", "--bool", "push.followTags"), "true")
        self.assertEqual((f.root / ".git/config").read_bytes(), configuration)

        f.clear_events()
        result = f.release()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_no_writes(f)
        self.assertEqual(dict(line.split()[::-1] for line in f.git("ls-remote", "origin").splitlines()), after)
        self.assertEqual(f.git("rev-parse", "refs/tags/unrequested-tag"), local_tag)
        self.assertEqual(f.git("config", "--bool", "push.followTags"), "true")
        self.assertEqual((f.root / ".git/config").read_bytes(), configuration)

  def test_lost_accepted_writes_reconcile_once_without_replay(self):
    with Fixture() as f:
      f.change(lose_tag=True, lose_promotion=True, lose_release=True, link_fault="lost")
      result = f.release()
      self.assertEqual(result.returncode, 0, result.stderr)
      for action in WRITES:
        self.assertEqual(f.events().count(action), 1)

  def test_interrupted_build_reuses_verified_staging(self):
    with Fixture() as f:
      f.change(lose_build=True)
      self.assertNotEqual(f.release().returncode, 0)
      f.change(lose_build=False)
      result = f.release()
      self.assertEqual(result.returncode, 0, result.stderr)
      self.assertEqual(f.events().count("build"), 1)

  def test_tag_only_and_release_only_startup_refuse_without_staging_lookup(self):
    for only in ("tag", "release"):
      with self.subTest(only=only), Fixture() as f:
        if only == "tag":
          f.change(fail_release=True)
          self.assertNotEqual(f.release().returncode, 0)
          f.change(fail_release=False)
        else:
          f.change(releases=[{"id": 1, "tag_name": "v1.0.0", "draft": False, "prerelease": False}])
        f.clear_events()
        result = f.release()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Ambiguous", result.stderr)
        self.assert_no_writes(f)
        self.assertFalse(any(e.startswith("registry:") for e in f.events()))

  def test_preflight_errors_never_authorize_publication(self):
    for flag in ("wrong_remote", "wrong_push_remote", "multiple_remote", "multiple_push_remote", "wrong_main", "unsupported", "null_inventory", "malformed", "oversized", "realm_foreign"):
      with self.subTest(flag=flag), Fixture() as f:
        f.change(**{flag: True})
        result = f.release()
        self.assertNotEqual(result.returncode, 0)
        if flag.startswith("multiple_"):
          self.assertIn("multiple-destination-injected", f.events())
          self.assertIn("Require one exact Git destination", result.stderr)
        self.assert_no_writes(f)
    for status in (401, 500):
      with self.subTest(status=status), Fixture() as f:
        f.change(status=status)
        self.assertNotEqual(f.release().returncode, 0)
        self.assert_no_writes(f)

  def test_wrong_branch_dirty_source_and_local_tag_refuse(self):
    for kind in ("branch", "dirty", "tag"):
      with self.subTest(kind=kind), Fixture() as f:
        if kind == "branch":
          f.git("switch", "--quiet", "-c", "task")
        elif kind == "dirty":
          (f.root / "user-work").write_text("keep")
        else:
          f.git("tag", "v1.0.0")
        self.assertNotEqual(f.release().returncode, 0)
        self.assert_no_writes(f)

  def test_newer_publications_and_repeated_inventory_pages_refuse(self):
    for newer in (True, False):
      with self.subTest(newer=newer), Fixture() as f:
        f.change(releases=[{"id": 2, "tag_name": "v2.0.0" if newer else "v0.9.0", "draft": False, "prerelease": False}], repeat_pages=not newer)
        result = f.release()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Newer" if newer else "Pagination", result.stderr)
        self.assert_no_writes(f)

  def test_confirmed_tag_loss_or_replacement_stops_before_next_write(self):
    for stage, kind in (("before_promotion", "absent"), ("before_release", "absent"), ("before_promotion", "lightweight"),
                        ("before_promotion", "changed"), ("before_promotion", "wrong_peel")):
      with self.subTest(stage=stage, kind=kind), Fixture() as f:
        predecessor = f.predecessor()
        f.change(tag_change={"stage": stage, "kind": kind})
        result = f.release()
        injection = "tag-change:" + stage + ":" + kind
        events = f.events()
        self.assertIn(injection, events)
        self.assertLess(events.index("tag-confirmed"), events.index(injection))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(set(events[events.index(injection) + 1:]) & WRITES)
        self.assertEqual(events.count("tag-push"), 1)
        self.assertEqual(f.state()["aliases"]["latest"], f.state()["prepared"] if stage == "before_release" else predecessor)

  def test_promotion_read_faults_are_injected_and_block_later_writes(self):
    for fault in ("overflow", "malformed", "null", "unauthorized", "server_error", "transport", "timeout", "foreign"):
      with self.subTest(fault=fault), Fixture() as f:
        predecessor = f.predecessor()
        f.change(promotion_read_fault=fault)
        result = f.release()
        injection = "promotion-read-injected:" + fault
        events = f.events()
        self.assertIn(injection, events)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(set(events[events.index(injection) + 1:]) & WRITES)
        self.assertEqual(events.count("tag-push"), 1)
        if fault != "foreign":
          self.assertEqual(f.state()["aliases"]["latest"], predecessor)

  def test_already_prepared_alias_skips_duplicate_promotion(self):
    with Fixture() as f:
      f.change(promotion_read_fault="prepared")
      result = f.release()
      self.assertEqual(result.returncode, 0, result.stderr)
      self.assertIn("promotion-read-injected:prepared", f.events())
      self.assertNotIn("promote", f.events())

  def test_n1_confirmed_promotion_never_accepts_predecessor_again(self):
    for kind in ("predecessor", "missing", "foreign"):
      with self.subTest(kind=kind), Fixture() as f:
        f.predecessor()
        f.change(after_promotion_alias=kind)
        result = f.release()
        events = f.events()
        injection = "post-promotion-injected:" + kind
        self.assertIn(injection, events)
        self.assertEqual(events.count("promote"), 1)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(set(events[events.index(injection) + 1:]) & WRITES)
        self.assertEqual(f.state()["releases"], [])

  def test_n1_negative_control_exposes_the_old_post_despite_nonzero_exit(self):
    # Mutate only an owned fixture: the original upstream allowance must fail
    # the no-later-write invariant, even though its eventual exit is nonzero.
    with Fixture() as f:
      path = f.root / "scripts/release-common.sh"
      source = path.read_text()
      start = source.index('  if [[ "$promotion_confirmed" == true ]]; then')
      end = source.index('\n  fi', start) + len('\n  fi')
      path.write_text(source[:start] + '  [[ "$current" == "$predecessor" || "$current" == "$prepared" ]] || stop changed' + source[end:])
      f.commit()
      f.predecessor()
      f.change(after_promotion_alias="predecessor")
      result = f.release()
      events = f.events()
      self.assertIn("post-promotion-injected:predecessor", events)
      self.assertNotEqual(result.returncode, 0)
      self.assertGreater(events.index("release-post"), events.index("post-promotion-injected:predecessor"))

  def test_missing_tools_refuse_before_credentials_or_commands(self):
    for tool in ("python3", "docker", "git", "curl"):
      with self.subTest(tool=tool), Fixture() as f:
        for name in ("python3", "dirname"):
          (f.bin / name).symlink_to(shutil.which(name))
        (f.bin / tool).unlink()
        f.environment["PATH"] = str(f.bin)
        result = subprocess.run(["/usr/bin/bash", str(f.root / "scripts/release.sh")], env=f.environment, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(tool if tool != "python3" else "Python", result.stderr)
        self.assertEqual(f.events(), [])
        self.assertFalse((f.root / ".template-operation.lock").exists())

  def test_help_needs_no_other_tools(self):
    with Fixture() as f:
      result = subprocess.run(["/usr/bin/bash", str(f.root / "scripts/release.sh"), "--help"],
                              env=f.environment | {"PATH": "/nonexistent"}, capture_output=True, text=True)
      self.assertEqual(result.returncode, 0, result.stderr)

  def test_docker_inspect_login_and_build_failures_stop_at_their_boundary(self):
    for stage in ("inspect", "login", "build"):
      with self.subTest(stage=stage), Fixture() as f:
        f.change(docker_fail=stage)
        result = f.release()
        events = f.events()
        injection = "docker-failed:" + stage
        self.assertIn(injection, events)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(set(events[events.index(injection) + 1:]) & WRITES)
        self.assertNotIn("tag-push", events)

  def test_bad_metadata_or_changed_alias_after_build_stops_before_tag(self):
    for flag in ("bad_metadata", "change_latest_build", "oversize_after_promotion"):
      with self.subTest(flag=flag), Fixture() as f:
        f.change(**{flag: True})
        self.assertNotEqual(f.release().returncode, 0)
        self.assertEqual(f.events().count("build"), 1)
        self.assertNotIn("release-post", f.events())
        if flag != "oversize_after_promotion":
          self.assertNotIn("tag-push", f.events())

  def test_completed_foreign_body_missing_objects_and_changed_source_do_not_repair(self):
    for kind in ("body", "alias", "objects", "source", "local-tag"):
      with self.subTest(kind=kind), Fixture() as f:
        self.assertEqual(f.release().returncode, 0)
        state = f.state()
        if kind == "body":
          f.change(releases=[state["releases"][0] | {"body": "Foreign body"}])
        elif kind == "alias":
          f.change(aliases={})
        elif kind == "objects":
          f.change(objects={})
        elif kind == "local-tag":
          f.git("tag", "v1.0.0")
        else:
          (f.root / "new-source").write_text("new")
          f.commit()
        f.clear_events()
        self.assertNotEqual(f.release().returncode, 0)
        self.assert_no_writes(f)

  def test_engine_context_tls_and_builder_selection_survive_private_auth(self):
    cases = [(None, "tcp://host.fixture.invalid:2376", None, "tcp://host.fixture.invalid:2376"),
             ("", "tcp://host.fixture.invalid:2376", None, "tcp://host.fixture.invalid:2376"),
             (None, None, "remote-builder", "tcp://context.fixture.invalid:2376"),
             ("remote-builder", "tcp://ignored:2376", None, "tcp://context.fixture.invalid:2376"),
             ("default", "tcp://ignored:2376", None, "unix:///var/run/docker.sock"),
             (None, None, None, "unix:///var/run/docker.sock")]
    for context, host, configured, endpoint in cases:
      with self.subTest(context=context, host=host), Fixture() as f:
        original = f.connection(context, host, configured, explicit_builder=True)
        before = (original / "config.json").read_bytes()
        result = f.release()
        self.assertEqual(result.returncode, 0, result.stderr)
        selections = [json.loads(line) for line in (f.base / "docker-selections").read_text().splitlines()]
        self.assertEqual({s["endpoint"] for s in selections}, {endpoint})
        self.assertEqual({s["builder"] for s in selections}, {"fixture-builder"})
        self.assertEqual({s["tls_verify"] for s in selections}, {f.environment.get("DOCKER_TLS_VERIFY")})
        self.assertEqual({s["cert_path"] for s in selections}, {f.environment.get("DOCKER_CERT_PATH")})
        self.assertEqual((original / "config.json").read_bytes(), before)

  def test_relative_paths_empty_config_and_builder_override_preserve_selection(self):
    for kind in ("relative", "empty", "builder"):
      with self.subTest(kind=kind), Fixture() as f:
        directory = f.connection(host="tcp://host.fixture.invalid:2376", explicit_builder=True)
        if kind == "relative":
          for key in ("DOCKER_CONFIG", "DOCKER_CERT_PATH", "BUILDX_CONFIG"):
            f.environment[key] = str(Path(f.environment[key]).relative_to(f.base))
        elif kind == "empty":
          directory.rename(f.base / ".docker")
          f.environment["DOCKER_CONFIG"] = ""
          f.environment["ORIGINAL_DOCKER_CONFIG"] = str(f.base / ".docker")
        else:
          f.environment["BUILDX_BUILDER"] = "override-builder"
        result = f.release()
        self.assertEqual(result.returncode, 0, result.stderr)
        selections = [json.loads(line) for line in (f.base / "docker-selections").read_text().splitlines()]
        self.assertEqual({s["endpoint"] for s in selections}, {"tcp://host.fixture.invalid:2376"})
        self.assertEqual({s["builder"] for s in selections}, {"override-builder" if kind == "builder" else "fixture-builder"})

  def test_n2_config_only_plugin_discovery_is_preserved(self):
    for symlink in (False, True):
      with self.subTest(symlink=symlink), Fixture() as f:
        directory = f.connection()
        plugin = directory / "cli-plugins/docker-buildx"
        plugin.parent.mkdir()
        backing = f.base / "buildx-custom-name" if symlink else plugin
        backing.write_text("fixture private selected Buildx\n")
        backing.chmod(0o755)
        if symlink:
          plugin.symlink_to(backing)
        (f.base / "system/docker-buildx").unlink()
        result = f.release()
        self.assertEqual(result.returncode, 0, result.stderr)
        selections = [json.loads(line) for line in (f.base / "docker-selections").read_text().splitlines()]
        self.assertEqual({s["plugin"] for s in selections}, {str(backing)})
        self.assertEqual(sum(e.startswith("plugin-info:") for e in f.events()), 2)

  def test_n2_changed_selected_plugin_refuses_before_publication(self):
    with Fixture() as f:
      directory = f.connection()
      plugin = directory / "cli-plugins/docker-buildx"
      plugin.parent.mkdir()
      plugin.write_text("different selected plugin\n")
      plugin.chmod(0o755)
      f.change(plugin_switch=True)
      result = f.release()
      self.assertIn("plugin-switch-injected", f.events())
      self.assertNotEqual(result.returncode, 0)
      self.assertIn("plugin changed", result.stderr)
      self.assert_no_writes(f)

  def test_unusable_context_fails_without_fallback(self):
    with Fixture() as f:
      directory = f.connection(configured="remote-builder")
      (directory / "contexts/tls/fixture-id/docker/ca.pem").unlink()
      self.assertNotEqual(f.release().returncode, 0)
      self.assert_no_writes(f)

  def test_private_external_regular_credentials_required(self):
    for kind in ("missing", "public", "symlink", "inside", "oversized"):
      with self.subTest(kind=kind), Fixture() as f:
        if kind == "missing":
          f.token.unlink()
        elif kind == "public":
          f.token.chmod(0o644)
        elif kind == "symlink":
          backing = f.base / "backing"
          f.token.rename(backing)
          f.token.symlink_to(backing)
        elif kind == "inside":
          f.environment["GIT_TOKEN"] = str(f.root / "VERSION")
        else:
          f.token.write_text("x" * 4097)
        self.assertNotEqual(f.release().returncode, 0)
        self.assert_no_writes(f)

  def test_explicit_login_and_token_file_override_are_used_without_env_leaks(self):
    with Fixture() as f:
      alternate = f.base / "alternate-private-token"
      f.token.rename(alternate)
      f.environment |= {"GIT_USERNAME": "override-login", "GIT_TOKEN": str(alternate)}
      result = f.release()
      self.assertEqual(result.returncode, 0, result.stderr)
      self.assertIn("login-user:override-login", f.events())

  def test_validation_does_not_follow_active_input_symlinks(self):
    for name in ("VERSION", "notes.md", "Cargo.toml", "Cargo.lock"):
      for external in (False, True):
        with self.subTest(name=name, external=external), Fixture() as f:
          original = f.root / name
          backing = (f.base if external else f.root) / "backing"
          original.rename(backing)
          original.symlink_to(backing)
          result = f.release("--validate")
          self.assertNotEqual(result.returncode, 0)
          self.assertIn("Path escapes project" if external else "Symlink input refused", result.stderr)
          self.assertEqual(f.events(), [])
          self.assertFalse((f.root / ".template-operation.lock").exists())

  def test_old_markers_and_lock_symlinks_do_not_modify_targets(self):
    for name in ("release-template.lock.json", "scripts/release-template", ".template-transaction", ".release-template/v1.0.0.json", ".template-operation.lock"):
      with self.subTest(name=name), Fixture() as f:
        path = f.root / name
        path.parent.mkdir(exist_ok=True)
        path.symlink_to(f.token)
        self.assertNotEqual(f.release().returncode, 0)
        self.assertTrue(path.is_symlink())
        self.assertEqual(f.token.read_text(), "SENTINEL_PRIVATE_TOKEN\n")
        self.assert_no_writes(f)

  def test_lock_excludes_another_owner(self):
    import fcntl
    with Fixture() as f, (f.root / ".template-operation.lock").open("w") as held:
      fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
      self.assertNotEqual(f.release().returncode, 0)
      self.assert_no_writes(f)

  def test_committed_snapshot_excludes_ignored_secrets_and_external_symlinks(self):
    for unsafe in (False, True):
      with self.subTest(unsafe=unsafe), Fixture() as f:
        with (f.root / ".gitignore").open("a") as out:
          out.write("ignored-secret\n")
        if unsafe:
          (f.root / "escape").symlink_to(f.token)
        f.commit()
        (f.root / "ignored-secret").write_text("SENTINEL_PRIVATE_TOKEN")
        result = f.release()
        if unsafe:
          self.assertNotEqual(result.returncode, 0)
          self.assertIn("symlink escapes", result.stderr)
          self.assert_no_writes(f)
        else:
          self.assertEqual(result.returncode, 0, result.stderr)

  def test_signal_cleans_temporaries_and_releases_lock(self):
    import fcntl
    with Fixture() as f:
      f.change(hang_build=True)
      process = subprocess.Popen(["bash", str(f.root / "scripts/release.sh")], cwd=f.base,
                                 env=f.environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
      try:
        deadline = time.monotonic() + 30
        while "build" not in f.events() and process.poll() is None and time.monotonic() < deadline:
          time.sleep(0.02)
        self.assertIn("build", f.events())
        process.terminate()
        stdout, stderr = process.communicate(timeout=10)
        self.assertNotEqual(process.returncode, 0)
        self.assertNotIn("SENTINEL_PRIVATE_TOKEN", stdout + stderr)
        self.assertEqual(list(f.base.glob("bash-release.*")), [])
        with (f.root / ".template-operation.lock").open() as lock:
          fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
      finally:
        if process.poll() is None:
          process.kill()
        process.communicate()

  def test_package_association_scopes_and_preserves_other_packages(self):
    with Fixture() as f:
      unrelated = {"other": {"repository": {"id": 999}}}
      f.change(unrelated_packages=unrelated)
      result = f.release()
      self.assertEqual(result.returncode, 0, result.stderr)
      self.assertEqual(f.state()["linked_repository"], f.target())
      self.assertEqual(f.state()["unrelated_packages"], unrelated)
      self.assertLess(f.events().index("release-post"), f.events().index("package-link"))
      self.assertIn("http:GET:/api/v1/packages/fixtures/container/application-hello/latest", f.events())
      self.assertFalse(any("DELETE" in e or "unlink" in e for e in f.events()))

  def test_correct_or_conflicting_link_never_reassigns(self):
    for correct in (False, True):
      with self.subTest(correct=correct), Fixture() as f:
        linked = f.target() if correct else f.target() | {"id": 99}
        f.change(linked_repository=linked)
        result = f.release()
        self.assertEqual(result.returncode == 0, correct, result.stderr)
        self.assertNotIn("package-link", f.events())
        self.assertEqual(f.state()["linked_repository"], linked)
        self.assertEqual(len(f.state()["releases"]), 1)

  def test_completed_retry_can_finish_only_missing_link(self):
    with Fixture() as f:
      f.change(link_fault="denied")
      self.assertNotEqual(f.release().returncode, 0)
      before = f.state()
      f.reset_association()
      self.assertEqual(f.release().returncode, 0)
      self.assert_no_publication_writes(f)
      self.assertEqual(f.events().count("package-link"), 1)
      self.assertEqual(f.state()["releases"], before["releases"])
      self.assertEqual(f.state()["aliases"], before["aliases"])
      f.clear_events()
      self.assertEqual(f.release().returncode, 0)
      self.assert_no_writes(f)

  def test_unknown_server_gate_and_hidden_links_do_not_reassign(self):
    for server in ("1.24.6", "1.26.0", "28.2.1", "28.2.0-dev", "28.2.0+custom", "1.25.0-dev"):
      with self.subTest(server=server), Fixture() as f:
        f.change(gitea_version=server)
        self.assertNotEqual(f.release().returncode, 0)
        self.assertNotIn("package-link", f.events())
        f.change(linked_repository=f.target())
        f.clear_events()
        self.assertEqual(f.release().returncode, 0)
        self.assert_no_writes(f)
        self.assertNotIn("http:GET:/api/v1/version", f.events())
    with Fixture() as f:
      foreign = f.target() | {"id": 99, "name": "private", "full_name": "fixtures/private"}
      f.change(linked_repository=foreign, hide_link=True)
      self.assertNotEqual(f.release().returncode, 0)
      self.assertEqual(f.state()["linked_repository"], foreign)
      self.assertEqual(f.events().count("package-link"), 1)
      self.assertEqual(f.state()["package_reads"], 2)

  def test_link_errors_never_repeat_post_or_rollback_publication(self):
    with Fixture() as f:
      self.assertEqual(f.release().returncode, 0)
      before = f.state()
      for fault in ("transport", "timeout", "denied", "missing", "server", "wrong_status", "ignore", "conflict_after", "overflow", "nonempty"):
        with self.subTest(fault=fault):
          f.reset_association()
          f.change(link_fault=fault)
          result = f.release()
          self.assertNotEqual(result.returncode, 0)
          self.assertNotIn("Publication verified", result.stdout)
          self.assertEqual(f.events().count("package-link"), 1)
          self.assert_no_publication_writes(f)
          self.assertEqual(f.state()["releases"], before["releases"])
          self.assertEqual(f.state()["aliases"], before["aliases"])
          if fault in ("overflow", "nonempty"):
            self.assertEqual(f.state()["package_reads"], 2)

  def test_failed_and_refreshed_association_evidence_never_authorizes_post(self):
    with Fixture() as f:
      self.assertEqual(f.release().returncode, 0)
      cases = [(resource, kind, 1) for resource, kinds in (
        ("target", ("missing", "unauthorized", "server", "redirect", "null", "wrong_name")),
        ("version", ("missing", "unauthorized", "server", "redirect", "null", "malformed", "transport")),
        ("package", ("missing", "unauthorized", "server", "redirect", "malformed", "null", "overflow", "absent_repository", "wrong_name", "wrong_version", "transport", "timeout"))) for kind in kinds]
      cases += [("target", "wrong_id", 2), ("package", "absent_repository", 2)]
      for resource, kind, read in cases:
        with self.subTest(resource=resource, kind=kind, read=read):
          f.reset_association()
          f.change(association_fault={"resource": resource, "kind": kind, "read": read})
          result = f.release()
          self.assertNotEqual(result.returncode, 0)
          self.assertIn("association-read-injected:" + kind, f.events(), result.stderr)
          self.assert_no_writes(f)

  def test_post_link_failures_preserve_accepted_link_and_refuse_success(self):
    with Fixture() as f:
      self.assertEqual(f.release().returncode, 0)
      for read, kind in ((3, "malformed"), (3, "transport"), (4, "null"), (3, "change_latest")):
        with self.subTest(read=read, kind=kind):
          # Restore the last verified publication's alias before testing the next fault.
          aliases = f.state()["aliases"] | {"latest": f.state()["prepared"]}
          f.change(aliases=aliases)
          f.reset_association()
          f.change(association_fault={"resource": "package", "kind": kind, "read": read})
          self.assertNotEqual(f.release().returncode, 0)
          self.assertEqual(f.events().count("package-link"), 1)
          self.assertEqual(f.state()["linked_repository"], f.target())
          self.assert_no_publication_writes(f)


class DataTests(unittest.TestCase):
  def setUp(self):
    self.data = helper_module()

  def test_strict_json_versions_and_destination_normalization(self):
    for value in (b'{"x":1,"x":2}', b'{"x":NaN}', b'"\xff"'):
      with self.assertRaises((self.data.Refusal, ValueError, UnicodeError)):
        self.data.decode(value)
    for value in ("01.0.0", "1.0", "v1.0.0", None):
      with self.assertRaises(self.data.Refusal):
        self.data.version(value)
    for url in ("git@git.lan:fixtures/application.git", "ssh://git@git.lan:2222/fixtures/application.git", "https://git.lan:443/fixtures/application.git"):
      self.assertEqual(self.data.remote_key(url), ("git.lan", "fixtures/application"))
    self.assertEqual(self.data.remote_key("https://git.lan:8443/fixtures/application.git"), ("git.lan:8443", "fixtures/application"))

  def test_exact_byte_budgets_and_overflow(self):
    for limit in (16, self.data.ANNOTATION_LIMIT, self.data.CONFIG_LIMIT, self.data.API_LIMIT, self.data.REF_LIMIT):
      for extra in (0, 1):
        command = [sys.executable, "-c", "import sys; sys.stdout.buffer.write(b'x'*int(sys.argv[1]))", str(limit + extra)]
        with self.subTest(limit=limit, extra=extra):
          if extra:
            with self.assertRaisesRegex(self.data.Refusal, "byte limit"):
              self.data.command(command, limit)
          else:
            self.assertEqual(len(self.data.command(command, limit)), limit)

  def test_unending_output_stderr_and_blocked_input_are_bounded(self):
    for descriptor in (1, 2):
      with self.assertRaisesRegex(self.data.Refusal, "byte limit"):
        self.data.command([sys.executable, "-c", f"import os\nwhile True: os.write({descriptor},b'x'*65536)"], 1024)
    with self.assertRaisesRegex(self.data.Refusal, "timed out"):
      self.data.command([sys.executable, "-c", "import time; time.sleep(5)"], seconds=0.05, input_data=b"x" * 65536)

  def test_timeout_kills_descendants(self):
    with tempfile.TemporaryDirectory(dir="/tmp/opencode") as directory:
      marker = Path(directory) / "escaped"
      child = "import time; from pathlib import Path; time.sleep(.3); Path(" + repr(str(marker)) + ").write_text('bad')"
      code = "import subprocess,sys,time; subprocess.Popen([sys.executable,'-c',sys.argv[1]]); time.sleep(5)"
      with self.assertRaises(self.data.Refusal):
        self.data.command([sys.executable, "-c", code, child], seconds=0.1)
      time.sleep(0.4)
      self.assertFalse(marker.exists())

  def test_inventory_budgets_termination_and_duplicate_evidence(self):
    d = self.data
    config = {"gitea": "https://fixture.invalid", "repository": "fixtures/application", "tag": "v1.0.0", "version": "1.0.0"}
    record = {"tag_name": "v0.9.0", "draft": False, "prerelease": False}
    body, empty = d.canonical([record]), b"[]"
    for cap, value, replies, valid in (("ITEM_LIMIT", 1, [body, empty, empty], True), ("ITEM_LIMIT", 0, [body], False),
      ("INVENTORY_LIMIT", len(body) + 4, [body, empty, empty], True), ("INVENTORY_LIMIT", len(body) + 3, [body, empty, empty], False),
      ("PAGE_LIMIT", 2, [empty, empty], True), ("PAGE_LIMIT", 1, [empty], False)):
      with self.subTest(cap=cap, value=value), patch.object(d, cap, value), patch.object(d, "http", side_effect=[(200, {}, b) for b in replies]), patch.object(d, "command", return_value=b""):
        if valid:
          self.assertIsNone(d.inventory("unused", "unused", config)["object"])
        else:
          with self.assertRaises(d.Refusal):
            d.inventory("unused", "unused", config)
    for pages in (([record], [record]), ([record], [record | {"id": 2}])):
      with patch.object(d, "http", side_effect=[(200, {}, d.canonical(p)) for p in pages]), self.assertRaises(d.Refusal):
        d.inventory("unused", "unused", config)
    with patch.object(d.time, "monotonic", side_effect=[0, 601]), self.assertRaises(d.Refusal):
      d.inventory("unused", "unused", config)
    for refs in (b"bad ref\n", ("a" * 40 + "\trefs/tags/v1.0.0^{}\n").encode()):
      with patch.object(d, "http", return_value=(200, {}, b"[]")), patch.object(d, "command", return_value=refs), self.assertRaises(d.Refusal):
        d.inventory("unused", "unused", config)

  def test_committed_literal_notes_and_deterministic_annotated_identity(self):
    d = self.data
    with Fixture() as f:
      raw = b"# Release v1.0.0\r\n\r\nProject  fixes.\rKeep spaces.  \r\n\r\n"
      literal = "notes with space*.md"
      (f.root / literal).write_bytes(raw)
      (f.root / "notes-link.md").symlink_to("notes.md")
      f.commit()
      commit = f.git("rev-parse", "HEAD")
      config = d.settings(f.root) | {"notes": literal}
      (f.root / literal).write_text("Uncommitted replacement")
      self.assertEqual(d.notes(f.root, commit, config), "# Release v1.0.0\n\nProject  fixes.\nKeep spaces.  \n")
      for name in ("notes-link.md", "scripts", "notes with space?.md", "../notes.md"):
        with self.assertRaises(d.Refusal):
          d.blob(f.root, commit, name)
      identity = d.base_identity(f.root, commit, config) | {"digest": "sha256:" + "a" * 64, "predecessor": None}
      oid = d.create_tag(f.root, identity)
      self.assertEqual(d.create_tag(f.root, identity), oid)
      self.assertEqual(d.inspect_tag(f.root, commit, oid, config), identity)
      for override in ({"extra": True}, {"repository": "foreign/project"}, {"digest": "latest"}, {"predecessor": "bad"}):
        with self.assertRaises(d.Refusal):
          d.inspect_tag(f.root, commit, d.create_tag(f.root, identity | override), config)
      f.git("tag", "v1.0.0")
      with self.assertRaisesRegex(d.Refusal, "Local tag conflicts"):
        d.create_tag(f.root, identity)

  def test_note_validation_and_platform_tokens(self):
    d = self.data
    for raw in (b"\xff", b"# Release v1.0.0\n", b"# Release v1.0.0\n\nBad\x00text\n"):
      with self.assertRaises((d.Refusal, UnicodeError)):
        d.normalize_notes(raw, "v1.0.0")
    with tempfile.TemporaryDirectory(dir="/tmp/opencode") as directory:
      path = Path(directory) / "builder"
      path.write_text("Platforms: linux/amd64, linux/arm64\n")
      self.assertEqual(d.dispatch(["platforms", str(path)]), 0)
      path.write_text("Platforms: linux/amd64foreign, linux/arm64\n")
      with self.assertRaises(d.Refusal):
        d.dispatch(["platforms", str(path)])

  def test_registry_errors_are_not_absence_and_bearer_tokens_stay_same_origin(self):
    d = self.data
    def reply(code, body, headers=None):
      raw = "HTTP/1.1 " + str(code) + " Fixture\r\n" + "".join(k + ": " + v + "\r\n" for k, v in (headers or {}).items()) + "\r\n"
      return 0, {"stdout": str(code).encode(), "headers": raw.encode(), "body": body}
    args = ("unused", "https://fixture.invalid", "fixture.invalid/fixtures/application", "manifests")
    with patch.object(d, "capture", return_value=reply(404, b"{}")):
      self.assertIsNone(d.registry(*args, "latest", missing=True))
    body = b'{"schemaVersion":2}'
    for response, ref in ((reply(401, b"{}"), "latest"), (reply(500, b"{}"), "latest"), (reply(302, b"{}"), "latest"),
                          (reply(200, b"{broken"), "latest"), (reply(200, body), "sha256:" + "0" * 64)):
      with patch.object(d, "capture", return_value=response), self.assertRaises((d.Refusal, ValueError)):
        d.registry(*args, ref, missing=True)
    foreign = reply(401, b"{}", {"WWW-Authenticate": 'Bearer realm="https://foreign.invalid/token"'})
    with patch.object(d, "capture", return_value=foreign) as capture, self.assertRaises(d.Refusal):
      d.registry(*args, "latest")
    self.assertEqual(capture.call_count, 1)
    with tempfile.TemporaryDirectory(dir="/tmp/opencode") as work:
      replies = [reply(401, b"{}", {"WWW-Authenticate": 'Bearer realm="https://fixture.invalid/token",service="registry"'}),
                 reply(200, b'{"token":"FIXTURE_BEARER_TOKEN"}'), reply(200, body, {"Docker-Content-Digest": d.sha(body)})]
      with patch.object(d, "capture", side_effect=replies) as capture:
        self.assertEqual(d.registry(work, args[1], args[2], "manifests", d.sha(body))[0], d.sha(body))
        self.assertEqual(capture.call_count, 3)
        for call in capture.call_args_list:
          self.assertNotIn("FIXTURE_BEARER_TOKEN", " ".join(call.args[0]))

  def test_conditional_shell_guards_do_not_hide_failed_reads(self):
    script = '''set -Eeuo pipefail
source "$1"
root=unused work=unused commit=unused predecessor='' prepared=digest promotion_confirmed=false
fault_status=$3
data() { return "$fault_status"; }
if "$2"; then echo continued; fi
'''
    for helper in ("scan", "tag_state", "alias_guard"):
      for status in ("1", "2"):
        result = subprocess.run(["bash", "-c", script, "fixture", str(ROOT / "scripts/release-common.sh"), helper, status], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("continued", result.stdout)

  def test_image_shape_platform_and_labels_are_verified(self):
    d = self.data
    config = {"gitea": "https://fixture.invalid", "image": "fixture.invalid/fixtures/application", "repository": "fixtures/application", "version": "1.0.0"}
    commit, root, member, content = "a" * 40, "sha256:" + "1" * 64, "sha256:" + "2" * 64, "sha256:" + "3" * 64
    labels = {"org.opencontainers.image.revision": commit, "org.opencontainers.image.version": "1.0.0", "org.opencontainers.image.source": "https://fixture.invalid/fixtures/application"}
    index = {"schemaVersion": 2, "mediaType": "application/vnd.oci.image.index.v1+json", "manifests": [
      {"digest": member, "platform": {"os": "linux", "architecture": "amd64"}}, {"digest": member, "platform": {"os": "linux", "architecture": "arm64"}}]}
    manifest = {"schemaVersion": 2, "config": {"digest": content}}
    for value in ({"os": "linux", "architecture": "riscv64", "config": {"Labels": labels}},
                  {"os": "linux", "architecture": "amd64", "config": {"Labels": labels | {"org.opencontainers.image.version": "2.0.0"}}}):
      with patch.object(d, "registry", side_effect=[(root, index), (member, manifest), (content, value)]), self.assertRaises(d.Refusal):
        d.image("unused", config, root, commit)
    for bad in ({}, index | {"manifests": []}, index | {"schemaVersion": 1}):
      with patch.object(d, "registry", return_value=(root, bad)), self.assertRaises(d.Refusal):
        d.image("unused", config, root, commit)

  def test_package_identity_and_null_are_strict(self):
    d = self.data
    config = {"gitea": "https://fixture.invalid", "repository": "fixtures/application", "image": "fixture.invalid/fixtures/application-hello"}
    repository = {"id": 17, "name": "application", "full_name": "fixtures/application", "owner": {"login": "fixtures"}}
    package = {"id": 23, "name": "application-hello", "type": "container", "version": "latest", "owner": {"login": "fixtures"}, "repository": None}
    invalid = [None, [], {}, package | {"id": True}, package | {"type": "generic"}, package | {"name": "foreign"},
               package | {"version": "other"}, package | {"owner": {"login": "foreign"}}, package | {"repository": False},
               {k: v for k, v in package.items() if k != "repository"}]
    for record in invalid:
      with patch.object(d, "api_document", return_value=record), self.assertRaises(d.Refusal):
        d.package_linked("unused", config, "unused", 17)
    with patch.object(d, "api_document", return_value=package):
      self.assertFalse(d.package_linked("unused", config, "unused", 17))
    for override in ({"id": True}, {"id": 0}, {"id": 99}, {"full_name": "foreign/application"}, {"name": "foreign"}):
      with patch.object(d, "api_document", return_value=package | {"repository": repository | override}), self.assertRaises(d.Refusal):
        d.package_linked("unused", config, "unused", 17)


if __name__ == "__main__":
  unittest.main()

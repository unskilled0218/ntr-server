"""Historical association and observable secret-safe progress contracts."""

import os
from pathlib import Path
import selectors
import subprocess
import sys
import time
import unittest
from unittest.mock import patch

from test_bash_release import Fixture, ROOT, WRITES, helper_module


class RecoveryTests(unittest.TestCase):
  def assert_only_association(self, f):
    self.assertFalse(set(f.events()) & (WRITES - {"package-link"}), f.events())
    self.assertFalse(any(e.startswith(("plugin-info:", "registry:rt-", "login")) or e == "builder" for e in f.events()), f.events())

  def remote_refs(self, f):
    return f.git("ls-remote", "origin")

  def test_old_publication_recovers_under_new_tool_without_docker_and_repeats_read_only(self):
    d = helper_module()
    with Fixture() as f:
      args = f.prepare_recovery()
      historical = d.settings_inputs(lambda name, limit=4194304: d.blob(f.root, args[5], name, limit))
      self.assertEqual(historical["version"], "1.0.0")
      self.assertEqual(helper_module().settings(f.root)["version"], "1.0.1")
      before = f.state()
      remote = self.remote_refs(f)
      local = f.git("show-ref")
      config = (f.root / ".git/config").read_bytes()
      for lost in (False, True):
        with self.subTest(lost=lost):
          f.reset_association()
          f.change(link_fault="lost" if lost else None)
          result = f.release(*args)
          self.assertEqual(result.returncode, 0, result.stderr)
          self.assertIn("Association verified: v1.0.0", result.stdout)
          self.assertNotIn("Publication verified:", result.stdout)
          self.assertIn("Historical publication verified", result.stderr)
          self.assertIn("http:GET:/api/v1/version", f.events())
          self.assertEqual(f.events().count("package-link"), 1)
          self.assert_only_association(f)
          for field in ("objects", "aliases", "releases"):
            self.assertEqual(f.state()[field], before[field])
          self.assertEqual(self.remote_refs(f), remote)
          self.assertEqual(f.git("show-ref"), local)
          self.assertEqual(f.git("status", "--porcelain"), "")
          self.assertEqual((f.root / ".git/config").read_bytes(), config)
          if lost:
            self.assertIn("Link response lost", result.stderr)
          f.clear_events()
          result = f.release(*args)
          self.assertEqual(result.returncode, 0, result.stderr)
          self.assertFalse(set(f.events()) & WRITES)
          self.assert_only_association(f)
          self.assertEqual(self.remote_refs(f), remote)

  def test_recovery_argument_errors_are_effect_free(self):
    with Fixture() as f:
      f.token.unlink()
      valid = ("--associate-existing", "v1.0.0", "--expected-tag-oid", "a" * 40, "--expected-source", "b" * 40)
      variants = [valid[:i] for i in range(1, 6)] + [valid + ("extra",),
        (valid[0], "v01.0.0", *valid[2:]), (*valid[:3], "bad", *valid[4:]),
        (*valid[:4], "--expected-tag-oid", valid[5]), (*valid[:5], "B" * 40)]
      for arguments in variants:
        with self.subTest(arguments=arguments):
          result = f.release(*arguments)
          self.assertEqual(result.returncode, 2, result.stderr)
          self.assertIn("Usage:", result.stderr)
          self.assertEqual(f.events(), [])
          self.assertFalse((f.root / ".template-operation.lock").exists())

  def test_recovery_refuses_wrong_pins_partial_states_and_changed_evidence(self):
    with Fixture() as f:
      args = f.prepare_recovery()
      initial = f.state()
      for fault in ("oid", "source", "no_release", "no_tag", "body", "objects", "latest", "dirty", "wrong_remote", "wrong_push_remote", "wrong_main"):
        with self.subTest(fault=fault):
          f.change(**initial)
          f.clear_events()
          options = list(args)
          if fault == "oid":
            options[3] = "a" * 40
          elif fault == "source":
            options[5] = f.git("rev-parse", "HEAD")
          elif fault == "no_release":
            f.change(releases=[])
          elif fault == "no_tag":
            f.git("push", "--quiet", "origin", ":refs/tags/v1.0.0")
          elif fault == "body":
            f.change(releases=[initial["releases"][0] | {"body": "Foreign"}])
          elif fault == "objects":
            f.change(objects={})
          elif fault == "latest":
            f.change(aliases=initial["aliases"] | {"latest": "sha256:" + "0" * 64})
          elif fault == "dirty":
            (f.root / "notes.md").write_text("Changed tool input")
          else:
            f.change(**{fault: True})
          result = f.release(*options)
          self.assertNotEqual(result.returncode, 0, result.stderr)
          self.assertFalse(set(f.events()) & WRITES, f.events())
          self.assertNotIn("Association verified:", result.stdout)
          if fault == "dirty":
            (f.root / "notes.md").write_text("# Release v1.0.0\n\nProject changes.\n")
          if fault == "no_tag":
            f.git("push", "--quiet", "origin", args[3] + ":refs/tags/v1.0.0")
          for flag in ("wrong_remote", "wrong_push_remote", "wrong_main"):
            f.change(**{flag: False})

  def test_recovery_rejects_canonical_but_foreign_historical_bindings(self):
    d = helper_module()
    with Fixture() as f:
      args = f.prepare_recovery()
      historical = d.settings_inputs(lambda name, limit=4194304: d.blob(f.root, args[5], name, limit))
      identity = d.inspect_tag(f.root, args[5], args[3], historical)
      for override in ({"config": "sha256:" + "a" * 64}, {"scripts": "sha256:" + "a" * 64},
                       {"notes": identity["notes"] | {"digest": "sha256:" + "a" * 64}}, {"repository": "foreign/project"}):
        with self.subTest(override=override):
          oid = d.create_tag(f.root, identity | override)
          f.git("push", "--quiet", "--force", "origin", oid + ":refs/tags/v1.0.0")
          f.clear_events()
          result = f.release(*args[:3], oid, *args[4:])
          self.assertNotEqual(result.returncode, 0)
          self.assertIn("Conflicting publication identity", result.stderr)
          self.assertFalse(set(f.events()) & WRITES, f.events())

  def test_recovery_conflicts_and_lost_failures_never_replay_or_rollback(self):
    with Fixture() as f:
      args = f.prepare_recovery()
      original = f.state()
      for fault in ("transport", "timeout", "denied", "nonempty", "wrong_status", "ignore", "conflict_after"):
        with self.subTest(fault=fault):
          f.reset_association()
          f.change(link_fault=fault)
          result = f.release(*args)
          self.assertNotEqual(result.returncode, 0)
          self.assertEqual(f.events().count("package-link"), 1)
          self.assert_only_association(f)
          self.assertEqual(f.state()["aliases"], original["aliases"])
          self.assertEqual(f.state()["releases"], original["releases"])
          self.assertNotIn("Association verified:", result.stdout)
      for hidden in (False, True):
        f.reset_association()
        foreign = f.target() | {"id": 99, "name": "private", "full_name": "fixtures/private"}
        f.change(linked_repository=foreign, hide_link=hidden, link_fault=None)
        result = f.release(*args)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(f.state()["linked_repository"], foreign)
        self.assertEqual(f.events().count("package-link"), int(hidden))
        self.assert_only_association(f)

  def test_recovery_checks_drift_immediately_before_and_after_link(self):
    with Fixture() as f:
      args = f.prepare_recovery()
      initial = f.state()
      tool = f.root / "scripts/release-common.sh"
      original_tool = tool.read_bytes()
      for kind in ("change_latest", "change_tag", "change_tool"):
        for read, accepted in ((2, False), (3, True)):
          with self.subTest(kind=kind, read=read):
            f.change(**initial)
            f.reset_association()
            f.change(association_fault={"resource": "package", "read": read, "kind": kind})
            result = f.release(*args)
            self.assertIn("association-read-injected:" + kind, f.events(), result.stderr)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(f.events().count("package-link"), int(accepted))
            self.assertEqual(f.state().get("linked_repository"), f.target() if accepted else None)
            self.assert_only_association(f)
            self.assertNotIn("Association verified:", result.stdout)
            tool.write_bytes(original_tool)
            if kind == "change_tag":
              f.git("push", "--quiet", "origin", args[3] + ":refs/tags/v1.0.0")

  def test_normal_completed_publication_does_not_adopt_changed_scripts(self):
    with Fixture() as f:
      f.prepare_recovery(bump_version=False)
      # Recovery needs no Docker; restore the fake only to exercise normal preflight.
      (f.bin / "docker").write_bytes((ROOT / "tests/fixtures/bash_release_ports.py").read_bytes())
      (f.bin / "docker").chmod(0o755)
      result = f.release()
      self.assertNotEqual(result.returncode, 0)
      self.assertIn("conflicting startup state", result.stderr)
      self.assertFalse(set(f.events()) & WRITES, f.events())

  def test_recovery_refuses_non_ancestor_and_noncanonical_tag(self):
    d = helper_module()
    with Fixture() as f:
      args = f.prepare_recovery()
      historical = d.settings_inputs(lambda name, limit=4194304: d.blob(f.root, args[5], name, limit))
      identity = d.inspect_tag(f.root, args[5], args[3], historical)
      independent = f.git("commit-tree", f.git("rev-parse", args[5] + "^{tree}"), "-m", "Independent source")
      oid = d.create_tag(f.root, identity | {"commit": independent})
      f.git("push", "--quiet", "--force", "origin", oid + ":refs/tags/v1.0.0")
      f.clear_events()
      result = f.release(*args[:3], oid, args[4], independent)
      self.assertNotEqual(result.returncode, 0)
      self.assertTrue(any("merge-base" in e and e.startswith("git-command-failed:1:") for e in f.events()), result.stderr)
      self.assertFalse(set(f.events()) & WRITES)
      raw = d.tag_bytes(identity).replace(b'"format":', b' "format":')
      oid = subprocess.check_output(["/usr/bin/git", "-C", str(f.root), "mktag"], input=raw, env=f.environment).decode().strip()
      f.git("push", "--quiet", "--force", "origin", oid + ":refs/tags/v1.0.0")
      f.clear_events()
      result = f.release(*args[:3], oid, *args[4:])
      self.assertNotEqual(result.returncode, 0)
      self.assertIn("Different canonical tag bytes", result.stderr)
      self.assertFalse(set(f.events()) & WRITES)

  def test_historical_input_validation_is_bounded_and_never_executes_scripts(self):
    d = helper_module()
    for kind in ("symlink", "missing", "oversized", "unsafe_notes"):
      with self.subTest(kind=kind), Fixture() as f:
        if kind == "symlink":
          (f.root / "scripts/release-common.sh").unlink()
          (f.root / "scripts/release-common.sh").symlink_to("../VERSION")
        elif kind == "missing":
          (f.root / "notes.md").unlink()
        elif kind == "oversized":
          (f.root / "notes.md").write_bytes(b"x" * 65537)
        else:
          import json
          path = f.root / "scripts/release.json"
          path.write_text(json.dumps(json.loads(path.read_text()) | {"notes": "../token"}))
        f.commit()
        commit = f.git("rev-parse", "HEAD")
        with self.assertRaises(d.Refusal):
          d.settings_inputs(lambda name, limit=4194304: d.blob(f.root, commit, name, limit))
    with Fixture() as f:
      helper = f.root / "scripts/release-data.py"
      marker = f.base / "historical-code-was-executed"
      guard = '\nif os.environ.get("HISTORICAL_EXEC_SENTINEL") == "1":\n  Path(' + repr(str(marker)) + ').write_text("unsafe")\n\n'
      helper.write_text(helper.read_text().replace('if __name__ == "__main__":', guard + 'if __name__ == "__main__":'))
      f.commit()
      args = f.prepare_recovery()
      helper.write_text(helper.read_text().replace(guard, ""))
      f.commit()
      f.environment["HISTORICAL_EXEC_SENTINEL"] = "1"
      result = f.release(*args)
      self.assertEqual(result.returncode, 0, result.stderr)
      self.assertFalse(marker.exists(), "Historical script executed instead of being read as data")
      self.assert_only_association(f)

  def test_recovery_lock_and_signal_cleanup_do_not_require_docker(self):
    import fcntl
    with Fixture() as f:
      args = f.prepare_recovery()
      lock_path = f.root / ".template-operation.lock"
      with lock_path.open() as held:
        fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
        result = f.release(*args)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(f.events(), [])
      f.change(hang_http=True)
      process = subprocess.Popen(["bash", str(f.root / "scripts/release.sh"), *args], cwd=f.base, env=f.environment,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
      try:
        deadline = time.monotonic() + 30
        while "http-hang-injected" not in f.events() and process.poll() is None and time.monotonic() < deadline:
          time.sleep(.02)
        self.assertIn("http-hang-injected", f.events())
        process.terminate()
        stdout, stderr = process.communicate(timeout=10)
        self.assertNotEqual(process.returncode, 0)
        self.assertNotIn(b"Association verified:", stdout)
        self.assertNotIn(b"SENTINEL_PRIVATE_TOKEN", stdout + stderr)
        self.assertEqual(list(f.base.glob("bash-release.*")), [])
        with lock_path.open() as lock:
          fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        self.assertFalse(set(f.events()) & WRITES)
      finally:
        if process.poll() is None:
          process.kill()
        process.communicate()


class ProgressTests(unittest.TestCase):
  def test_fresh_28_publication_logs_phases_without_leaking_tool_output(self):
    with Fixture() as f:
      f.change(gitea_version="28.2.0", secret_output=True)
      result = f.release()
      self.assertEqual(result.returncode, 0, result.stderr)
      self.assertIn("secret-output-injected", f.events())
      phases = ["Validating local", "Checking source", "Reading complete", "Building staging", "Publishing the exact",
                "Promoting the verified", "Creating the notes-only", "Verifying accepted", "Publication verified; checking", "Attempting one package link", "Package association verified"]
      positions = [result.stderr.index(phase) for phase in phases]
      self.assertEqual(positions, sorted(positions))
      self.assertIn("Publication verified: v1.0.0", result.stdout)
      self.assertNotIn("[release +", result.stdout)
  def test_exact_supported_contracts_and_heartbeat_cadence(self):
    d = helper_module()
    for version in ("1.25.0", "1.25.99", "28.2.0"):
      d.package_contract({"version": version})
    for value in (None, {}, {"version": 28}, {"version": "28.2.1"}, {"version": "28.2.0-dev"},
                  {"version": "28.2.0+custom"}, {"version": "1.25.00"}):
      with self.subTest(value=value), self.assertRaises(d.Refusal):
        d.package_contract(value)
    with patch.object(d, "progress") as progress:
      heartbeat = d.Heartbeat(100)
      for now in (100, 109.9, 110, 110.1, 119.9, 120, 120.1):
        heartbeat.tick(now)
      self.assertEqual(progress.call_count, 2)
      progress.assert_called_with("waiting")

  def test_heartbeat_is_visible_before_command_timeout_without_secret_output(self):
    helper = ROOT / "scripts/release-data.py"
    code = '''import importlib.util,sys
spec=importlib.util.spec_from_file_location('data',sys.argv[1]); d=importlib.util.module_from_spec(spec); spec.loader.exec_module(d)
def fast(self, started): self.next=started
d.Heartbeat.__init__=fast
try: d.command([sys.executable,'-c',"import sys,time; print('SENTINEL_PRIVATE_TOKEN',flush=True); print('UNRELATED_AUTH_SENTINEL',file=sys.stderr,flush=True); time.sleep(5)"],seconds=.5)
except d.CommandFailed: sys.exit(2)
'''
    process = subprocess.Popen([sys.executable, "-B", "-c", code, str(helper)], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
      with selectors.DefaultSelector() as selector:
        selector.register(process.stderr, selectors.EVENT_READ)
        self.assertTrue(selector.select(2), "No prompt heartbeat")
        first = process.stderr.readline()
      self.assertIn(b"Still waiting", first)
      self.assertIsNone(process.poll(), "Heartbeat was buffered until completion")
      stdout, stderr = process.communicate(timeout=3)
      self.assertEqual(process.returncode, 2)
      self.assertNotIn(b"SENTINEL_PRIVATE_TOKEN", stdout + first + stderr)
      self.assertNotIn(b"UNRELATED_AUTH_SENTINEL", stdout + first + stderr)
    finally:
      if process.poll() is None:
        process.kill()
      process.communicate()

  def test_publication_progress_is_prompt_and_signal_cleanup_hides_tool_secrets(self):
    with Fixture() as f:
      f.change(hang_build=True, secret_output=True)
      process = subprocess.Popen(["bash", str(f.root / "scripts/release.sh")], cwd=f.base, env=f.environment,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
      collected = bytearray()
      try:
        with selectors.DefaultSelector() as selector:
          selector.register(process.stderr, selectors.EVENT_READ)
          deadline = time.monotonic() + 30
          while b"Building staging content" not in collected and time.monotonic() < deadline:
            self.assertTrue(selector.select(5), collected.decode())
            chunk = os.read(process.stderr.fileno(), 65536)
            self.assertTrue(chunk, collected.decode())
            collected.extend(chunk)
        self.assertIn(b"Building staging content", collected)
        self.assertIsNone(process.poll())
        deadline = time.monotonic() + 5
        while "build-secret-output-injected" not in f.events() and process.poll() is None and time.monotonic() < deadline:
          time.sleep(.02)
        self.assertIn("build-secret-output-injected", f.events())
        process.terminate()
        stdout, stderr = process.communicate(timeout=10)
        self.assertNotEqual(process.returncode, 0)
        self.assertNotIn(b"SENTINEL_PRIVATE_TOKEN", stdout + collected + stderr)
        self.assertNotIn(b"UNRELATED_AUTH_SENTINEL", stdout + collected + stderr)
        self.assertNotIn(b"Publication verified:", stdout)
        self.assertEqual(list(f.base.glob("bash-release.*")), [])
      finally:
        if process.poll() is None:
          process.kill()
        process.communicate()


if __name__ == "__main__":
  unittest.main()

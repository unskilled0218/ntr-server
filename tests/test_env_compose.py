"""Render the single Compose file without real secrets or container startup."""

import json
import pathlib
import shutil
import subprocess
import tempfile
import unittest


class DotenvComposeTests(unittest.TestCase):
  def setUp(self):
    if not shutil.which("docker"):
      self.skipTest("Docker Compose is unavailable")
    version = subprocess.run(["docker", "compose", "version"], capture_output=True)
    if version.returncode:
      self.skipTest("Docker Compose is unavailable")
    self.root = pathlib.Path(__file__).resolve().parents[1]

  def test_env_file_is_passed_with_read_only_private_paths_and_persistent_data(self):
    root = self.root
    self.assertFalse((root / "docker-compose.ha.yml").exists())
    with tempfile.TemporaryDirectory(prefix="ntr-env-compose-") as name:
      project = pathlib.Path(name)
      (project / ".env").write_text((root / ".env.example").read_text())
      for directory in ["secrets", "certificates", "data"]:
        (project / directory).mkdir()
      base = ["docker", "compose", "--project-directory", name, "-f", str(root / "docker-compose.yml")]
      rendered = subprocess.run(base + ["config", "--format", "json"], check=True, capture_output=True, text=True)
      service = json.loads(rendered.stdout)["services"]["ntr-server"]
      self.assertEqual(service["environment"]["TASK_BACKEND"], "homeassistant")
      self.assertEqual(service["environment"]["HA_TOKEN_FILE"], "./secrets/ha-token")
      self.assertEqual(service["environment"]["HA_CA_FILE"], "./certificates/homeassistant.lanCA.crt")
      self.assertEqual(service["environment"]["HA_METADATA_FILE"], "./ha-metadata.json")
      self.assertNotIn("HA_TOKEN", service["environment"])
      volumes = {v["target"]: v for v in service["volumes"]}
      self.assertEqual(volumes["/data"]["source"], str(project / "data"))
      for directory in ["secrets", "certificates"]:
        mount = volumes[f"/data/{directory}"]
        self.assertEqual(mount["source"], str(project / directory))
        self.assertTrue(mount["read_only"])
        self.assertFalse(mount["bind"]["create_host_path"])
      self.assertEqual(service["ports"][0]["host_ip"], "127.0.0.1")
      self.assertEqual(service["image"], "git.lan/ntr/ntr-server:latest")
      self.assertEqual(service["user"], "1000:1000")
      self.assertEqual(service["restart"], "unless-stopped")
      self.assertEqual(service["healthcheck"]["test"], ["CMD", "curl", "-fsS", "http://127.0.0.1:8080/api/health"])
      # File-backed tasks use the same file; no token or CA files are needed.
      (project / ".env").write_text("TASK_BACKEND=file\n")
      file_mode = json.loads(subprocess.run(base + ["config", "--format", "json"], check=True, capture_output=True, text=True).stdout)
      self.assertEqual(file_mode["services"]["ntr-server"]["environment"], {"TASK_BACKEND": "file"})

  def test_missing_dotenv_is_rejected(self):
    with tempfile.TemporaryDirectory(prefix="ntr-env-compose-") as name:
      result = subprocess.run(["docker", "compose", "--project-directory", name, "-f", str(self.root / "docker-compose.yml"), "config", "--quiet"], capture_output=True, text=True)
      self.assertNotEqual(result.returncode, 0)
      self.assertIn(".env", result.stderr)


if __name__ == "__main__":
  unittest.main()

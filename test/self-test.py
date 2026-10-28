#!/usr/bin/env python3
"""S6-Testwerkzeuge prüfen, ohne Cargo-Build, Hub, Modell oder echte Nutzerdaten."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("metrics", ROOT / "gate-metrics.py")
metrics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(metrics)


def diagnostic(message="first", line=1, target="copi"):
    return {"reason": "compiler-message", "target": {"name": target}, "message": {
        "level": "warning", "message": message, "code": {"code": "unused"},
        "spans": [{"file_name": "src/main.rs", "line_start": line, "is_primary": True}]}}


def executable(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)
    path.chmod(0o755)
    return path


class MetricsTests(unittest.TestCase):
    def test_deduplication_and_locations(self):
        lines = ["not JSON", "42", json.dumps({"reason": "build-finished"}),
                 json.dumps(diagnostic()), json.dumps(diagnostic(target="cocopi")),
                 json.dumps(diagnostic(line=2))]
        self.assertEqual(metrics.warnings(lines), (3, 2))

    def test_threshold_and_invalid_baselines(self):
        with tempfile.TemporaryDirectory(prefix="s6-metrics.") as directory:
            d = Path(directory)
            log = d / "clippy.log"
            log.write_text(json.dumps(diagnostic()) + "\n")
            baseline = d / "baseline.json"
            for value, rc in [({"schema": 1, "unique_warning_count": 1}, 0),
                              ({"schema": 1, "unique_warning_count": 0}, 1),
                              ({"schema": 2, "unique_warning_count": 1}, 2),
                              ({"schema": True, "unique_warning_count": 1}, 2),
                              ({"schema": 1, "unique_warning_count": True}, 2),
                              ({"schema": 1, "unique_warning_count": -1}, 2),
                              ({}, 2), ([], 2), (None, 2)]:
                with self.subTest(value=value):
                    baseline.write_text(json.dumps(value))
                    result = subprocess.run(["python3", str(ROOT / "gate-metrics.py"),
                                             str(log), str(baseline)], capture_output=True, text=True)
                    self.assertEqual(result.returncode, rc, result.stderr)
                    self.assertEqual(json.loads(result.stdout)["unique"], 1)
            for content in ("{",):
                baseline.write_text(content)
                self.assertEqual(subprocess.run(["python3", str(ROOT / "gate-metrics.py"),
                                                 str(log), str(baseline)], capture_output=True).returncode, 2)
            baseline.unlink()
            self.assertEqual(subprocess.run(["python3", str(ROOT / "gate-metrics.py"),
                                             str(log), str(baseline)], capture_output=True).returncode, 2)


class ScriptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="s6-tools.")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.home = self.base / "real-home"
        self.home.mkdir()
        self.sentinel = self.home / "preserve"
        self.sentinel.write_text("unchanged")
        self.env = dict(os.environ, TMPDIR=str(self.base), HOME=str(self.home),
                        PYTHONDONTWRITEBYTECODE="1")

    def run_script(self, script, *args, **extra):
        result = subprocess.run(["bash", str(ROOT / script), *map(str, args)],
                                env=dict(self.env, **extra), capture_output=True,
                                text=True, timeout=15)
        self.assertEqual(self.sentinel.read_text(), "unchanged")
        self.assertEqual(sorted(p.name for p in self.home.iterdir()), ["preserve"])
        return result

    def test_spawn_observation_positive_and_negative_paths(self):
        for mode in ("clean", "pi", "node", "missing-shot", "exit", "timeout"):
            with self.subTest(mode=mode):
                app = executable(self.base / (mode + ".sh"), "#!/bin/sh\n" + {
                    "clean": 'test "$COPI_SCENE" = remote || exit 8\nprintf image > "$2"\n',
                    "pi": 'pi || :\nprintf image > "$2"\n',
                    "node": 'node || :\nprintf image > "$2"\n',
                    "missing-shot": 'exit 0\n', "exit": 'exit 5\n',
                    "timeout": 'sleep 5\n'}[mode])
                result = self.run_script("no-local-spawn.sh", app, "remote", NO_LOCAL_SPAWN_TIMEOUT="1")
                self.assertEqual(result.returncode, 0 if mode == "clean" else 1, result.stderr)
                self.assertEqual(len(result.stdout.splitlines()), 1)
                if mode == "clean":
                    self.assertIn("kein Remote-only-Nachweis", result.stdout)
        self.assertEqual(self.run_script("no-local-spawn.sh").returncode, 2)
        self.assertEqual(self.run_script("no-local-spawn.sh", self.base / "absent").returncode, 2)

    def setup_gate(self):
        worktree = self.base / "worktree"
        (worktree / "copi").mkdir(parents=True)
        (worktree / "copi/Cargo.toml").write_text("# Stub only\n")
        (worktree / "ui").mkdir()
        (worktree / "scripts").mkdir()
        for name in ("a", "b"):
            (worktree / f"ui/{name}.js").write_text("// stub\n")
            (worktree / f"scripts/test-{name}.cjs").write_text("// stub\n")
        hub = self.base / "hub"
        (hub / "test").mkdir(parents=True)
        (hub / "test/smoke.mjs").write_text("// stub\n")
        executable(hub / "test/fake-pi.mjs", "#!/bin/sh\nexit 0\n")
        tools = self.base / "tools"
        calls = self.base / "calls"
        warning = json.dumps(diagnostic())
        executable(tools / "cargo", f'''#!/bin/bash
printf 'cargo %s target=%s home=%s\n' "$*" "$CARGO_TARGET_DIR" "$HOME" >> '{calls}'
if [[ $1 == clippy ]]; then printf '%s\n' '{warning}'; fi
if [[ -f "$PWD/fail-$1" ]]; then exit 7; fi
exit 0
''')
        executable(tools / "rustc", "#!/bin/sh\nprintf 'rustc stub\\n'\n")
        executable(tools / "node", f'''#!/bin/bash
printf 'node %s\n' "$*" >> '{calls}'
for arg in "$@"; do [[ $arg != *a.js && $arg != *test-a.cjs ]] || [[ ! -f "$PWD/fail-js" ]] || exit 9; done
exit 0
''')
        for repository in (worktree, hub):
            subprocess.run(["git", "init", "-q", str(repository)], check=True, capture_output=True)
            subprocess.run(["git", "-C", str(repository), "-c", "user.name=S6 test",
                            "-c", "user.email=s6@example.invalid", "-c", "commit.gpgsign=false",
                            "-c", "core.hooksPath=/dev/null", "commit", "-q", "--allow-empty", "-m", "stub"],
                           check=True, capture_output=True)
        self.env["PATH"] = str(tools) + ":" + os.environ["PATH"]
        self.env["CARGO_TARGET_DIR"] = str(self.base / "foreign-target")
        return worktree, hub, calls

    def test_gate_green_skipped_and_lock(self):
        worktree, hub, calls = self.setup_gate()
        result = self.run_script("v3-gate.sh", worktree, hub)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(len(result.stdout.splitlines()), 7)
        self.assertTrue(all(line.startswith("PASS") for line in result.stdout.splitlines()))
        entries = calls.read_text()
        self.assertIn("--offline --locked --workspace --features copi/cocopi", entries)
        self.assertIn(f"target={self.base}/target-worktree", entries)
        self.assertNotIn("foreign-target", entries)
        self.assertFalse((self.base / "target-worktree/.v3-gate-lock").exists())
        result = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("SKIP hub-smoke", result.stdout)
        lock = self.base / "target-worktree/.v3-gate-lock"
        lock.mkdir()
        result = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(result.returncode, 2)
        self.assertTrue(lock.exists())  # Fremden Lock nicht entfernen.

    def test_gate_failures_continue_all_files(self):
        worktree, hub, calls = self.setup_gate()
        for command in ("fmt", "clippy", "test", "check"):
            (worktree / f"copi/fail-{command}").touch()
        (worktree / "fail-js").touch()
        result = self.run_script("v3-gate.sh", worktree, hub)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(len(result.stdout.splitlines()), 7)
        self.assertEqual(sum(line.startswith("FAIL") for line in result.stdout.splitlines()), 6)
        self.assertIn("PASS hub-smoke", result.stdout)
        self.assertIn("node --check ui/b.js", calls.read_text())
        self.assertIn("node scripts/test-b.cjs", calls.read_text())
        self.assertFalse((self.base / "target-worktree/.v3-gate-lock").exists())

    def test_gate_rejects_empty_js_globs(self):
        worktree, _, _ = self.setup_gate()
        for directory in ("ui", "scripts"):
            for path in (worktree / directory).iterdir():
                path.unlink()  # Nur selbst angelegte Stub-Dateien.
        result = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("FAIL js-syntax", result.stdout)
        self.assertIn("FAIL js-tests", result.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)

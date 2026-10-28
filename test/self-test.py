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
checks_spec = importlib.util.spec_from_file_location("checks", ROOT / "gate-checks.py")
checks = importlib.util.module_from_spec(checks_spec)
checks_spec.loader.exec_module(checks)


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
        (worktree / "copi/main.rs").write_text("fn main() {}\n")
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
        executable(tools / "rustfmt", f'''#!/bin/bash
printf 'rustfmt %s\\n' "$*" >> '{calls}'
[[ ! -f "$PWD/fail-fmt" ]]
''')
        executable(tools / "node", f'''#!/bin/bash
printf 'node %s\n' "$*" >> '{calls}'
for arg in "$@"; do
    [[ $arg != *a.js && $arg != *test-a.cjs ]] || [[ ! -f "$PWD/fail-js" ]] || exit 9
    [[ $arg != *test-statusline-net.cjs ]] || [[ ! -f "$PWD/fail-known" ]] || exit 9
    [[ $arg != --test ]] || [[ ! -f "$PWD/fail-mobile" ]] || exit 9
done
exit 0
''')
        for repository in (worktree, hub):
            subprocess.run(["git", "init", "-q", str(repository)], check=True, capture_output=True)
            subprocess.run(["git", "-C", str(repository), "add", "."], check=True, capture_output=True)
            subprocess.run(["git", "-C", str(repository), "-c", "user.name=S6 test",
                            "-c", "user.email=s6@example.invalid", "-c", "commit.gpgsign=false",
                            "-c", "core.hooksPath=/dev/null", "commit", "-q", "--allow-empty", "-m", "stub"],
                           check=True, capture_output=True)
        subprocess.run(["git", "-C", str(worktree), "branch", "cocopi-v3"], check=True)
        (worktree / "copi/main.rs").write_text("// changed\nfn main() {}\n")
        self.env["PATH"] = str(tools) + ":" + os.environ["PATH"]
        self.env["CARGO_TARGET_DIR"] = str(self.base / "foreign-target")
        return worktree, hub, calls

    def test_gate_green_skipped_and_lock(self):
        worktree, hub, calls = self.setup_gate()
        result = self.run_script("v3-gate.sh", worktree, hub)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(len(result.stdout.splitlines()), 9)
        self.assertEqual(sum(line.startswith("PASS") for line in result.stdout.splitlines()), 7)
        self.assertEqual(sum(line.startswith("SKIP") for line in result.stdout.splitlines()), 2)
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
        for command in ("clippy", "test", "check"):
            (worktree / f"copi/fail-{command}").touch()
        (worktree / "fail-fmt").touch()
        (worktree / "fail-js").touch()
        result = self.run_script("v3-gate.sh", worktree, hub)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(len(result.stdout.splitlines()), 9)
        self.assertEqual(sum(line.startswith("FAIL") for line in result.stdout.splitlines()), 6)
        self.assertIn("PASS hub-smoke", result.stdout)
        self.assertIn("node --check ui/b.js", calls.read_text())
        self.assertIn("node scripts/test-b.cjs", calls.read_text())
        self.assertFalse((self.base / "target-worktree/.v3-gate-lock").exists())

    def test_fmt_selection_merge_base_and_working_files(self):
        worktree, _, calls = self.setup_gate()
        unchanged = worktree / "unchanged.rs"
        unchanged.write_text("fn base() {}\n")
        deleted = worktree / "deleted.rs"
        deleted.write_text("fn deleted() {}\n")
        subprocess.run(["git", "-C", str(worktree), "add", "unchanged.rs", "deleted.rs"], check=True)
        subprocess.run(["git", "-C", str(worktree), "-c", "user.name=S6 test",
                        "-c", "user.email=s6@example.invalid", "-c", "commit.gpgsign=false",
                        "-c", "core.hooksPath=/dev/null", "commit", "-qm", "format base"], check=True)
        subprocess.run(["git", "-C", str(worktree), "branch", "-f", "cocopi-v3"], check=True)
        # Branch-Ref darf vorausliegen: Auswahl muss die gemeinsame Basis benutzen.
        subprocess.run(["git", "-C", str(worktree), "branch", "alternate-base"], check=True)
        subprocess.run(["git", "-C", str(worktree), "checkout", "-q", "cocopi-v3"], check=True)
        (worktree / "only-on-base.rs").write_text("fn other_branch() {}\n")
        subprocess.run(["git", "-C", str(worktree), "add", "only-on-base.rs"], check=True)
        subprocess.run(["git", "-C", str(worktree), "-c", "user.name=S6 test",
                        "-c", "user.email=s6@example.invalid", "-c", "commit.gpgsign=false",
                        "-c", "core.hooksPath=/dev/null", "commit", "-qm", "ahead base"], check=True)
        subprocess.run(["git", "-C", str(worktree), "checkout", "-q", "-"], check=True)
        subprocess.run(["git", "-C", str(worktree), "add", "copi/main.rs"], check=True)
        subprocess.run(["git", "-C", str(worktree), "-c", "user.name=S6 test",
                        "-c", "user.email=s6@example.invalid", "-c", "commit.gpgsign=false",
                        "-c", "core.hooksPath=/dev/null", "commit", "-qm", "changed Rust"], check=True)
        deleted.unlink()
        (worktree / "new file.rs").write_text("fn new_file() {}\n")
        base, selected, total = checks.fmt_selection(worktree, "cocopi-v3")
        self.assertEqual(selected, ["copi/main.rs", "new file.rs"])
        self.assertEqual(total, 3)
        self.assertEqual(base, subprocess.check_output(
            ["git", "-C", str(worktree), "rev-parse", "alternate-base"], text=True).strip())
        result = self.run_script("v3-gate.sh", worktree, V3_GATE_BASE="alternate-base")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        entries = calls.read_text()
        self.assertIn("rustfmt --check --edition 2021 --config skip_children=true -- copi/main.rs", entries)
        self.assertIn("-- new file.rs", entries)
        self.assertNotIn("unchanged.rs", entries)
        self.assertNotIn("deleted.rs", entries)
        self.assertNotIn("only-on-base.rs", entries)
        result = self.run_script("v3-gate.sh", worktree, V3_GATE_BASE="missing-ref")
        self.assertEqual(result.returncode, 1)
        self.assertIn("FAIL fmt", result.stdout)

    def test_fmt_no_changes_does_not_invoke_rustfmt(self):
        worktree, _, calls = self.setup_gate()
        (worktree / "copi/main.rs").write_text("fn main() {}\n")
        result = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(result.returncode, 0)
        self.assertIn("PASS fmt | INFO Rust-Dateien gesamt=1; ausgewählt=0;", result.stdout)
        self.assertNotIn("rustfmt ", calls.read_text())

    def test_js_baseline_known_new_and_resolved(self):
        worktree, _, _ = self.setup_gate()
        (worktree / "scripts/test-statusline-net.cjs").write_text("// known stub\n")
        (worktree / "fail-known").touch()
        command = ["python3", str(ROOT / "gate-checks.py"), "js", str(worktree),
                   str(ROOT / "js-baseline.txt")]
        result = subprocess.run(command, env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("KNOWN scripts/test-statusline-net.cjs", result.stdout)
        self.assertNotIn("aus Baseline streichen", result.stdout)
        gate = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(gate.returncode, 0, gate.stdout + gate.stderr)
        self.assertIn("KNOWN js-tests", gate.stdout)
        self.assertNotIn("FAIL js-tests", gate.stdout)
        (worktree / "fail-js").touch()
        result = subprocess.run(command, env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("FAIL scripts/test-a.cjs", result.stdout)
        self.assertIn("PASS scripts/test-b.cjs", result.stdout)
        self.assertIn("KNOWN scripts/test-statusline-net.cjs", result.stdout)
        gate = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(gate.returncode, 1)
        self.assertIn("FAIL js-tests", gate.stdout)
        (worktree / "fail-js").unlink()
        (worktree / "fail-known").unlink()
        result = subprocess.run(command, env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn("jetzt grün – aus Baseline streichen", result.stdout)
        baseline = self.base / "invalid-baseline.txt"
        baseline.write_text("../bad.cjs\n")
        result = subprocess.run(command[:-1] + [str(baseline)], env=self.env, capture_output=True)
        self.assertEqual(result.returncode, 2)

    def test_optional_mobile_and_r2_checks(self):
        worktree, _, calls = self.setup_gate()
        (worktree / "mobile/test").mkdir(parents=True)
        (worktree / "mobile/test/stub.mjs").write_text("// stub\n")
        (worktree / "deploy/r2").mkdir(parents=True)
        r2_test = worktree / "deploy/r2/test_scripts.py"
        r2_test.write_text("print('R2 stub')\n")
        result = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("PASS mobile-tests", result.stdout)
        self.assertIn("PASS r2-tests", result.stdout)
        self.assertIn("node --test mobile/test/stub.mjs", calls.read_text())
        (worktree / "fail-mobile").touch()
        r2_test.write_text("raise SystemExit(7)\n")
        result = self.run_script("v3-gate.sh", worktree)
        self.assertEqual(result.returncode, 1)
        self.assertIn("FAIL mobile-tests", result.stdout)
        self.assertIn("FAIL r2-tests", result.stdout)
        self.assertIn("SKIP hub-smoke", result.stdout)

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

#!/usr/bin/env python3
"""I2c gezielt prüfen: kein leeres Rust-Testfilter-Ergebnis als Erfolg werten."""
import os
from pathlib import Path
import re
import subprocess
import sys


def rust_case(worktree, label, package_args, test_filter):
    command = ["cargo", "test", "--offline", "--locked", *package_args,
               test_filter, "--", "--nocapture"]
    print("COMMAND " + " ".join(command), flush=True)
    result = subprocess.run(command, cwd=worktree / "copi", stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True)
    print(result.stdout, end="", flush=True)
    cases = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)\s*$", result.stdout, re.M)
    skipped = re.search(r"übersprungen|node fehlt|\bSKIP\b", result.stdout, re.I)
    good = result.returncode == 0 and bool(cases) and not skipped
    for name, state in cases:
        passed = state == "ok" and result.returncode == 0 and not skipped
        print(f"{'PASS' if passed else 'FAIL'} i2c-szenarien/{name}", flush=True)
        good = good and passed
    if not cases or skipped or result.returncode:
        print(f"FAIL i2c-szenarien/{label} | exit={result.returncode}; "
              f"Tests={len(cases)}; übersprungen={bool(skipped)}", flush=True)
    return good


def main():
    if len(sys.argv) != 2 or not os.environ.get("COCOPI_PI_HUB"):
        print("FAIL i2c-szenarien | Worktree und COCOPI_PI_HUB erforderlich")
        return 2
    worktree = Path(sys.argv[1]).resolve()
    results = [rust_case(worktree, label, args, test_filter) for label, args, test_filter in [
        ("sync-e2e", ["-p", "copi", "--features", "cocopi", "--bin", "cocopi"],
         "local_node_end_to_end_upload_conflict_import_then_release"),
        ("node-rust-krypto", ["-p", "pi-backend", "--features", "cocopi", "--lib"],
         "node_encrypts_rust_decrypts_"),
        ("grossupload-header", ["-p", "pi-backend", "--features", "cocopi", "--lib"],
         "explicit_upload_header_is_only_sent_after_confirmation"),
    ]]
    env = dict(os.environ, I2_HUB_DIR=os.environ["COCOPI_PI_HUB"])
    result = subprocess.run(["node", "--test", str(Path(__file__).parent / "i2/abholfach.i2c.test.mjs")],
                            cwd=worktree, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True)
    print(result.stdout, end="", flush=True)
    good = (result.returncode == 0 and bool(re.search(r"^# pass [1-9][0-9]*$", result.stdout, re.M))
            and not re.search(r"^# (skipped|todo) [1-9][0-9]*$", result.stdout, re.M))
    print(f"{'PASS' if good else 'FAIL'} i2c-szenarien/zwangscheckout-zweitgeraet-72h", flush=True)
    return int(not (all(results) and good))


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Baselinebezogene Format- und JS-Prüfungen; Aufruf durch v3-gate.sh."""
import argparse
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys


def git_paths(root, *args):
    data = subprocess.check_output(["git", "-C", str(root), *args])
    return {os.fsdecode(path) for path in data.split(b"\0") if path}


def fmt_selection(root, base_ref):
    base = subprocess.check_output(
        ["git", "-C", str(root), "merge-base", "HEAD", base_ref], text=True
    ).strip()
    # Diff gegen die Arbeitskopie: auch uncommittete Änderungen werden geprüft.
    changed = git_paths(root, "diff", "--name-only", "--diff-filter=ACMR", "-z",
                        base, "--", "*.rs")
    new = git_paths(root, "ls-files", "--others", "--exclude-standard", "-z", "--", "*.rs")
    all_paths = git_paths(root, "ls-files", "--cached", "--others", "--exclude-standard",
                          "-z", "--", "*.rs")
    exists = lambda path: (root / path).is_file()
    return base, sorted(filter(exists, changed | new)), len(list(filter(exists, all_paths)))


def check_fmt(root, base_ref):
    base, paths, total = fmt_selection(root, base_ref)
    print(f"INFO Rust-Dateien gesamt={total}; ausgewählt={len(paths)}; merge-base={base}", flush=True)
    failed = False
    for path in paths:
        print(f"CHECK {path}", flush=True)
        # Keine unveränderten Untermodule nachladen: nur die ausgewählte Datei.
        result = subprocess.run(["rustfmt", "--check", "--edition", "2021",
                                 "--config", "skip_children=true", "--", path], cwd=root)
        failed |= result.returncode != 0
    return int(failed)


def read_js_baseline(path):
    known = set()
    for line in path.read_text().splitlines():
        entry = line.strip()
        if not entry or entry.startswith("#"):
            continue
        parts = PurePosixPath(entry).parts
        if (len(parts) != 2 or parts[0] != "scripts" or not parts[1].startswith("test-")
                or not parts[1].endswith(".cjs") or entry in known):
            raise ValueError(f"Ungültiger oder doppelter Baseline-Eintrag: {entry!r}")
        known.add(entry)
    return known


def check_js(root, baseline):
    known = read_js_baseline(baseline)
    paths = sorted(root.glob("scripts/test-*.cjs"))
    if not paths:
        print("FAIL Keine scripts/test-*.cjs gefunden", flush=True)
        return 1
    failed = False
    known_failed = 0
    for path in paths:
        relative = path.relative_to(root).as_posix()
        print(f"TEST {relative}", flush=True)
        result = subprocess.run(["node", relative], cwd=root)
        if result.returncode:
            if relative in known:
                known_failed += 1
                print(f"KNOWN {relative} | exit={result.returncode}", flush=True)
            else:
                failed = True
                print(f"FAIL {relative} | neuer Fehler, exit={result.returncode}", flush=True)
        else:
            print(f"PASS {relative}", flush=True)
            if relative in known:
                print(f"HINWEIS {relative}: jetzt grün – aus Baseline streichen", flush=True)
    existing = {path.relative_to(root).as_posix() for path in paths}
    for removed in sorted(known - existing):
        print(f"HINWEIS {removed}: nicht mehr vorhanden – aus Baseline streichen", flush=True)
    print(f"FILES {len(paths)}; KNOWN {known_failed}", flush=True)
    return int(failed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    fmt = commands.add_parser("fmt")
    fmt.add_argument("root", type=Path)
    fmt.add_argument("base_ref")
    js = commands.add_parser("js")
    js.add_argument("root", type=Path)
    js.add_argument("baseline", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "fmt":
            return check_fmt(args.root.resolve(), args.base_ref)
        return check_js(args.root.resolve(), args.baseline)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Prüfung nicht möglich: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())

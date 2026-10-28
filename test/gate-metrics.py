#!/usr/bin/env python3
"""Clippy JSON: echte Warnungsdiagnosen zählen, Cargo-Zusammenfassungen ignorieren."""
import json
import sys


def warnings(log):
    raw = 0
    unique = set()
    for line in log:
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue  # Cargo schreibt Statusmeldungen auf stderr im selben Log.
        if not isinstance(item, dict) or item.get("reason") != "compiler-message":
            continue
        message = item.get("message", {})
        if message.get("level") != "warning":
            continue
        raw += 1
        # Ohne target/package_id/rendered: identischer include!(main.rs)-Code in
        # copi und cocopi zählt einmal. Orte gehören zum Fingerprint.
        key = {
            "message": message.get("message"),
            "code": message.get("code"),
            "spans": [
                {k: span.get(k) for k in ("file_name", "line_start", "column_start", "line_end", "column_end", "is_primary")}
                for span in message.get("spans", [])
            ],
        }
        unique.add(json.dumps(key, sort_keys=True))
    return raw, len(unique)


def main():
    if len(sys.argv) != 3:
        print("Aufruf: gate-metrics.py <clippy-log> <baseline.json>", file=sys.stderr)
        return 2
    try:
        with open(sys.argv[1], encoding="utf-8") as log:
            raw, unique = warnings(log)
    except OSError as error:
        print(str(error), file=sys.stderr)
        return 2
    try:
        with open(sys.argv[2], encoding="utf-8") as f:
            baseline = json.load(f)
        if not isinstance(baseline, dict):
            raise ValueError("Warnungsbaseline muss ein Objekt sein")
        allowed = baseline["unique_warning_count"]
        if type(allowed) is not int or allowed < 0 or type(baseline["schema"]) is not int or baseline["schema"] != 1:
            raise ValueError("Ungültige Warnungsbaseline")
    except (OSError, ValueError, KeyError) as error:
        print(json.dumps({"raw": raw, "unique": unique, "baseline": None}))
        print(str(error), file=sys.stderr)
        return 2
    print(json.dumps({"raw": raw, "unique": unique, "baseline": allowed}, separators=(",", ":")))
    return int(unique > allowed)


if __name__ == "__main__":
    sys.exit(main())

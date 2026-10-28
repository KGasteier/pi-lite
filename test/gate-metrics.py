#!/usr/bin/env python3
"""Clippy JSON zählen und neue Warnungen gegen den festen Diagnosebestand zeigen."""
from collections import Counter
import json
import sys


def diagnostics(log):
    raw, unique = 0, {}
    for line in log:
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(item, dict) or item.get("reason") != "compiler-message":
            continue
        message = item.get("message", {})
        if message.get("level") != "warning":
            continue
        raw += 1
        key = {
            "message": message.get("message"), "code": message.get("code"),
            "spans": [
                {k: span.get(k) for k in ("file_name", "line_start", "column_start", "line_end", "column_end", "is_primary")}
                for span in message.get("spans", [])
            ],
        }
        unique[json.dumps(key, sort_keys=True)] = message
    return raw, list(unique.values())


def warnings(log):
    raw, unique = diagnostics(log)
    return raw, len(unique)


def identity(message):
    # Zeilenverschiebungen allein sind keine neue Warnung. Quelltextanker und
    # Multimenge unterscheiden gleichartige Warnungen an verschiedenen Stellen.
    return json.dumps({
        "message": message.get("message"),
        "code": (message.get("code") or {}).get("code"),
        "primary": [{"file": s.get("file_name"),
                     "source": [t.get("text", "").strip() for t in s.get("text", [])]}
                    for s in message.get("spans", []) if s.get("is_primary")],
    }, sort_keys=True)


def new_warnings(current, baseline):
    remaining = Counter(identity(m) for m in baseline)
    added = []
    for message in current:
        key = identity(message)
        if remaining[key]:
            remaining[key] -= 1
        else:
            added.append(message)
    return added


def locations(message):
    code = (message.get("code") or {}).get("code") or "warning"
    spans = [s for s in message.get("spans", []) if s.get("is_primary")]
    return [f"{s.get('file_name')}:{s.get('line_start')}:{code}" for s in spans] or [f"<ohne Ort>:0:{code}"]


def main():
    if len(sys.argv) not in (3, 4):
        print("Aufruf: gate-metrics.py <clippy-log> <baseline.json> [neue-warnungen.log]", file=sys.stderr)
        return 2
    try:
        with open(sys.argv[1], encoding="utf-8") as log:
            raw, current = diagnostics(log)
    except OSError as error:
        print(str(error), file=sys.stderr)
        return 2
    unique = len(current)
    try:
        with open(sys.argv[2], encoding="utf-8") as f:
            baseline = json.load(f)
        if not isinstance(baseline, dict):
            raise ValueError("Warnungsbaseline muss ein Objekt sein")
        allowed = baseline["unique_warning_count"]
        if type(allowed) is not int or allowed < 0 or type(baseline["schema"]) is not int or baseline["schema"] != 1:
            raise ValueError("Ungültige Warnungsbaseline")
        details = baseline.get("diagnostics")
        if details is not None and (not isinstance(details, list) or
                                    any(not isinstance(m, dict) for m in details) or len(details) != allowed):
            raise ValueError("Ungültige Diagnosebaseline")
        added = new_warnings(current, details) if details is not None else None
        if len(sys.argv) == 4:
            with open(sys.argv[3], "w", encoding="utf-8") as report:
                if added is None:
                    report.write("UNBEKANNT neue Warnungen: Baseline enthält nur die Anzahl.\n")
                else:
                    report.write(f"Neue Warnungen ggü. {baseline.get('application_commit', 'Baseline')}: {len(added)}\n")
                    for message in sorted(added, key=lambda m: locations(m)):
                        for location in locations(message):
                            report.write(f"NEW {location} | {message.get('message')}\n")
    except (OSError, ValueError, KeyError) as error:
        print(json.dumps({"raw": raw, "unique": unique, "baseline": None}))
        print(str(error), file=sys.stderr)
        return 2
    print(json.dumps({"raw": raw, "unique": unique, "baseline": allowed,
                      "new": None if added is None else len(added)}, separators=(",", ":")))
    # Bestehende Gate-Politik bleibt: Warnungsanzahl nicht erhöhen.
    return int(unique > allowed)


if __name__ == "__main__":
    sys.exit(main())

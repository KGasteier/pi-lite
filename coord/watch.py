#!/usr/bin/env python3
"""Kurzstatus der cocopi-v2-Subagents: Zustand, Logwachstum, letzte Werkzeugaufrufe, Fehler."""
import json, os, sys, time
D = "/home/medienhaus/Work/pi-lite/coord/runs"
for L in sys.argv[1:] or []:
    st = {}
    try: st = json.load(open(f"{D}/{L}.status.json"))
    except Exception: pass
    log = f"{D}/{L}.jsonl"
    size = os.path.getsize(log) if os.path.exists(log) else 0
    age = int(time.time() - os.path.getmtime(log)) if size else -1
    tools, last_text, err = [], "", ""
    if size:
        for line in open(log, errors="replace"):
            try: e = json.loads(line)
            except Exception: continue
            t = e.get("type")
            if t == "message_end":
                m = e.get("message", {})
                if m.get("stopReason") == "error" or m.get("errorMessage"):
                    err = (m.get("errorMessage") or "")[:200]
                for c in m.get("content", []) if isinstance(m.get("content"), list) else []:
                    if c.get("type") == "toolCall":
                        a = c.get("arguments", {})
                        tools.append(f"{c.get('name')}:{(a.get('command') or a.get('path') or '')[:70]}")
                    elif c.get("type") == "text" and c.get("text", "").strip():
                        last_text = c["text"].strip().replace("\n", " ")[:160]
    errf = f"{D}/{L}.err"
    errtail = open(errf, errors="replace").read()[-200:].strip() if os.path.exists(errf) else ""
    print(f"[{L}] {st.get('state','?')} rc={st.get('rc','-')} log={size//1024}KiB still={age}s calls={len(tools)}")
    for t in tools[-3:]: print("     ", t)
    if last_text: print("      »", last_text)
    if err: print("      FEHLER:", err)
    if errtail: print("      stderr:", errtail)
import glob
HB = "/home/medienhaus/Work/pi-lite/coord/heartbeat"
for f in sorted(glob.glob(HB + "/*")):
    age = int(time.time() - os.path.getmtime(f))
    last = open(f, errors="replace").read().strip().splitlines()[-1:] or [""]
    print(f"[hb {os.path.basename(f)}] vor {age//60} min: {last[0][:140]}" + ("  !! >30 min" if age > 1800 else ""))

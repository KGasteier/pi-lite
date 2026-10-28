#!/usr/bin/env python3
"""systemctl-show-Ausgabe ohne Nebenwirkungen gegen Slotgrenzen prüfen."""
import re
import sys


def matches(text: str, cpu: int, high: int, maximum: int, tasks: int) -> bool:
    fields = dict(line.split("=", 1) for line in text.splitlines() if "=" in line)
    time = fields.get("CPUQuotaPerSecUSec", "")
    units = {"us": 1, "ms": 1000, "s": 1000000, "min": 60000000}
    parts = re.findall(r"([0-9.]+)(us|ms|s|min)", time)
    if not parts or re.sub(r"([0-9.]+)(us|ms|s|min)\s*", "", time):
        return False
    actual = sum(float(value) * units[unit] for value, unit in parts)
    return (actual == cpu and fields.get("MemoryHigh") == str(high)
            and fields.get("MemoryMax") == str(maximum)
            and fields.get("TasksMax") == str(tasks))


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit("CPU in µs, MemoryHigh/Max in Byte und TasksMax angeben")
    sys.exit(0 if matches(sys.stdin.read(), *(int(x) for x in sys.argv[1:])) else 1)

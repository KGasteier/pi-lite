#!/usr/bin/python3 -I
"""Inhaltsfreie Pool-Projektion, niemals Empfangsschlüssel oder Belegungsnamen lesen."""
import datetime
import json
import os
from pathlib import Path
import stat
import pwd
import subprocess
import sys


def read_safe(path, owner=0):
    # Nur nicht-symlinkende Eltern, Datei begrenzt; Aufrufer bereits Root.
    for parent in (path, *path.parents):
        if parent.is_symlink():
            raise ValueError('Link')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd) as file:
        info = os.fstat(file.fileno())
        if (not stat.S_ISREG(info.st_mode) or info.st_size > 65536
                or info.st_uid != owner or info.st_mode & 0o077):
            raise ValueError('Format')
        return json.load(file)


def validate(data, now):
    if set(data) != {'protokoll', 'zeit', 'slots'} or data.get('protokoll') != 1:
        raise ValueError('Vertrag')
    moment = datetime.datetime.fromisoformat(data['zeit'].replace('Z', '+00:00'))
    if not 0 <= (now - moment).total_seconds() <= 120:
        raise ValueError('Status veraltet')
    rows = data['slots']
    if len(rows) != 10 or {row['slot'] for row in rows} != {f'account{n:02}' for n in range(1, 11)}:
        raise ValueError('Slots unvollständig')
    for row in rows:
        if set(row) != {'slot', 'zustand'} or row['zustand'] not in ('frei', 'belegt', 'leeren', 'gesperrt'):
            raise ValueError('Unbekannter Zustand oder personenbezogene Felder')
    return sorted(rows, key=lambda row: row['slot'])


def read_receipt(slot):
    # N7: dieselbe enge Belegprüfung wie beim manuellen Admin-Aufruf;
    # kein Live-pruefen nach Hubstart und keine Home-/Prozessprüfung.
    result = subprocess.run(['/usr/local/sbin/cocopi-slotctl', 'pruefen', '--beleg', slot],
                            stdin=subprocess.DEVNULL, capture_output=True, text=True,
                            timeout=10, check=True,
                            env={'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'LC_ALL': 'C'})
    return json.loads(result.stdout.splitlines()[-1])

def main():
    if os.geteuid() != 0:
        return 1
    now = datetime.datetime.now(datetime.timezone.utc)
    try:
        reception_uid = pwd.getpwnam('cocopi-empfang').pw_uid
        rows = validate(read_safe(Path('/var/lib/cocopi-empfang/pool-status.json'), reception_uid), now)
        issues = 0
        for row in rows:
            slot, state = row['slot'], row['zustand']
            proof = 'nicht nötig (belegt)' if state == 'belegt' else 'nicht bestätigt'
            if state == 'frei':
                receipt = read_receipt(slot)
                if (receipt.get('slot') != slot or receipt.get('zustand') != 'leer'
                        or receipt.get('nachweis') != 'vor_hubstart'
                        or not 0 <= receipt.get('quota_bytes', -1) <= 1048576):
                    raise ValueError('Löschbeleg fehlt')
                proof = 'leer vor Hubstart; Epoche ' + str(receipt['epoche']) + '; Beleg ' + receipt['zeit']
            if state in ('gesperrt', 'leeren'):
                issues += 1
            print(f'{slot}: {state}; {proof}')
        return bool(issues)
    except (OSError, ValueError, KeyError, TypeError, IndexError, OverflowError, subprocess.SubprocessError):
        print('Pool: unbekannt. Aktuelle inhaltsfreie Statusdatei / Löschbelege fehlen oder sind ungültig.', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())

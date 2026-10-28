#!/usr/bin/python3 -I
"""Nur den rootverwalteten Slot-Zähler lesen; keine Inhalte oder Test-Nullwerte."""
import os
import re
import stat
import sys


def read_epoch(slot, root='/'):
    if not re.fullmatch(r'account(0[1-9]|10)', slot):
        raise ValueError('Ungültiger Arbeitsplatz')
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    fd = os.open(root, flags)
    try:
        for part in ('var', 'lib', 'cocopi', 'slots'):
            child = os.open(part, flags, dir_fd=fd)
            os.close(fd)
            fd = child
            info = os.fstat(fd)
            if info.st_uid != 0 or info.st_mode & 0o022:
                raise ValueError('Verwaltungspfad unsicher')
        value = os.open(slot + '.epoche', os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
        with os.fdopen(value) as file:
            info = os.fstat(file.fileno())
            if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1
                    or info.st_uid != 0 or stat.S_IMODE(info.st_mode) != 0o644):
                raise ValueError('Epochen-Datei unsicher')
            text = file.read(32)
        if not re.fullmatch(r'(0|[1-9][0-9]{0,15})\n?', text):
            raise ValueError('Ungültige Epoche')
        value = int(text)
        if value > 9007199254740991:
            raise ValueError('Epoche zu groß')
        return value
    finally:
        os.close(fd)


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2:
            raise ValueError('Genau einen Arbeitsplatz angeben')
        print(read_epoch(sys.argv[1]))
    except (ValueError, OSError):
        print('Epoche fehlt oder ist unsicher. Administration um Prüfung bitten.', file=sys.stderr)
        sys.exit(1)

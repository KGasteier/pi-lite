#!/usr/bin/python3 -I
"""Empfang-EnvironmentFile prüfen, niemals sourcen oder Werte ausgeben."""
import re
import sys
from urllib.parse import urlsplit


def validate(text, slots):
    expected = {
        'EMPFANG_PORT': '8800',
        'EMPFANG_STATE': '/var/lib/cocopi-empfang',
        'EMPFANG_SLOTS': slots,
        'EMPFANG_SLOTCTL': '/usr/local/lib/cocopi-r2/slotctl-empfang',
        'EMPFANG_EPOCHEN_DIR': '/var/lib/cocopi/slots',
    }
    values = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        key, separator, value = line.partition('=')
        if not separator or not re.fullmatch(r'[A-Z][A-Z0-9_]*', key) or key in values:
            raise ValueError('Ungültige oder doppelte Einstellung')
        # Bewusst nur eindeutiges, unquotiertes Teilformat von EnvironmentFile.
        if not value or any(c.isspace() or c in "\"'\\$`" for c in value):
            raise ValueError('Mehrdeutiger Wert')
        values[key] = value
    # N7/N9 nutzen vorhandenen Helfer/Schlüssel, N8 den Client und TAGESENDE.
    # N10 bleibt fest 384 MiB; keine erfundene überschreibbare Servervariable.
    allowed = set(expected) | {'EMPFANG_ORIGIN', 'EMPFANG_OIDC_ISSUER',
                              'EMPFANG_OIDC_CLIENT_ID', 'EMPFANG_TAGESENDE', 'DO_NOT_TRACK'}
    if set(values) - allowed or any(values.get(key) != value for key, value in expected.items()):
        raise ValueError('Unbekannte oder abweichende Einstellung')
    for key in ('EMPFANG_ORIGIN', 'EMPFANG_OIDC_ISSUER'):
        url = urlsplit(values.get(key, ''))
        if (url.scheme != 'https' or not url.hostname or url.username or url.password
                or url.query or url.fragment or url.hostname.endswith('.invalid')):
            raise ValueError('HTTPS-Adresse fehlt')
        if key == 'EMPFANG_ORIGIN' and url.path:
            raise ValueError('Origin ohne Pfad angeben')
    if values.get('EMPFANG_OIDC_CLIENT_ID', '') in ('', 'BITTE_EINTRAGEN'):
        raise ValueError('Client-ID fehlt')
    if not re.fullmatch(r'([01]\d|2[0-3]):[0-5]\d', values.get('EMPFANG_TAGESENDE', '')):
        raise ValueError('Tagesende fehlt')


if __name__ == '__main__':
    try:
        if len(sys.argv) != 3:
            raise ValueError('EnvironmentFile und Arbeitsplätze angeben')
        with open(sys.argv[1], encoding='utf-8') as file:
            validate(file.read(), sys.argv[2])
    except (OSError, ValueError):
        print('Empfang-Konfiguration unvollständig oder abweichend. AUTHENTIK.md prüfen; nichts gestartet.', file=sys.stderr)
        sys.exit(1)

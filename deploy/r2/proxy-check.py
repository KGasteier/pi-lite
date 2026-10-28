#!/usr/bin/python3 -I
"""Nur bestätigte HTTPS-403 gelten als Proxy-Sperre; kein Redirect/TLS-Fallback."""
import json
import re
import subprocess
import sys


def paths():
    for prefix in ('', *(f'/slot/account{n:02}' for n in range(1, 11))):
        for endpoint in ('belegung', 'freigabe', 'sync/dienst/archiv'):
            yield f'{prefix}/v1/{endpoint}'
            yield f'{prefix}/v1/{endpoint}/'
    for path in ('/slot/account01/v1/%62elegung', '/slot/account01//v1/freigabe',
                 '/slot/account01/x/../v1/belegung', '/slot/account01/v1/freigabe%2f',
                 '/slot/account01/v1/%2e%2e/v1/belegung'):
        yield path


def main():
    if len(sys.argv) != 2 or not re.fullmatch(r'https://[a-zA-Z0-9.-]+(?::[0-9]+)?', sys.argv[1]):
        print('Bitte explizite HTTPS-Proxyadresse ohne Geheimnisse angeben.', file=sys.stderr)
        return 1
    for path in paths():
        # Kein Token, keine gültige Signatur, nie --insecure, nie --location.
        try:
            result = subprocess.run(['/usr/bin/curl', '--silent', '--noproxy', '*',
                '--path-as-is', '--max-time', '5', '--output', '/dev/null',
                '--write-out', '%{http_code}', '-X', 'POST', '-H', 'Content-Type: application/json',
                '--data', '{"protokoll":1}', sys.argv[1] + path], capture_output=True, timeout=8)
        except (OSError, subprocess.TimeoutExpired):
            print('Proxy-Sperren: Verbindung nicht bestätigt.', file=sys.stderr)
            return 1
        if result.returncode or result.stdout != b'403':
            print('Proxy-Sperren: nicht bestätigt. Keine Freigabe.', file=sys.stderr)
            return 1
    print('Proxy-Sperren: alle direkten, Slot- und Angriffspfade antworten 403.')
    return 0


if __name__ == '__main__':
    sys.exit(main())

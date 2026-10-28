#!/usr/bin/env python3
"""Isolierte Regression: kein Root, kein Systemdienst, kein Quota-Schreiben."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), BASE / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


QUOTA = load('quota-read')
SLICE = load('slice-read')


class SafeScripts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.geteuid() == 0:
            raise unittest.SkipTest('Diese Tests ausschließlich als normaler Nutzer ausführen')

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='cocopi-s4-test-')
        self.addCleanup(self.temp.cleanup)
        self.env = dict(os.environ, HOME=self.temp.name, XDG_CONFIG_HOME=self.temp.name,
                        PYTHONDONTWRITEBYTECODE='1', LC_ALL='C', TZ='UTC')
        # Eine versehentlich ausgeführte Änderungsfunktion muss sofort auffallen.
        self.sentinel = Path(self.temp.name) / 'mutation-attempt'
        for command in ('useradd', 'userdel', 'usermod', 'groupadd', 'install',
                        'setquota', 'loginctl', 'systemctl', 'runuser', 'openssl',
                        'sudo', 'rm', 'mv', 'tar', 'chown', 'chmod'):
            self.env[f'BASH_FUNC_{command}%%'] = (
                f'() {{ printf "%s\\n" "{command}" >> "{self.sentinel}"; return 97; }}'
            )

    def run_script(self, script, *args):
        result = subprocess.run(['bash', str(BASE / script), *args], env=self.env,
                                cwd=self.temp.name, text=True, capture_output=True, timeout=10)
        self.assertFalse(self.sentinel.exists(), result.stdout + result.stderr)
        self.assertEqual(list(Path(self.temp.name).iterdir()), [])
        return result

    def helper(self, code):
        return subprocess.run(['bash', '-c', 'source "$1/common.sh"; ' + code,
                               'bash', str(BASE)], env=self.env, cwd=self.temp.name,
                              text=True, capture_output=True, timeout=10)

    def test_bash_syntax(self):
        for path in [*BASE.rglob('*.sh'), BASE / 'slots.conf']:
            with self.subTest(path=path.name):
                result = subprocess.run(['bash', '-n', str(path)], capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_default_is_explicit_dry_run(self):
        for script in ('einrichten.sh', 'provision.sh', 'check.sh', 'teardown.sh', 'admin-check.sh'):
            with self.subTest(script=script):
                default = self.run_script(script, '--slots', 'account01,account02')
                explicit = self.run_script(script, '--dry-run', '--slots', 'account01,account02')
                self.assertEqual(default.returncode, 0, default.stderr)
                self.assertEqual(default.stdout, explicit.stdout)
                self.assertTrue('dry-run' in default.stdout or 'Vorschau' in default.stdout)
                self.assertNotIn('OK:', default.stdout)
                self.assertIn('account01', default.stdout)
                self.assertIn('account02', default.stdout)
                self.assertNotIn('--- account10', default.stdout)

    def test_apply_requires_root_without_mutation(self):
        for script in ('einrichten.sh', 'provision.sh', 'check.sh', 'teardown.sh', 'admin-check.sh'):
            with self.subTest(script=script):
                result = self.run_script(script, '--ausfuehren', '--slots', 'account01,account02')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('benötigt Root', result.stderr)

    def test_bad_arguments_fail_closed(self):
        for script in ('einrichten.sh', 'provision.sh', 'check.sh', 'teardown.sh', 'admin-check.sh'):
            for args in (('--slots', ''), ('--slots', 'account00'), ('--slots', 'account11'),
                         ('--slots', 'account01,account01'), ('--slots', 'account01, account02'),
                         ('--slots', '../../root'), ('--slots', 'account01;true'),
                         ('--slots', 's01,s02'), ('--slots', 'cocopi-s01'),
                         ('--slots',), ('--dry-run', '--apply'),
                         ('--slots', 'account01', '--slots', 'account02'), ('--unknown',)):
                with self.subTest(script=script, args=args):
                    self.assertNotEqual(self.run_script(script, *args).returncode, 0)

    def test_teardown_needs_explicit_slots(self):
        self.assertNotEqual(self.run_script('teardown.sh').returncode, 0)
        self.assertNotEqual(self.run_script('provision.sh', '--purge').returncode, 0)
        result = self.run_script('teardown.sh', '--slots', 'account01,account02', '--purge')
        self.assertEqual(result.returncode, 0)
        self.assertIn('erst nach gültigem Archiv', result.stdout)

    def test_plan_ports_and_budget(self):
        result = self.run_script('provision.sh', '--slots', 'account01,account02')
        self.assertIn('Port 8801', result.stdout)
        self.assertIn('Port 8802', result.stdout)
        self.assertIn('9765625 KiB NVMe + 68359375 KiB SATA', result.stdout)
        self.assertIn('Inhalt niemals anzeigen', result.stdout)
        result = self.helper('[[ $((NVME_QUOTA_KIB+DATA_QUOTA_KIB)) == 78125000 ]] && '
                             '[[ $((78125000*1024)) == 80000000000 ]] && '
                             '[[ ${#SLOTS[@]} == 10 && ${SLOTS[9]} == account10 ]]')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_all_accounts_map_to_same_unix_name_and_stable_ids(self):
        result = self.helper(r'''
getent() { return 2; }
for slot in "${SLOTS[@]}"; do
    slot_vars "$slot"
    printf '%s:%s:%s:%s:%s:%s\n' "$SLOT" "$USER_NAME" "$EXPECTED_UID" "$PORT" "$UNIT" "$HUB_STATE"
done
''')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            f'account{n:02}:account{n:02}:{20000+n}:{8800+n}:'
            f'cocopi-hub@account{n:02}.service:/home/account{n:02}/.cocopi-hub'
            for n in range(1, 11)
        ])

    def test_generated_hub_contract_and_private_state_directory(self):
        # Nur Dateierzeugung abfangen: MODE bleibt dry-run; keine Systemänderung.
        fixture = r'''
getent() { return 2; }
managed_file() {
    if [[ $1 == /etc/cocopi/hub-*.env ]]; then
        /usr/bin/cat
    else
        /usr/bin/cat >/dev/null
    fi
}
set -- --slots account01,account02
'''
        body = (BASE / 'provision.sh').read_text()
        body = body[body.index('parse_args provision'):]
        result = self.helper(fixture + body)
        self.assertEqual(result.returncode, 0, result.stderr)
        env_lines = [line for line in result.stdout.splitlines() if line.startswith('HUB_')]
        for n in (1, 2):
            for line in (f'HUB_SLOT=account{n:02}', 'HUB_MAX_OPEN_SESSIONS=2',
                         'HUB_MAX_ACTIVE_SESSIONS=1', 'HUB_GLOBAL_HEAVY_JOBS=1',
                         f'HUB_STATE=/home/account{n:02}/.cocopi-hub',
                         f'HUB_EPOCHE_DATEI=/var/lib/cocopi/slots/account{n:02}.epoche',
                         f'HUB_ROOTS=/home/account{n:02}/arbeit',
                         'HUB_EMPFANG_PUBKEY=BITTE_EINTRAGEN'):
                self.assertIn(line, env_lines)
            self.assertIn(f'PLAN: bei neuem Konto Skelett /etc/cocopi/skel als account{n:02}', result.stdout)
        self.assertNotIn('HUB_STATE=off', result.stdout)
        self.assertFalse(self.sentinel.exists())

    def test_empfang_env_matches_s7_names_without_shell_evaluation(self):
        checker = load('empfang-env-check')
        text = (BASE / 'empfang.env.example').read_text().replace('cocopi.example.invalid', 'seminar.example.org').replace('auth.example.invalid', 'auth.example.org').replace('BITTE_EINTRAGEN', 'public-client')
        checker.validate(text, 'account01,account02')
        for bad in (text.replace('EMPFANG_STATE=', 'EMPFANG_STATE_DIR='),
                    text.replace('/var/lib/cocopi/slots', '/home/account01'),
                    text.replace('EMPFANG_OIDC_ISSUER=https:', 'EMPFANG_OIDC_ISSUER=http:'),
                    text.replace('public-client', '$(touch /tmp/nie)'),
                    text + '\nEMPFANG_PORT=8801\n',
                    text.replace('account01,account02', 'account01..account10')):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                checker.validate(bad, 'account01,account02')
        self.assertFalse(self.sentinel.exists())

    def test_epoch_reader_checks_numeric_value_rights_and_links(self):
        from unittest.mock import patch
        from types import SimpleNamespace
        reader = load('epoch-read')
        root = Path(self.temp.name)
        parent = root / 'var/lib/cocopi/slots'
        parent.mkdir(parents=True)
        for path in (root / 'var', root / 'var/lib', root / 'var/lib/cocopi', parent):
            path.chmod(0o755)
        epoch = parent / 'account01.epoche'
        epoch.write_text('42\n')
        epoch.chmod(0o644)
        real_fstat = reader.os.fstat
        def root_stat(fd):
            info = real_fstat(fd)
            return SimpleNamespace(st_uid=0, st_mode=info.st_mode, st_nlink=info.st_nlink)
        with patch.object(reader.os, 'fstat', root_stat):
            self.assertEqual(reader.read_epoch('account01', str(root)), 42)
            for value in ('01', '-1', '9007199254740992', '42\n43', ''):
                epoch.write_text(value)
                with self.assertRaises(ValueError):
                    reader.read_epoch('account01', str(root))
            epoch.write_text('42\n')
            epoch.chmod(0o666)
            with self.assertRaises(ValueError):
                reader.read_epoch('account01', str(root))
            epoch.unlink()
            epoch.symlink_to('/nonexistent-cocopi-epoch')
            with self.assertRaises(OSError):
                reader.read_epoch('account01', str(root))
        with self.assertRaises(ValueError):
            reader.read_epoch('../account01', str(root))

    def test_pool_projection_rejects_stale_incomplete_and_personal_fields(self):
        import copy
        import datetime
        reader = load('pool-read')
        now = datetime.datetime.now(datetime.timezone.utc)
        packet = {'protokoll': 1, 'zeit': now.isoformat(), 'slots': [
            {'slot': f'account{n:02}', 'zustand': 'frei' if n % 2 else 'belegt'}
            for n in range(1, 11)]}
        self.assertEqual(len(reader.validate(packet, now)), 10)
        for change in ('stale', 'missing', 'duplicate', 'name', 'state'):
            bad = copy.deepcopy(packet)
            if change == 'stale':
                bad['zeit'] = (now - datetime.timedelta(seconds=121)).isoformat()
            elif change == 'missing': bad['slots'].pop()
            elif change == 'duplicate': bad['slots'][0]['slot'] = 'account02'
            elif change == 'name': bad['slots'][0]['person'] = 'nicht ausgeben'
            else: bad['slots'][0]['zustand'] = 'alles gut'
            with self.subTest(change=change), self.assertRaises(ValueError):
                reader.validate(bad, now)

    def test_proxy_checks_are_mocked_and_fail_closed(self):
        from unittest.mock import patch
        checker = load('proxy-check')
        seen = []
        def fake(args, **kwargs):
            seen.append(args)
            return subprocess.CompletedProcess(args, 0, b'403', b'')
        with patch.object(sys, 'argv', ['proxy-check', 'https://example.invalid']), \
             patch.object(checker.subprocess, 'run', fake):
            self.assertEqual(checker.main(), 0)
        self.assertEqual(len(seen), 71)
        self.assertTrue(all('--path-as-is' in args and '--location' not in args and '--insecure' not in args for args in seen))
        for code in (b'200', b'401', b'404', b'500', b'302', b'000'):
            with patch.object(sys, 'argv', ['proxy-check', 'https://example.invalid']), \
                 patch.object(checker.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, code, b'')):
                self.assertEqual(checker.main(), 1)

    def test_proxy_templates_block_n5_before_routing(self):
        import re
        routes = (BASE / 'proxy/routes.caddy').read_text()
        for name in ('intern', 'dienst'):
            pattern = re.search(r'@' + name + r' path_regexp \w+ (.+)', routes).group(1)
            first_route = routes.index('handle /v1/empfang/')
            self.assertLess(routes.index('respond @' + name + ' "Nur intern" 403'), first_route)
            for prefix in ('', *(f'/slot/account{n:02}' for n in range(1, 11))):
                paths = ('/v1/belegung', '/v1/freigabe') if name == 'intern' else ('/v1/sync/dienst', '/v1/sync/dienst/archiv/abc')
                for path in paths:
                    for suffix in ('', '/'):
                        self.assertIsNotNone(re.fullmatch(pattern, prefix + path + suffix))
            for path in ('/v1/empfang/hallo', '/slot/account01/v1/sync/hoch', '/slot/account10/v1/anmelden'):
                self.assertIsNone(re.fullmatch(pattern, path))
        self.assertLess(routes.index('respond @unsicher'), routes.index('handle /v1/empfang/'))
        for variant in ('Caddyfile', 'Caddyfile.tailscale'):
            text = (BASE / 'proxy' / variant).read_text()
            self.assertIn('import /etc/cocopi/proxy/routes.caddy', text)
            self.assertNotIn('reverse_proxy', text)
        local = (BASE / 'proxy/Caddyfile.tailscale').read_text()
        self.assertIn('http://127.0.0.1:8799', local)
        self.assertIn('bind 127.0.0.1', local)
        serve = (BASE / 'proxy/tailscale-serve.sh').read_text()
        commands = [line.strip() for line in serve.splitlines() if line.strip().startswith('tailscale ')]
        self.assertEqual(commands, ['tailscale serve --bg --https=443 http://127.0.0.1:8799'])
        for n in range(1, 11):
            self.assertIn(f'handle /slot/account{n:02}/v1/*', routes)
            self.assertIn(f'reverse_proxy 127.0.0.1:{8800+n}', routes)

    def test_system_units_and_sudoers_have_narrow_contract(self):
        hub = (BASE / 'systemd/cocopi-hub@.service').read_text()
        reception = (BASE / 'systemd/cocopi-empfang.service').read_text()
        self.assertIn('Environment=HUB_EPOCHE_DATEI=/var/lib/cocopi/slots/%i.epoche', hub)
        self.assertIn('EMPFANG_EPOCHEN_DIR=/var/lib/cocopi/slots', (BASE / 'empfang.env.example').read_text())
        for text in ('User=%i', 'Slice=cocopi-%i.slice', 'EnvironmentFile=/etc/cocopi/hub-%i.env', 'KillMode=control-group'):
            self.assertIn(text, hub)
        for text in ('User=cocopi-empfang', 'StateDirectory=cocopi-empfang', 'StateDirectoryMode=0700', '/empfang/empfang.mjs'):
            self.assertIn(text, reception)
        sudoers = (BASE / 'sudoers/cocopi-slotctl').read_text()
        import re
        commands = re.findall(r'/usr/local/sbin/cocopi-slotctl ([^,\n\\]+)', sudoers)
        self.assertEqual(len(commands), 31)
        allowed = {'status'} | {f'{action} account{n:02}' for n in range(1, 11)
            for action in ('status', 'pruefen', '--ausfuehren leeren')}
        self.assertEqual(set(commands), allowed)
        self.assertIn('NOSETENV:', sudoers)
        self.assertNotIn('SETENV:', sudoers.replace('NOSETENV:', ''))

    def test_tailscale_default_is_only_plan(self):
        env = dict(self.env)
        env['BASH_FUNC_tailscale%%'] = f'() {{ echo forbidden >> "{self.sentinel}"; return 97; }}'
        result = subprocess.run(['bash', str(BASE / 'proxy/tailscale-serve.sh')], env=env,
                                text=True, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.sentinel.exists())
        self.assertIn('127.0.0.1:8799', result.stdout)

    def test_admin_proxy_argument_rejects_tokens_and_unsafe_urls(self):
        for url in ('http://example.invalid', 'https://user:token@example.invalid',
                    'https://example.invalid/token', 'https://example.invalid?token=x'):
            result = self.run_script('admin-check.sh', '--proxy-url', url)
            self.assertNotEqual(result.returncode, 0)

    def test_sudo_listing_rejects_any_allow_rule(self):
        fixtures = (
            ('User account01 may run the following commands:\n    (ALL : ALL) !ALL', 0, True),
            ('User account01 is not allowed to run sudo on host.', 1, True),
            ('sudo: configuration error', 1, False),
            ('User x may run:\n    (root) NOPASSWD: /usr/bin/apt', 0, False),
            ('User x may run:\n    (ALL : ALL) !ALL\n    (root) /bin/bash', 0, False),
            ('unknown listing', 0, False),
        )
        for listing, status, denied in fixtures:
            with self.subTest(listing=listing):
                # Fixture wird als Daten über stdin gelesen, nie evaluiert.
                code = ('sudo() { printf "%s\\n" "$LISTING"; return "$STATUS"; }; '
                        'sudo_denied account01')
                env = dict(self.env, LISTING=listing, STATUS=str(status))
                result = subprocess.run(['bash', '-c', 'source "$1/common.sh"; ' + code,
                                         'bash', str(BASE)], env=env, capture_output=True)
                self.assertEqual(result.returncode == 0, denied)

    def test_symlink_and_parent_traversal_rejected(self):
        link = Path(self.temp.name) / 'link'
        link.symlink_to('/nonexistent-cocopi-test-target')
        result = self.helper('! no_symlinks "$HOME/link" && ! no_symlinks "$HOME/../x"')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_preflight_clean_new_accounts_returns_success(self):
        # Die echte Funktionsdefinition testen, nicht eine duplizierte Implementierung.
        # Alle Hostabfragen sind Fixtures, alle Änderungsbefehle bleiben Sentinel-gesperrt.
        source = (BASE / 'provision.sh').read_text()
        definition = source[source.index('preflight() {'):source.index('\n\nif [[ $MODE == apply')]
        fixture = r'''
require_commands() { :; }
check_mounts() { :; }
check_hub_contract() { :; }
root_controlled() { :; }
stat() { printf '%s\n' cgroup2fs; }
command() { printf '%s\n' /usr/bin/true; }
readlink() { printf '%s\n' /usr/bin/true; }
getent() { return 2; }
parse_args provision --slots account01,account02
HOME_BASE=$HOME/homes
DATA_BASE=$HOME/data
STATE_BASE=$HOME/state
HUB_EMPFANG_PUBKEY=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
preflight
'''
        result = subprocess.run(['bash', '-c', 'source "$1/common.sh"; ' + definition + fixture,
                                 'bash', str(BASE)], env=self.env, cwd=self.temp.name,
                                text=True, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.sentinel.exists())

    def test_hub_contract_unconfirmed_fails_before_any_service(self):
        result = self.helper('check_hub_contract')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Hub-Vertrag unbestätigt', result.stderr)
        self.assertFalse(self.sentinel.exists())

    def test_mount_preflight_rejects_unknown_unquotified_and_inactive(self):
        fixture = r'''
findmnt() {
    case "$3" in
        TARGET) printf '%s\n' "${@: -1}" ;;
        FSTYPE) printf '%s\n' "$MOCK_FS" ;;
        OPTIONS) printf '%s\n' "$MOCK_OPTIONS" ;;
        SOURCE) printf '%s\n' "device-${@: -1}" ;;
    esac
}
quotaon() { printf '%s\n' "$MOCK_QUOTA"; }
check_mounts
'''
        for filesystem, options, quota, good in (
            ('ext4', 'rw,usrquota', 'user quota on / is on', True),
            ('xfs', 'rw,prjquota', 'user quota on / is on', False),
            ('ext4', 'rw,relatime', 'user quota on / is on', False),
            ('ext4', 'rw,usrquota', 'user quota on / is off', False),
        ):
            with self.subTest(filesystem=filesystem, options=options, quota=quota):
                env = dict(self.env, MOCK_FS=filesystem, MOCK_OPTIONS=options, MOCK_QUOTA=quota)
                result = subprocess.run(['bash', '-c', 'source "$1/common.sh"; ' + fixture,
                                         'bash', str(BASE)], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode == 0, good, result.stderr)

    def test_launcher_rejects_wrong_slot_without_reading_token(self):
        for args in ((), ('account00',), ('../../root',), ('account01',), ('s01',)):
            result = self.run_script('hub-launch.sh', *args)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, '')

    def test_admin_dry_run_is_quiet_and_honest(self):
        result = self.run_script('admin-check.sh', '--slots', 'account01,account02')
        self.assertEqual(result.returncode, 0, result.stderr)
        for title in ('Konto', 'Eingerichtet', 'Verbindung', 'Zugang', 'Speicher', 'Letzte Aktivität'):
            self.assertIn(title, result.stdout)
        self.assertEqual(result.stdout.count('nicht geprueft'), 12)
        self.assertIn('kein Anwesenheitsnachweis', result.stdout)
        self.assertNotIn('OK:', result.stdout)

    def admin_fixture(self, extra=''):
        # Echte Kontrolllogik, aber ausschließlich Attrappen für Hostabfragen.
        source = (BASE / 'admin-check.sh').read_text()
        body = source[source.index('ISSUES=0\n'):]
        definitions = source[source.index('last_activity()'):source.index('\nISSUES=0\n')]
        fixture = r'''
MODE=apply
SLOT_LIST=account01,account02
SELECTED=(account01 account02)
getent() { return 2; }
slot_vars() {
    SLOT=$1; USER_NAME=$1; UID_NUM=20001; EXPECTED_UID=20001
    SLOT_HOME=$HOME/homes/$1; SLOT_DATA=$HOME/data/$1
    PORT=8801; UNIT=cocopi-hub@$1.service
}
require_commands() { :; }
assert_account() { return 0; }
user_systemctl() { return 97; }
hub_systemctl() { [[ $* == 'is-active --quiet cocopi-hub@account01.service' ]]; }
pool_checks() { :; }
python3() { [[ $1 == */epoch-read.py ]] && printf '7\n'; }
hub_loopback() { [[ $SLOT == account01 ]]; }
quota_usage() { printf '%s' '1.00/10.00 GB'; }
'''
        result = self.helper(definitions + fixture + extra + body)
        self.assertFalse(self.sentinel.exists(), result.stderr)
        self.assertEqual(list(Path(self.temp.name).iterdir()), [])
        return result

    def test_admin_fixture_active_and_inactive_slot(self):
        result = self.admin_fixture()
        self.assertEqual(result.returncode, 1, result.stderr)
        rows = [line for line in result.stdout.splitlines() if line.startswith('account')]
        self.assertEqual(len(rows), 2)
        self.assertIn('vorhanden', rows[0])
        self.assertIn('aktiv', rows[0])
        self.assertIn('nur lokal', rows[0])
        self.assertIn('7', rows[0])
        self.assertIn('NVMe 1.00/10.00 GB; SATA 1.00/10.00 GB', rows[0])
        self.assertTrue(rows[0].endswith('unbekannt'))
        self.assertIn('nicht aktiv', rows[1])
        self.assertIn('bitte pruefen', rows[1])

    def test_admin_fixture_missing_wrong_identity_and_quota_unavailable(self):
        fixtures = (
            ('slot_vars() { SLOT=$1; UID_NUM=; };\n', 'fehlt'),
            ('assert_account() { return 1; };\n', 'bitte pruefen'),
            ('quota_usage() { return 1; };\n', 'unbekannt'),
            ('quota_usage() { printf "%s" "10.00/10.00 GB (voll)"; };\n', '(voll)'),
            ('quota_usage() { printf "%s" "1.00/0.00 GB (Grenze pruefen)"; };\n', 'Grenze pruefen'),
        )
        for fixture, expected in fixtures:
            with self.subTest(expected=expected):
                result = self.admin_fixture(fixture)
                self.assertEqual(result.returncode, 1, result.stderr)
                rows = [line for line in result.stdout.splitlines() if line.startswith('account')]
                self.assertIn(expected, rows[0])

    def test_admin_missing_epoch_is_not_green(self):
        result = self.admin_fixture('hub_systemctl() { return 0; }; hub_loopback() { return 0; }; python3() { return 1; };\n')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('bitte pruefen', result.stdout)

    def test_admin_fixture_all_observed_checks_pass(self):
        result = self.admin_fixture('hub_systemctl() { return 0; }; hub_loopback() { return 0; };\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        # Bekannte Betriebswerte ergeben noch keinen erfundenen Aktivitätszeitpunkt.
        self.assertEqual(sum(line.endswith('unbekannt') for line in result.stdout.splitlines()), 2)

    def test_admin_activity_is_labelled_service_start_not_student_work(self):
        result = self.admin_fixture(r'''
hub_systemctl() {
    if [[ $1 == show && $* == *ActiveEnterTimestamp* ]]; then
        printf '%s\n' '2026-10-28 18:00:00 UTC'
    else
        return 0
    fi
}
hub_loopback() { return 0; }
''')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Dienststart 2026-10-28 18:00:00 UTC', result.stdout)
        self.assertIn('letzte Arbeit am Hub ist noch unbekannt', result.stdout)
        result = self.admin_fixture(r'''
hub_systemctl() { if [[ $1 == show ]]; then printf '%s\n' 'secret-unparseable'; else return 0; fi; }
hub_loopback() { return 0; }
''')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn('secret-unparseable', result.stdout)

    def test_listener_identity_and_loopback_fail_closed(self):
        fixture = r'''
UID_NUM=20001
PORT=8801
UNIT=cocopi-hub@account01.service
hub_systemctl() {
    if [[ $1 == show ]]; then printf '123\n'; fi
}
stat() { printf '%s\n' "$MOCK_UID"; }
ss() { printf '%s\n' "$MOCK_LISTENERS"; }
hub_loopback
'''
        good = 'LISTEN 0 128 127.0.0.1:8801 0.0.0.0:* users:(("node",pid=123,fd=5))'
        for listeners, uid, success in (
            (good, '20001', True), (good, '20002', False), ('', '20001', False),
            (good.replace('127.0.0.1:8801', '0.0.0.0:8801'), '20001', False),
            (good.replace('127.0.0.1:8801', '[::]:8801'), '20001', False),
            (good.replace('pid=123,', 'pid=456,'), '20001', False),
            (good + '\n' + good.replace('127.0.0.1:8801', '[::1]:8801'), '20001', False),
            (good + '\n' + good.replace('127.0.0.1:8801', '127.0.0.1:8888'), '20001', False),
        ):
            with self.subTest(listeners=listeners, uid=uid):
                env = dict(self.env, MOCK_LISTENERS=listeners, MOCK_UID=uid)
                result = subprocess.run(['bash', '-c', 'source "$1/common.sh"; ' + fixture,
                                         'bash', str(BASE)], env=env, text=True, capture_output=True)
                self.assertEqual(result.returncode == 0, success, result.stderr)
                self.assertFalse(self.sentinel.exists())

    def test_quota_usage_text_does_not_hide_limits_or_inode_exhaustion(self):
        data = QUOTA.Dqblk()
        data.bhard = data.bsoft = 9765625
        data.ihard = data.isoft = 200000
        data.space = 1_000_000_000
        self.assertEqual(QUOTA.usage_text(data, 9765625, 200000), '1.00/10.00 GB')
        data.inodes = 200000
        self.assertIn('(voll)', QUOTA.usage_text(data, 9765625, 200000))
        data.inodes = 0
        data.space = 10_000_000_001
        self.assertIn('(voll)', QUOTA.usage_text(data, 9765625, 200000))
        data.bhard = 0
        self.assertIn('Grenze pruefen', QUOTA.usage_text(data, 9765625, 200000))

    def test_slice_reader_positive_and_negative(self):
        text = 'CPUQuotaPerSecUSec=1s\nMemoryHigh=2147483648\nMemoryMax=2684354560\nTasksMax=256\n'
        self.assertTrue(SLICE.matches(text, 1000000, 2147483648, 2684354560, 256))
        self.assertTrue(SLICE.matches(text.replace('1s', '1000ms'), 1000000, 2147483648, 2684354560, 256))
        for bad in (text.replace('1s', 'infinity'), text.replace('1s', '1sjunk'),
                    text.replace('256', '512'), text.replace('2147483648', '999')):
            self.assertFalse(SLICE.matches(bad, 1000000, 2147483648, 2684354560, 256))

    def test_quota_reader_exact_split_and_over_budget(self):
        data = QUOTA.Dqblk()
        data.bhard = data.bsoft = 9765625
        data.ihard = data.isoft = 200000
        self.assertTrue(QUOTA.limits_match(data, 9765625, 200000))
        data.bsoft = 0
        self.assertFalse(QUOTA.limits_match(data, 9765625, 200000))
        data.bsoft = data.bhard
        data.space = data.bhard * 1024 + 1
        self.assertFalse(QUOTA.limits_match(data, 9765625, 200000))


class SlotctlTests(unittest.TestCase):
    def setUp(self):
        if os.geteuid() == 0:
            self.skipTest('Fake-Root ist absichtlich nur unprivilegiert erlaubt')
        self.temp = tempfile.TemporaryDirectory(prefix='cocopi-pool-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for path in ('home/account01', 'mnt/data/account01', 'etc/cocopi/skel/arbeit/Projekte',
                     'etc/cocopi/skel/arbeit/Ergebnisse', 'etc/cocopi/skel/.pi/agent/skills',
                     'etc/cocopi/skel/.pi/agent/sessions', 'var/lib/cocopi-slots',
                     'var/lib/cocopi/slots', 'run'):
            (self.root / path).mkdir(parents=True, exist_ok=True)
        epoch = self.root / 'var/lib/cocopi/slots/account01.epoche'
        epoch.write_text('0\n')
        epoch.chmod(0o644)
        (self.root / 'etc/cocopi/skel/.profile').write_text('umask 077\n')
        self.fixture()
        self.env = dict(os.environ, COCOPI_ROOT=str(self.root), HOME=str(self.root),
                        XDG_CONFIG_HOME=str(self.root), PYTHONDONTWRITEBYTECODE='1')

    def fixture(self, **values):
        import json
        (self.root / 'run/cocopi-test.json').write_text(json.dumps(values))

    def run_slot(self, *args, success=True):
        result = subprocess.run([sys.executable, '-I', str(BASE / 'cocopi-slotctl'), *args],
                                env=self.env, text=True, capture_output=True, timeout=10)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return result

    def test_default_does_not_delete_or_write_lock(self):
        secret = self.root / 'home/account01/privat'
        secret.write_text('synthetisch')
        result = self.run_slot('leeren', 'account01')
        self.assertIn('nicht_geprueft', result.stdout)
        self.assertEqual(secret.read_text(), 'synthetisch')
        self.assertFalse((self.root / 'var/lib/cocopi-slots/pool.lock').exists())
        self.assertEqual((self.root / 'var/lib/cocopi/slots/account01.epoche').read_text(), '0\n')

    def test_clear_proof_and_repeat_are_idempotent(self):
        import json
        (self.root / 'home/account01/.verlauf').write_text('synthetisch')
        (self.root / 'mnt/data/account01/build').write_bytes(b'x' * 100)
        result = json.loads(self.run_slot('--ausfuehren', 'leeren', 'account01').stdout)
        self.assertEqual(result['zustand'], 'leer')
        self.assertGreaterEqual(result['bytes_geloescht'], 100)
        self.assertEqual(result['nachweis'], 'vor_hubstart')
        self.assertEqual(result['epoche'], 1)
        self.run_slot('pruefen', 'account01')
        self.run_slot('--ausfuehren', 'leeren', 'account01')
        epoch = self.root / 'var/lib/cocopi/slots/account01.epoche'
        self.assertEqual(epoch.read_text(), '2\n')
        self.assertEqual(epoch.stat().st_mode & 0o777, 0o644)
        self.assertEqual(list(epoch.parent.iterdir()), [epoch])
        self.assertEqual(list((self.root / 'mnt/data/account01').iterdir()), [])
        self.assertTrue((self.root / 'home/account01/arbeit/Ergebnisse').is_dir())

    def test_epoch_replacement_is_atomic_for_existing_readers(self):
        epoch = self.root / 'var/lib/cocopi/slots/account01.epoche'
        with epoch.open() as old:
            before = os.fstat(old.fileno()).st_ino
            self.run_slot('--ausfuehren', 'leeren', 'account01')
            self.assertEqual(old.read(), '0\n')
            self.assertEqual(epoch.read_text(), '1\n')
            self.assertNotEqual(epoch.stat().st_ino, before)

    def test_epoch_is_mandatory_and_malformed_values_block_before_deletion(self):
        epoch = self.root / 'var/lib/cocopi/slots/account01.epoche'
        survivor = self.root / 'home/account01/bleibt'
        survivor.write_text('synthetisch')
        for text in ('-1', '01', '1.5', '1\n2', '9007199254740991', '9007199254740992', '9' * 40, ''):
            epoch.write_text(text)
            self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
            self.assertTrue(survivor.exists())
        epoch.unlink()
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.assertTrue(survivor.exists())
        epoch.symlink_to(survivor)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.assertEqual(survivor.read_text(), 'synthetisch')

    def test_epoch_unsafe_mode_and_hardlink_rejected(self):
        epoch = self.root / 'var/lib/cocopi/slots/account01.epoche'
        epoch.chmod(0o666)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        epoch.chmod(0o644)
        os.link(epoch, epoch.parent / 'hardlink')
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)

    def test_failed_clear_never_rolls_back_epoch(self):
        self.fixture(quota_bytes=1048577)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.assertEqual((self.root / 'var/lib/cocopi/slots/account01.epoche').read_text(), '1\n')
        self.assertFalse((self.root / 'var/lib/cocopi-slots/account01.leer.json').exists())

    def test_slot_traversal_and_additional_commands_rejected(self):
        for slot in ('../', '../account01', 'account00', 'account11', 'account01/..',
                     'account01;id', '/etc', 'account01\n', 'ACCOUNT01'):
            self.run_slot('--ausfuehren', 'leeren', slot, success=False)
        self.run_slot('leeren', success=False)
        self.run_slot('start', 'account01', success=False)
        self.run_slot('--dry-run', '--ausfuehren', 'leeren', 'account01', success=False)

    def test_inner_symlink_to_etc_unlinked_not_followed(self):
        (self.root / 'home/account01/angriff').symlink_to('/etc', target_is_directory=True)
        (self.root / 'mnt/data/account01/kaputt').symlink_to('/does-not-exist')
        self.run_slot('--ausfuehren', 'leeren', 'account01')
        self.assertFalse((self.root / 'home/account01/angriff').is_symlink())
        self.assertTrue(Path('/etc/passwd').is_file())  # nur Existenz, keine Inhalte

    def test_slot_root_and_parent_symlinks_rejected(self):
        path = self.root / 'home/account01'
        path.rmdir()
        path.symlink_to('/etc', target_is_directory=True)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        path.unlink()
        path.mkdir()
        (self.root / 'mnt/data/account01').rmdir()
        (self.root / 'mnt/data').rmdir()
        (self.root / 'mnt/data').symlink_to('/etc', target_is_directory=True)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)

    def test_live_processes_prevent_any_deletion(self):
        path = self.root / 'home/account01/bleibt'
        path.write_text('synthetisch')
        self.fixture(prozesse=True)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.assertTrue(path.exists())
        self.assertFalse((self.root / 'home/account01/.profile').exists())

    def test_skeleton_symlink_prevents_deletion(self):
        path = self.root / 'home/account01/bleibt'
        path.write_text('synthetisch')
        (self.root / 'etc/cocopi/skel/link').symlink_to('/etc')
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.assertTrue(path.exists())

    def test_quota_residue_prevents_empty_proof(self):
        self.fixture(quota_bytes=1048577)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.run_slot('pruefen', 'account01', success=False)
        result = self.run_slot('status', 'account01')
        self.assertIn('nicht_leer_oder_unbekannt', result.stdout)
        self.assertNotIn('"zustand": "leer"', result.stdout)

    def test_special_file_cannot_hang_proof(self):
        os.mkfifo(self.root / 'home/account01/fifo')
        self.run_slot('pruefen', 'account01', success=False)
        self.run_slot('--ausfuehren', 'leeren', 'account01')

    def test_quota_inode_proof_rejects_small_external_residue(self):
        # Echter quota()-Code mit reinem Q_GETQUOTA-Modul-Fake: kein Kernelaufruf.
        import importlib.machinery
        from types import SimpleNamespace
        from unittest.mock import patch
        loader = importlib.machinery.SourceFileLoader('slotctl_test', str(BASE / 'cocopi-slotctl'))
        spec = importlib.util.spec_from_loader(loader.name, loader)
        module = importlib.util.module_from_spec(spec)
        loader.exec_module(module)
        with patch.dict(os.environ, self.env):
            helper = module.Slots()
        self.addCleanup(os.close, helper.rootfd)
        (self.root / 'etc/cocopi/slots/account01').mkdir(parents=True)
        (self.root / 'etc/cocopi/slots/account01/token').write_text('synthetisch')
        helper.fake = False
        helper.open_dir = lambda relative: os.open(self.root / relative, module.DIRECTORY)
        helper.owned_inodes = lambda fd, uid: 2
        quota_module = SimpleNamespace(query=lambda device, uid: SimpleNamespace(space=64, inodes=3 if device=='/' else 2), limits_match=lambda *args: True)
        quota_spec = SimpleNamespace(loader=SimpleNamespace(exec_module=lambda *args: None))
        real_stat = os.stat
        def token_stat(*args, **kwargs):
            result = real_stat(*args, **kwargs)
            return SimpleNamespace(st_mode=result.st_mode, st_nlink=1, st_uid=20001)
        with patch.object(module.importlib.util, 'spec_from_file_location', return_value=quota_spec), \
             patch.object(module.importlib.util, 'module_from_spec', return_value=quota_module), \
             patch.object(module, 'command', side_effect=lambda *args: args[-1]), \
             patch.object(module.os, 'stat', token_stat):
            self.assertEqual(helper.quota(20001, [1, 2]), 128)
            quota_module.query = lambda *args: SimpleNamespace(space=64, inodes=4)
            with self.assertRaises(RuntimeError):
                helper.quota(20001, [1, 2])

    def test_receipt_matches_clear_and_failed_retry_invalidates_it(self):
        import json
        result = json.loads(self.run_slot('--ausfuehren', 'leeren', 'account01').stdout)
        receipt = self.root / 'var/lib/cocopi-slots/account01.leer.json'
        self.assertEqual(json.loads(receipt.read_text()), result)
        self.assertEqual(receipt.stat().st_mode & 0o777, 0o600)
        self.fixture(prozesse=True)
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.assertFalse(receipt.exists())

    def test_first_proof_is_read_only_unless_explicit(self):
        import shutil
        shutil.copytree(self.root / 'etc/cocopi/skel', self.root / 'home/account01', dirs_exist_ok=True)
        receipt = self.root / 'var/lib/cocopi-slots/account01.leer.json'
        self.run_slot('pruefen', 'account01')
        self.assertFalse(receipt.exists())
        self.run_slot('--ausfuehren', 'pruefen', 'account01')
        self.assertTrue(receipt.exists())

    def test_skeleton_hardlink_blocks_before_deletion(self):
        source = self.root / 'etc/cocopi/skel/.profile'
        os.link(source, self.root / 'etc/cocopi/skel/kopie')
        survivor = self.root / 'home/account01/bleibt'
        survivor.write_text('synthetisch')
        self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)
        self.assertTrue(survivor.exists())

    def test_concurrent_clear_is_blocked(self):
        import fcntl
        with (self.root / 'var/lib/cocopi-slots/pool.lock').open('w') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.run_slot('--ausfuehren', 'leeren', 'account01', success=False)


if __name__ == '__main__':
    unittest.main(verbosity=2)

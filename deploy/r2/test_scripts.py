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
        for path in [*BASE.glob('*.sh'), BASE / 'slots.conf']:
            with self.subTest(path=path.name):
                result = subprocess.run(['bash', '-n', str(path)], capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_default_is_explicit_dry_run(self):
        for script in ('provision.sh', 'check.sh', 'teardown.sh', 'admin-check.sh'):
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
        for script in ('provision.sh', 'check.sh', 'teardown.sh', 'admin-check.sh'):
            with self.subTest(script=script):
                result = self.run_script(script, '--apply', '--slots', 'account01,account02')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('benötigt Root', result.stderr)

    def test_bad_arguments_fail_closed(self):
        for script in ('provision.sh', 'check.sh', 'teardown.sh', 'admin-check.sh'):
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
            f'cocopi-hub@account{n:02}.service:/mnt/data/cocopi/account{n:02}/hub-state'
            for n in range(1, 11)
        ])

    def test_generated_hub_contract_and_private_state_directory(self):
        # Nur Dateierzeugung abfangen: MODE bleibt dry-run; keine Systemänderung.
        fixture = r'''
getent() { return 2; }
managed_file() {
    if [[ $1 == */hub.env ]]; then
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
                         f'HUB_STATE=/mnt/data/cocopi/account{n:02}/hub-state'):
                self.assertIn(line, env_lines)
            self.assertIn(f'runuser -u account{n:02} -- env -i HOME=/home/account{n:02} '
                          f'PATH=/usr/bin:/bin /usr/bin/install -d -m 700 -- '
                          f'/mnt/data/cocopi/account{n:02}/hub-state', result.stdout)
        self.assertNotIn('HUB_STATE=off', result.stdout)
        self.assertFalse(self.sentinel.exists())

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
        self.assertEqual(result.stdout.count('nicht geprueft'), 10)
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
user_systemctl() { [[ $* == 'is-active --quiet cocopi-hub@account01.service' ]]; }
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

    def test_admin_fixture_all_observed_checks_pass(self):
        result = self.admin_fixture('user_systemctl() { return 0; }; hub_loopback() { return 0; };\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        # Bekannte Betriebswerte ergeben noch keinen erfundenen Aktivitätszeitpunkt.
        self.assertEqual(sum(line.endswith('unbekannt') for line in result.stdout.splitlines()), 2)

    def test_admin_activity_is_labelled_service_start_not_student_work(self):
        result = self.admin_fixture(r'''
user_systemctl() {
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
user_systemctl() { if [[ $1 == show ]]; then printf '%s\n' 'secret-unparseable'; else return 0; fi; }
hub_loopback() { return 0; }
''')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn('secret-unparseable', result.stdout)

    def test_listener_identity_and_loopback_fail_closed(self):
        fixture = r'''
UID_NUM=20001
PORT=8801
UNIT=cocopi-hub@account01.service
user_systemctl() {
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


if __name__ == '__main__':
    unittest.main(verbosity=2)

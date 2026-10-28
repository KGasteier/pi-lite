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
                        PYTHONDONTWRITEBYTECODE='1', LC_ALL='C')
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
        for script in ('provision.sh', 'check.sh', 'teardown.sh'):
            with self.subTest(script=script):
                default = self.run_script(script, '--slots', 's01,s02')
                explicit = self.run_script(script, '--dry-run', '--slots', 's01,s02')
                self.assertEqual(default.returncode, 0, default.stderr)
                self.assertEqual(default.stdout, explicit.stdout)
                self.assertIn('dry-run', default.stdout)
                self.assertNotIn('OK:', default.stdout)
                self.assertIn('s01', default.stdout)
                self.assertIn('s02', default.stdout)
                self.assertNotIn('--- s10', default.stdout)

    def test_apply_requires_root_without_mutation(self):
        for script in ('provision.sh', 'check.sh', 'teardown.sh'):
            with self.subTest(script=script):
                result = self.run_script(script, '--apply', '--slots', 's01,s02')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('benötigt Root', result.stderr)

    def test_bad_arguments_fail_closed(self):
        for script in ('provision.sh', 'check.sh', 'teardown.sh'):
            for args in (('--slots', ''), ('--slots', 's00'), ('--slots', 's11'),
                         ('--slots', 's01,s01'), ('--slots', 's01, s02'),
                         ('--slots', '../../root'), ('--slots', 's01;true'),
                         ('--slots',), ('--dry-run', '--apply'),
                         ('--slots', 's01', '--slots', 's02'), ('--unknown',)):
                with self.subTest(script=script, args=args):
                    self.assertNotEqual(self.run_script(script, *args).returncode, 0)

    def test_teardown_needs_explicit_slots(self):
        self.assertNotEqual(self.run_script('teardown.sh').returncode, 0)
        self.assertNotEqual(self.run_script('provision.sh', '--purge').returncode, 0)
        result = self.run_script('teardown.sh', '--slots', 's01,s02', '--purge')
        self.assertEqual(result.returncode, 0)
        self.assertIn('erst nach gültigem Archiv', result.stdout)

    def test_plan_ports_and_budget(self):
        result = self.run_script('provision.sh', '--slots', 's01,s02')
        self.assertIn('Port 8801', result.stdout)
        self.assertIn('Port 8802', result.stdout)
        self.assertIn('9765625 KiB NVMe + 68359375 KiB SATA', result.stdout)
        self.assertIn('Inhalt niemals anzeigen', result.stdout)
        result = self.helper('[[ $((NVME_QUOTA_KIB+DATA_QUOTA_KIB)) == 78125000 ]] && '
                             '[[ $((78125000*1024)) == 80000000000 ]] && '
                             '[[ ${#SLOTS[@]} == 10 && ${SLOTS[9]} == s10 ]]')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_sudo_listing_rejects_any_allow_rule(self):
        fixtures = (
            ('User cocopi-s01 may run the following commands:\n    (ALL : ALL) !ALL', 0, True),
            ('User cocopi-s01 is not allowed to run sudo on host.', 1, True),
            ('sudo: configuration error', 1, False),
            ('User x may run:\n    (root) NOPASSWD: /usr/bin/apt', 0, False),
            ('User x may run:\n    (ALL : ALL) !ALL\n    (root) /bin/bash', 0, False),
            ('unknown listing', 0, False),
        )
        for listing, status, denied in fixtures:
            with self.subTest(listing=listing):
                # Fixture wird als Daten über stdin gelesen, nie evaluiert.
                code = ('sudo() { printf "%s\\n" "$LISTING"; return "$STATUS"; }; '
                        'sudo_denied cocopi-s01')
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
parse_args provision --slots s01,s02
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
        for args in ((), ('s00',), ('../../root',), ('s01',)):
            result = self.run_script('hub-launch.sh', *args)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, '')

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

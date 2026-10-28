import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const source = process.env.I2_SEMINAR_DIR || fileURLToPath(new URL('../../', import.meta.url));
const crate = source && path.resolve(source, 'copi/crates/pi-net');
const ready = crate && fs.existsSync(path.join(crate, 'src/seminar.rs'));

test('Mitsteuern: ohne Zustimmung, nur Ansicht, Ablehnung und Check-out erreichen keinen Hub', {
  timeout: 190000,
  skip: !ready && 'S8-Automat fehlt im gewählten Repo (copi/crates/pi-net/src/seminar.rs); kein UI-/Produktionsnachweis.',
}, t => {
  if (!ready) return;
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cocopi-i2-consent-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  fs.mkdirSync(path.join(tmp, 'src'));
  fs.copyFileSync(fileURLToPath(new URL('./seminar-consent.rs', import.meta.url)), path.join(tmp, 'src/lib.rs'));
  fs.writeFileSync(path.join(tmp, 'Cargo.toml'), `[package]\nname="i2-s6-consent"\nversion="0.0.0"\nedition="2021"\n[dependencies]\npi-net={path=${JSON.stringify(crate)}}\n`);
  const env = { PATH: process.env.PATH, HOME: tmp, XDG_CONFIG_HOME: tmp, PI_CODING_AGENT_DIR: tmp,
    CARGO_HOME: process.env.CARGO_HOME || path.join(os.homedir(), '.cargo'),
    RUSTUP_HOME: process.env.RUSTUP_HOME || path.join(os.homedir(), '.rustup'),
    CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || path.resolve('..', 'target-s6'),
    CARGO_BUILD_JOBS: '3', CARGO_NET_OFFLINE: 'true', RUSTUP_AUTO_INSTALL: '0' };
  for (const args of [['generate-lockfile', '--offline'], ['test', '--offline', '--locked', '--', '--nocapture']]) {
    const r = spawnSync('cargo', args, { cwd: tmp, env, encoding: 'utf8', timeout: 180000, maxBuffer: 2 * 1024 * 1024 });
    assert.equal(r.error, undefined, String(r.error));
    assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
    if (args[0] === 'test') {
      assert.match(r.stdout, /2 passed; 0 failed/);
      t.diagnostic('S8-Automat: 2 unabhängige S6-Angriffstests bestanden; MemoryNet, kein echter Hub-Prompt.');
    }
  }
});

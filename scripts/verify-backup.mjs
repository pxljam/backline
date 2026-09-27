#!/usr/bin/env node
/**
 * Prove the newest dump restores (§15 asks for a *tested* restore, not a
 * documented one).
 *
 *   mise run backup:verify
 *
 * Starts a throwaway PostgreSQL, restores the most recent archive in
 * `infra/backups/`, counts what matters, and tears the container down. Under a
 * minute, nothing else on the machine is touched, and it answers the only
 * question a backup has: would this come back?
 */

import { execFileSync, spawnSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const DIR = join(ROOT, 'infra', 'backups');
const NAME = 'backline-restore-test';
// A Backline database always holds these: without a user nobody can sign in,
// and a collective is what everything else hangs from.
const REQUIRED = ['users', 'collectives'];
// Legitimately empty on a fresh instance — counted, never asserted.
const REPORTED = ['memberships', 'groups', 'events', 'venues', 'assets'];

const G = '\x1b[32m';
const R = '\x1b[31m';
const D = '\x1b[90m';
const O = '\x1b[0m';

function newestDump() {
  let files;
  try {
    files = readdirSync(DIR).filter((f) => f.endsWith('.sql.gz'));
  } catch {
    files = [];
  }
  if (!files.length) {
    console.error(`No dump in infra/backups/. Pull one first: ${D}mise run vps:backup${O}`);
    process.exit(1);
  }
  return join(DIR, files.sort().at(-1));
}

function psql(sql) {
  return execFileSync('docker', ['exec', '-i', NAME, 'psql', '-U', 'postgres', '-d', 'restore', '-tAc', sql], {
    encoding: 'utf8',
  }).trim();
}

const dump = newestDump();
console.log(`restoring ${D}${dump.replace(`${ROOT}/`, '')}${O}`);

spawnSync('docker', ['rm', '-f', NAME], { stdio: 'ignore' });
execFileSync('docker', [
  'run', '-d', '--rm', '--name', NAME,
  '-e', 'POSTGRES_PASSWORD=verify',
  '-e', 'POSTGRES_DB=restore',
  'postgres:16-alpine',
]);

let failed = false;
try {
  for (let i = 0; ; i++) {
    const ready = spawnSync('docker', ['exec', NAME, 'pg_isready', '-U', 'postgres'], { stdio: 'ignore' });
    if (ready.status === 0) break;
    if (i > 60) throw new Error('the throwaway database never became ready');
    execFileSync('sleep', ['1']);
  }

  const restore = spawnSync(
    'sh',
    ['-c', `gunzip -c "${dump}" | docker exec -i ${NAME} psql -q -U postgres -d restore`],
    { encoding: 'utf8' },
  );
  if (restore.status !== 0) throw new Error(restore.stderr?.trim() || 'psql refused the dump');

  for (const table of REQUIRED) {
    const count = Number(psql(`SELECT count(*) FROM ${table}`));
    if (count > 0) console.log(`  ${G}ok  ${O} ${table.padEnd(12)} ${count} row(s)`);
    else {
      console.log(`  ${R}FAIL${O} ${table.padEnd(12)} empty — a restore that loses these is not a restore`);
      failed = true;
    }
  }
  for (const table of REPORTED) {
    console.log(`  ${D}     ${table.padEnd(12)} ${psql(`SELECT count(*) FROM ${table}`)} row(s)${O}`);
  }
} catch (err) {
  console.error(`  ${R}FAIL${O} ${err instanceof Error ? err.message : String(err)}`);
  failed = true;
} finally {
  spawnSync('docker', ['rm', '-f', NAME], { stdio: 'ignore' });
}

console.log(failed ? `\n${R}The backup does not restore cleanly.${O}` : `\n${G}The backup restores.${O}`);
process.exit(failed ? 1 : 0);

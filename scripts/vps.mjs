#!/usr/bin/env node
/**
 * Deploy Backline to the shared VPS.
 *
 *   mise run vps:check     is the machine ready? (read-only)
 *   mise run vps:deploy    ship, build, start, bootstrap, verify
 *   mise run vps:status    what is running, and is it answering
 *   mise run vps:logs      tail a service
 *   mise run vps:stop      take the instance down
 *   mise run vps:backup    pull dumps, the media mirror and the server .env
 *
 * Ports 80 and 443, TLS and the firewall belong to the machine, not to this
 * application — the `vps-infra` repository owns them and runs a shared edge
 * that routes by container labels. So there is no `setup` command here: two
 * repositories provisioning the same box would fight over ufw. `check` asserts
 * the machine is ready and refuses to deploy otherwise.
 *
 * Secrets are generated **on the server**, once, into `~/backline/.env`. They
 * never travel through this laptop and are never written into the repository.
 */

import { execFileSync, spawnSync } from 'node:child_process';
import { promises as dns } from 'node:dns';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import readline from 'node:readline/promises';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const HOST = process.env.VPS_HOST || 'ubuntu@92.222.171.209';
const DOMAIN = process.env.BACKLINE_DOMAIN || 'backline.betafactory.co';
const MEDIA_DOMAIN = process.env.BACKLINE_MEDIA_DOMAIN || 'backline-media.betafactory.co';
const REMOTE_DIR = process.env.VPS_DIR || 'backline';

const COMPOSE = 'docker compose -f docker-compose.yml -f compose.prod.yml --env-file .env';
const SSH_OPTS = ['-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', '-o', 'StrictHostKeyChecking=accept-new'];

const G = '\x1b[32m';
const R = '\x1b[31m';
const Y = '\x1b[33m';
const D = '\x1b[90m';
const B = '\x1b[1m';
const O = '\x1b[0m';

const ok = (l, d = '') => console.log(`  ${G}ok  ${O} ${l}${d ? ` ${D}${d}${O}` : ''}`);
const warn = (l, d = '') => console.log(`  ${Y}warn${O} ${l}${d ? ` ${D}${d}${O}` : ''}`);
const bad = (l, d = '') => console.log(`  ${R}FAIL${O} ${l}${d ? ` ${D}${d}${O}` : ''}`);
const step = (t) => console.log(`\n${B}${t}${O}`);

/** Run on the VPS. Returns trimmed stdout, or null when the command failed. */
function remote(command, { input } = {}) {
  const result = spawnSync('ssh', [...SSH_OPTS, HOST, command], {
    encoding: input ? 'buffer' : 'utf8',
    input,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) return null;
  return (result.stdout?.toString() ?? '').trim();
}

/** Run on the VPS with the output streamed: a 20-minute build must not look hung. */
function remoteLive(command) {
  return spawnSync('ssh', [...SSH_OPTS, '-t', HOST, command], { stdio: 'inherit' }).status === 0;
}

function requireAccess() {
  const who = remote('id -un');
  if (who) return who;
  console.error(`
${R}Cannot reach ${HOST} with a key.${O}

Authorise your key once — it will ask for the password:

    ssh-copy-id -i ~/.ssh/id_ed25519.pub ${HOST}

After that no password is needed again, and it never has to pass through here.
`);
  process.exit(1);
}

async function ask(question) {
  // Without a terminal there is nobody to answer: take the empty value rather
  // than hang a deployment forever.
  if (!process.stdin.isTTY) {
    console.log(`  ${D}${question}(no terminal — left empty)${O}`);
    return '';
  }
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  const answer = await rl.question(question);
  rl.close();
  return answer.trim();
}

/**
 * Everything a fresh clone would hold, as a tarball.
 *
 * `git ls-files` reports files that are still tracked but already deleted from
 * the working tree, and tar aborts the whole transfer on the first missing one
 * — so the list is filtered against the disk before it is used. Ignored files
 * never travel, which is also what keeps the server-side `.env` safe from a
 * redeploy.
 */
function packWorkingTree() {
  const listed = execFileSync('git', ['ls-files', '-co', '--exclude-standard', '-z'], {
    cwd: ROOT,
    maxBuffer: 16 * 1024 * 1024,
  })
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .filter((relative) => existsSync(join(ROOT, relative)));

  try {
    return execFileSync('tar', ['--null', '-T', '-', '-czf', '-'], {
      cwd: ROOT,
      input: `${listed.join('\0')}\0`,
      maxBuffer: 256 * 1024 * 1024,
    });
  } catch (err) {
    throw new Error(`could not pack the working tree: ${err.stderr?.toString().trim() || err.message}`);
  }
}

/** Reads one key out of the server-side .env. */
function serverEnv(key) {
  const env = remote(`cat ~/${REMOTE_DIR}/.env 2>/dev/null`) ?? '';
  return env.match(new RegExp(`^${key}=(.*)$`, 'm'))?.[1] ?? '';
}

/** A/AAAA of a name, resolved from here — see the comment in `check`. */
async function resolveBoth(name) {
  const out = { a: [], aaaa: [] };
  try {
    out.a = await dns.resolve4(name);
  } catch {
    /* reported by the caller */
  }
  try {
    out.aaaa = await dns.resolve6(name);
  } catch {
    /* an AAAA is optional; a wrong one is not */
  }
  return out;
}

// ---------------------------------------------------------------------------

async function check() {
  console.log(`${B}Backline — VPS check${O}`);
  console.log(`${D}${HOST} · ${DOMAIN} · ${MEDIA_DOMAIN}${O}`);

  step('access');
  ok('ssh with a key', `as ${requireAccess()}`);
  ok('host', remote('hostname') ?? '?');
  const ram = remote("free -m | awk '/Mem:/{print $2}'");
  const swap = remote("free -m | awk '/Swap:/{print $2}'");
  ok('resources', `${remote('nproc')} cpu · ${ram} MB RAM · ${swap} MB swap · ${remote("df -h / | awk 'NR==2{print $4}'")} free`);
  // A release build of the whole Rust tree briefly needs more than this box
  // has; without swap the OOM killer picks something that was already serving.
  if (Number(swap) < 1024) warn('little or no swap', 'run `mise run bootstrap` in vps-infra');
  const freeGb = Number(remote("df -BG --output=avail / | tail -1 | tr -dc '0-9'") ?? 0);
  if (freeGb < 10) warn('less than 10 GB free', `${freeGb} GB — images and build cache need room`);

  step('runtime');
  const docker = remote('docker --version 2>/dev/null');
  docker ? ok('docker', docker) : bad('docker is not installed', 'run `mise run bootstrap` in vps-infra');
  const compose = remote('docker compose version --short 2>/dev/null');
  compose ? ok('docker compose', `v${compose}`) : bad('docker compose plugin missing');

  step('shared edge');
  // The edge owns 80/443 for the whole machine. Here that is the healthy state,
  // not a conflict: an application that published those ports would break every
  // other project on the box.
  const network = remote('docker network inspect edge >/dev/null 2>&1 && echo yes');
  network === 'yes'
    ? ok('the `edge` network exists')
    : bad('no `edge` network', 'run `mise run proxy:up` in vps-infra');
  const edgeUp = remote("docker ps --filter label=com.docker.compose.project=edge --format '{{.Names}}' | tr '\\n' ' '");
  edgeUp ? ok('edge running', edgeUp.trim()) : bad('the edge is not running', 'run `mise run proxy:up` in vps-infra');
  const listening = remote("ss -tlnp 2>/dev/null | awk 'NR>1{print $4}' | grep -E ':(80|443)$' | tr '\\n' ' ' || true");
  listening ? ok('80/443 are held', listening.trim()) : bad('nothing listens on 80/443', 'the edge is down');

  step('names');
  // Resolved from here, never on the VPS: Ubuntu maps its own hostname to
  // 127.0.1.1 in /etc/hosts, so asking the machine answers with the loopback
  // and looks like a broken zone.
  const publicIp = remote('curl -s --max-time 8 https://api.ipify.org || true');
  for (const name of [DOMAIN, MEDIA_DOMAIN]) {
    const { a, aaaa } = await resolveBoth(name);
    if (!a.length) bad('does not resolve', name);
    else if (publicIp && a.includes(publicIp)) ok(name, `A → ${a.join(', ')}`);
    else warn(`${name} does not point at this machine`, `A → ${a.join(', ')}, machine → ${publicIp}`);
    // Let's Encrypt prefers IPv6: an AAAA pointing elsewhere fails issuance
    // while the IPv4 looks perfectly healthy.
    if (aaaa.length) ok(`${name} AAAA`, aaaa.join(', '));
  }

  step('deployment');
  const present = remote(`test -d ~/${REMOTE_DIR} && echo yes`);
  if (present === 'yes') {
    ok('already deployed', `~/${REMOTE_DIR}`);
    const running = remote(`cd ~/${REMOTE_DIR} && ${COMPOSE} ps --format '{{.Service}} {{.State}}' 2>/dev/null || true`);
    console.log(running ? running.split('\n').map((l) => `         ${l}`).join('\n') : '         (nothing running)');
    const dump = remote(`ls -t ~/${REMOTE_DIR}/backups/db-*.sql.gz 2>/dev/null | head -1`);
    dump ? ok('latest dump', dump.split('/').pop()) : warn('no backup yet', 'the cron installs on first deploy');
  } else ok('not deployed yet', 'run: mise run vps:deploy');

  console.log(`\n${D}Reminder: the Telegram Login Widget only works once BotFather`);
  console.log(`/setdomain points at ${DOMAIN}.${O}\n`);
}

// ---------------------------------------------------------------------------

async function deploy() {
  console.log(`${B}Backline — deploy to ${DOMAIN}${O}`);
  requireAccess();

  if (!remote('docker compose version --short 2>/dev/null')) {
    bad('docker compose is not available on the VPS', 'run `mise run bootstrap` in vps-infra');
    process.exit(1);
  }
  if (remote('docker network inspect edge >/dev/null 2>&1 && echo yes') !== 'yes') {
    bad('no `edge` network on the VPS', 'run `mise run proxy:up` in vps-infra');
    process.exit(1);
  }

  step('ship');
  const tarball = packWorkingTree();
  const shipped = spawnSync(
    'ssh',
    [...SSH_OPTS, HOST, `mkdir -p ~/${REMOTE_DIR} && tar -C ~/${REMOTE_DIR} -xzf -`],
    { input: tarball, encoding: 'buffer' },
  );
  if (shipped.status !== 0) {
    bad('copy failed', shipped.stderr?.toString().trim());
    process.exit(1);
  }
  ok('sources copied', `${(tarball.length / 1024).toFixed(0)} KB → ~/${REMOTE_DIR}`);

  step('secrets');
  const envPath = `~/${REMOTE_DIR}/.env`;
  if (remote(`test -f ${envPath} && echo yes`) === 'yes') {
    ok('server-side .env already present', 'left untouched');
  } else {
    // The bot token is the one value that cannot be generated. Asked once here,
    // written straight to the server, never stored on this laptop.
    const token = process.env.TELEGRAM_BOT_TOKEN || (await ask('Telegram bot token (empty = bot disabled): '));
    const username = token
      ? process.env.TELEGRAM_BOT_USERNAME || (await ask('Telegram bot username (without @): '))
      : '';
    const adminEmail = process.env.BOOTSTRAP_ADMIN_EMAIL || (await ask('First instance admin — email: '));
    if (!adminEmail) {
      bad('an admin email is required', 'nobody could sign in otherwise');
      process.exit(1);
    }
    const adminName = process.env.BOOTSTRAP_ADMIN_NAME || (await ask('First instance admin — display name [Admin instance]: '));
    const collective =
      process.env.BOOTSTRAP_COLLECTIVE_NAME ??
      (await ask('First collective name (empty = create it later from the app): '));

    // `openssl rand -hex` only: base64 emits `+ / =`, and MINIO_ROOT_PASSWORD is
    // interpolated into a `/bin/sh -c` string in storage-init — a slash or a
    // shell metacharacter there produces a bucket that never gets created.
    const created = remote(
      `umask 077 && cat > ${envPath} <<EOF
BACKLINE_DOMAIN=${DOMAIN}
BACKLINE_MEDIA_DOMAIN=${MEDIA_DOMAIN}
POSTGRES_USER=backline
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_DB=backline
MINIO_ROOT_USER=backline
MINIO_ROOT_PASSWORD=$(openssl rand -hex 32)
S3_BUCKET=backline
S3_REGION=us-east-1
SESSION_SECRET=$(openssl rand -hex 32)
RUST_LOG=info,backline=info,sqlx=warn
TELEGRAM_BOT_TOKEN="${token}"
TELEGRAM_BOT_USERNAME="${username}"
BOOTSTRAP_ADMIN_EMAIL="${adminEmail}"
BOOTSTRAP_ADMIN_PASSWORD=$(openssl rand -hex 16)
BOOTSTRAP_ADMIN_NAME="${adminName || 'Admin instance'}"
BOOTSTRAP_COLLECTIVE_NAME="${collective}"
EOF
echo created`,
    );
    if (created !== 'created') {
      bad('could not write the server .env');
      process.exit(1);
    }
    ok('secrets generated on the server', envPath);
  }

  step('build');
  // One service at a time. `up --build` fans out, and rustc, vite and a Chrome
  // download in flight together are what makes a 4 GB box OOM.
  //
  // VPS_LOCAL_BUILD names the services to build here instead and ship as
  // images. The box shares 4 GB and 2 GB of swap with every other project on
  // it, and a release build of the Rust tree is the one step that can push it
  // far enough for the OOM killer to take something that was already serving —
  // so `api` is the candidate. `stills` is not: it is heavy on disk, not on
  // memory, and its Chrome layer costs far more to upload than to fetch there.
  const local = (process.env.VPS_LOCAL_BUILD ?? '').split(/[ ,]+/).filter(Boolean);
  for (const service of ['api', 'web', 'stills']) {
    if (local.includes(service)) {
      buildHereAndShip(service);
      continue;
    }
    console.log(`  ${D}building ${service} on the VPS…${O}`);
    if (!remoteLive(`cd ~/${REMOTE_DIR} && ${COMPOSE} build ${service}`)) {
      bad(`build failed: ${service}`, 'try: VPS_LOCAL_BUILD=api mise run vps:deploy');
      process.exit(1);
    }
    ok(`built ${service}`);
  }

  step('images');
  ensureExternalImages();

  step('start');
  // Never `… | tail`: the pipeline would report tail's status and a failed
  // start would read as a success.
  const up = remote(
    `cd ~/${REMOTE_DIR} && ${COMPOSE} up -d > /tmp/backline-up.log 2>&1 && echo __UP_OK__; tail -8 /tmp/backline-up.log`,
  );
  console.log((up ?? '').replace('__UP_OK__', '').trim().split('\n').map((l) => `         ${D}${l}${O}`).join('\n'));
  if (!up?.includes('__UP_OK__')) {
    bad('compose failed to start the stack', 'see: mise run vps:logs');
    process.exit(1);
  }

  // Migrations run themselves on api boot (RUN_MIGRATIONS=1).
  let healthy = false;
  for (let i = 0; i < 40; i++) {
    if (remote(`cd ~/${REMOTE_DIR} && ${COMPOSE} ps api --format '{{.Health}}' 2>/dev/null`) === 'healthy') {
      healthy = true;
      break;
    }
    execFileSync('sleep', ['3']);
  }
  healthy ? ok('api healthy') : warn('api not healthy yet', 'see: mise run vps:logs api');

  step('first admin');
  // Idempotent: with an instance admin already present it changes nothing, so
  // this runs on every deployment without thinking about it.
  // `--env-file` feeds compose's interpolation, not the container. The values
  // are sourced from the server-side .env and passed through by name, so the
  // admin password never lands in the long-running container's environment.
  const pass = ['BOOTSTRAP_ADMIN_EMAIL', 'BOOTSTRAP_ADMIN_PASSWORD', 'BOOTSTRAP_ADMIN_NAME', 'BOOTSTRAP_COLLECTIVE_NAME', 'BOOTSTRAP_COLLECTIVE_SLUG']
    .map((v) => `-e ${v}`)
    .join(' ');
  const bootstrap = remote(
    `cd ~/${REMOTE_DIR} && set -a && . ./.env && set +a && ${COMPOSE} run --rm --no-deps ${pass} api /usr/local/bin/bootstrap-admin 2>&1 | tail -4`,
  );
  console.log((bootstrap ?? 'bootstrap-admin failed').split('\n').map((l) => `         ${D}${l}${O}`).join('\n'));
  if (bootstrap?.includes('Error')) {
    bad('the first admin was not created', 'nobody can sign in — see: mise run vps:logs api');
    process.exitCode = 1;
  }

  step('backups');
  installBackupCron();

  step('verify');
  // First boot needs one ACME issuance per hostname; give it room.
  let health = null;
  for (let i = 0; i < 30; i++) {
    health = remote(`curl -fsS --max-time 6 https://${DOMAIN}/health 2>/dev/null || true`);
    if (health) break;
    execFileSync('sleep', ['3']);
  }
  if (health) ok('reachable over HTTPS', `${DOMAIN}/health → ${health}`);
  else {
    bad('not answering over HTTPS yet', 'check certificate issuance: mise run proxy:logs in vps-infra');
    process.exitCode = 1;
  }
  const media = remote(`curl -s -o /dev/null -w '%{http_code}' --max-time 6 https://${MEDIA_DOMAIN}/ 2>/dev/null || true`);
  // MinIO answers 403 to an unsigned request — proof that TLS and routing work.
  media && media !== '000'
    ? ok('media host answering', `${MEDIA_DOMAIN} → HTTP ${media}`)
    : warn('media host not answering yet', MEDIA_DOMAIN);

  console.log(`\n${B}Live${O}`);
  console.log(`  application    https://${DOMAIN}`);
  console.log(`  sign in        ${serverEnv('BOOTSTRAP_ADMIN_EMAIL')} / ${serverEnv('BOOTSTRAP_ADMIN_PASSWORD')}`);
  console.log(`  ${D}the password lives only in ${envPath} — change it after the first sign-in${O}`);
  console.log(`\n  ${D}accounts:  ssh ${HOST} "cd ${REMOTE_DIR} && ${COMPOSE} run --rm --no-deps api accounts list"${O}\n`);
}


/**
 * Build one service here and stream the image to the VPS.
 *
 * Built against the base compose alone: `compose.prod.yml` would demand the
 * production secrets just to parse, and it adds nothing to the build — same
 * context, same Dockerfile, same resulting `backline-<service>` tag that the
 * VPS then finds already present and does not rebuild.
 */
function buildHereAndShip(service) {
  console.log(`  ${D}building ${service} here…${O}`);
  const built = spawnSync('docker', ['compose', '-f', 'docker-compose.yml', 'build', service], {
    cwd: ROOT,
    stdio: 'inherit',
  });
  if (built.status !== 0) {
    bad(`local build failed: ${service}`);
    process.exit(1);
  }

  const image = `backline-${service}`;
  const size = spawnSync('docker', ['image', 'inspect', image, '--format', '{{.Size}}'], { encoding: 'utf8' });
  const mb = size.status === 0 ? `${(Number(size.stdout.trim()) / 1e6).toFixed(0)} MB uncompressed` : '';
  console.log(`  ${D}shipping ${image} ${mb}…${O}`);

  // Streamed, never staged on disk at either end.
  const shipped = spawnSync(
    'sh',
    ['-c', `docker save ${image} | gzip -1 | ssh ${SSH_OPTS.join(' ')} ${HOST} 'gunzip | docker load'`],
    { cwd: ROOT, stdio: 'inherit' },
  );
  if (shipped.status !== 0) {
    bad(`could not ship ${image}`);
    process.exit(1);
  }
  ok(`built here and shipped: ${service}`);
}


/**
 * Make sure every third-party image the stack needs is on the machine.
 *
 * MinIO stopped publishing pullable images: the Docker Hub repository is gone
 * and quay.io answers 401 to an anonymous request, pinned tags included. A
 * deployment must not depend on a registry that can revoke access overnight, so
 * an image the VPS cannot pull is shipped from here when this machine still has
 * it — the same stream used for a locally built image.
 *
 * This is a stopgap, not an answer: see DEPLOY.md, "object storage".
 */
function ensureExternalImages() {
  const listed = remote(`cd ~/${REMOTE_DIR} && ${COMPOSE} config --images 2>/dev/null`) ?? '';
  const external = listed
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    // Anything built from this repository is already dealt with.
    .filter((image) => !image.startsWith('backline-'));

  for (const image of external) {
    if (remote(`docker image inspect ${image} >/dev/null 2>&1 && echo yes`) === 'yes') {
      ok('present', image);
      continue;
    }
    if (remote(`docker pull -q ${image} >/dev/null 2>&1 && echo yes`) === 'yes') {
      ok('pulled', image);
      continue;
    }
    const here = spawnSync('docker', ['image', 'inspect', image], { stdio: 'ignore' });
    if (here.status !== 0) {
      bad('unavailable and not held here', image);
      process.exit(1);
    }
    warn('not pullable — shipping the copy held here', image);
    const shipped = spawnSync(
      'sh',
      ['-c', `docker save ${image} | gzip -1 | ssh ${SSH_OPTS.join(' ')} ${HOST} 'gunzip | docker load'`],
      { cwd: ROOT, stdio: 'inherit' },
    );
    if (shipped.status !== 0) {
      bad(`could not ship ${image}`);
      process.exit(1);
    }
    ok('shipped', image);
  }
}

/** A daily dump and media mirror on the server itself (§15). */
function installBackupCron() {
  remote(`chmod +x ~/${REMOTE_DIR}/infra/backup.sh`);
  const line = `10 3 * * * cd $HOME/${REMOTE_DIR} && ./infra/backup.sh >> $HOME/${REMOTE_DIR}/backups/backup.log 2>&1`;
  const installed = remote(
    `mkdir -p ~/${REMOTE_DIR}/backups && (crontab -l 2>/dev/null | grep -v 'backline/infra/backup.sh' | grep -v "${REMOTE_DIR}/infra/backup.sh"; echo "${line}") | crontab - && echo done`,
  );
  installed === 'done' ? ok('daily backup at 03:10', `~/${REMOTE_DIR}/backups`) : warn('could not install the cron entry');
}

// ---------------------------------------------------------------------------

function status() {
  requireAccess();
  const ps = remote(`cd ~/${REMOTE_DIR} && ${COMPOSE} ps --format '{{.Service}}\t{{.State}}\t{{.Status}}' 2>/dev/null`);
  console.log(ps || '(nothing deployed)');
  console.log(`\nhttps://${DOMAIN}/health → ${remote(`curl -fsS --max-time 6 https://${DOMAIN}/health 2>/dev/null || echo unreachable`)}`);
  console.log(`https://${MEDIA_DOMAIN}/ → HTTP ${remote(`curl -s -o /dev/null -w '%{http_code}' --max-time 6 https://${MEDIA_DOMAIN}/ 2>/dev/null || echo unreachable`)}`);
  const dump = remote(`ls -t ~/${REMOTE_DIR}/backups/db-*.sql.gz 2>/dev/null | head -1`);
  console.log(`\nlatest dump: ${dump ? dump.split('/').pop() : 'none yet'}`);
  console.log(`disk: ${remote("df -h / | awk 'NR==2{print $4\" free of \"$2}'")} · ${remote("free -m | awk '/Mem:/{print $7\" MB available\"}'")}`);
}

function logs() {
  requireAccess();
  const service = process.argv[3] && !process.argv[3].startsWith('-') ? process.argv[3] : '';
  spawnSync('ssh', [...SSH_OPTS, '-t', HOST, `cd ~/${REMOTE_DIR} && ${COMPOSE} logs -f --tail 80 ${service}`], {
    stdio: 'inherit',
  });
}

function stop() {
  requireAccess();
  console.log(remote(`cd ~/${REMOTE_DIR} && ${COMPOSE} down 2>&1 | tail -4`) ?? 'failed');
}

/** Pull what the server holds: dumps, the media mirror, and its secrets. */
function backup() {
  requireAccess();
  const target = join(ROOT, 'infra', 'backups');
  mkdirSync(target, { recursive: true });
  console.log(`${B}Backline — pulling backups${O} ${D}→ infra/backups${O}`);

  // Take a fresh dump first, so the pull is not a day old.
  const run = remote(`cd ~/${REMOTE_DIR} && ./infra/backup.sh 2>&1 | tail -3`);
  console.log((run ?? 'the server-side backup failed').split('\n').map((l) => `  ${D}${l}${O}`).join('\n'));

  const rsync = spawnSync('rsync', ['-az', '--delete', '-e', `ssh ${SSH_OPTS.join(' ')}`, `${HOST}:${REMOTE_DIR}/backups/`, `${target}/`], {
    stdio: 'inherit',
  });
  if (rsync.status !== 0) {
    bad('rsync failed', 'is rsync installed on both ends?');
    process.exit(1);
  }
  ok('dumps and media mirror pulled', target);

  // The secrets exist nowhere else: losing the VPS without them means losing
  // the sessions, the bucket and the database.
  const env = remote(`cat ~/${REMOTE_DIR}/.env`);
  if (env) {
    writeFileSync(join(target, 'server.env'), `${env}\n`, { mode: 0o600 });
    ok('server .env pulled', 'infra/backups/server.env — keep it out of the repository');
  }
}

// ---------------------------------------------------------------------------

const COMMANDS = { check, deploy, status, logs, stop, backup };
const command = process.argv[2];
if (!command || !COMMANDS[command]) {
  console.error(`usage: vps.mjs <${Object.keys(COMMANDS).join('|')}>`);
  process.exit(2);
}
await COMMANDS[command]();

#!/usr/bin/env node
/**
 * `backline` — the render CLI (§10.3).
 *
 * It authenticates against the instance, claims a job, downloads the
 * description and the media, renders locally with Remotion, uploads the
 * finished file, and reports progress. **The VPS never encodes.**
 *
 *   backline login                  link this machine to the account, by token
 *   backline jobs                   list pending renders
 *   backline render                 take a job, render it, upload the result
 *   backline render --watch         daemon: process jobs as they arrive
 *   backline render <id> --preview  fast low-resolution render
 */

import readline from "node:readline/promises";
import { stdin, stdout } from "node:process";
import { Api } from "./api";
import * as config from "./config";
import { detect } from "./machine";
import { renderJob } from "./render";

const args = process.argv.slice(2);
const command = args[0] ?? "help";

function flag(name: string): boolean {
  return args.includes(`--${name}`);
}

function option(name: string): string | undefined {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : undefined;
}

async function ask(question: string): Promise<string> {
  const rl = readline.createInterface({ input: stdin, output: stdout });
  const answer = await rl.question(question);
  rl.close();
  return answer.trim();
}

async function login(): Promise<void> {
  const existing = await config.load();
  const fallbackUrl = existing?.apiUrl ?? "https://backline.betafactory.co";
  const apiUrl =
    option("url") ||
    (await ask(`Instance address [${fallbackUrl}]: `)) ||
    fallbackUrl;

  // The token is created in the application's "Rendus" screen, and shown
  // there only once. It can be revoked at any time.
  const token =
    option("token") ?? (await ask("Machine token (Rendus screen in the app): "));
  if (!token) {
    console.error("No token provided.");
    process.exit(1);
  }

  const caps = await detect();
  const machineName =
    option("name") || (await ask(`Name for this machine [${hostname()}]: `)) || hostname();

  const api = new Api({ apiUrl, token, machineName });
  const { job } = await api.claim(caps).catch((e: Error) => {
    console.error(`Connection refused: ${e.message}`);
    process.exit(1);
  });

  const file = await config.save({ apiUrl, token, machineName });
  console.log(`Machine linked. Configuration: ${file}`);
  console.log(
    `Hardware: ${caps.gpu ? `GPU ${caps.gpuKind}` : "CPU only"}, ${caps.concurrency} task(s) in parallel.`,
  );
  if (job) {
    console.log(`A job is already waiting (${job.id}). Run \`backline render\`.`);
  }
}

function hostname(): string {
  return process.env.HOSTNAME ?? "machine";
}

async function jobs(): Promise<void> {
  const cfg = await config.require_();
  const api = new Api(cfg);
  const { job } = await api.claim(await detect());
  if (!job) {
    console.log("No pending render.");
    return;
  }
  // Claiming then releasing would misreport the state: we took the job, so
  // say so plainly.
  console.log(`1 render claimed: ${job.id} (${job.kind}). Run \`backline render\`.`);
}

async function renderOnce(api: Api, caps: Awaited<ReturnType<typeof detect>>): Promise<boolean> {
  const { job } = await api.claim(caps);
  if (!job) return false;

  console.log(`→ job ${job.id} (${job.kind})`);
  try {
    const bundle = await api.bundle(job.id);
    console.log(`  "${bundle.name}" — ${Object.keys(bundle.media).length} media file(s) to download`);

    let last = -1;
    const result = await renderJob(bundle, caps, (ratio) => {
      const pct = Math.floor(ratio * 100);
      if (pct !== last && pct % 5 === 0) {
        last = pct;
        stdout.write(`\r  rendering ${pct}%   `);
        void api.progress(job.id, ratio);
      }
    });
    stdout.write("\r  rendering 100%   \n");

    await api.complete(job.id, result.file, result.filename);
    console.log(`  ✓ ${result.filename} uploaded (${(result.file.length / 1e6).toFixed(1)} MB)`);
    return true;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`  ✗ failed: ${message}`);
    // The failure is reported back to the application: a job that fails
    // silently is worse than a job that fails.
    await api.fail(job.id, message);
    return true;
  }
}

async function render(): Promise<void> {
  const cfg = await config.require_();
  const api = new Api(cfg);
  const caps = await detect();

  console.log(
    `${cfg.machineName} — ${caps.gpu ? `GPU ${caps.gpuKind}` : "CPU only (slower, but it works)"}`,
  );

  if (!flag("watch")) {
    const did = await renderOnce(api, caps);
    if (!did) console.log("No pending render.");
    return;
  }

  console.log("Daemon mode: processing jobs as they arrive. Ctrl-C to stop.");
  let idle = 0;
  for (;;) {
    const did = await renderOnce(api, caps).catch((e: Error) => {
      console.error(`  ✗ ${e.message}`);
      return false;
    });
    if (did) {
      idle = 0;
      continue;
    }
    // Progressive backoff: do not hammer the API when there is nothing to do.
    idle = Math.min(idle + 1, 6);
    await new Promise((r) => setTimeout(r, 5000 * idle));
  }
}

function help(): void {
  console.log(`backline — video rendering for Backline

  backline login                  link this machine to the account, by token
  backline jobs                   list pending renders
  backline render                 take a job, render it, upload the result
  backline render --watch         daemon: process jobs as they arrive
  backline render --preview       fast low-resolution render

Options: --url <instance>  --token <token>  --name <machine name>
`);
}

const commands: Record<string, () => Promise<void> | void> = {
  login,
  jobs,
  render,
  help,
};

const run = commands[command] ?? help;
void (async () => {
  try {
    await run();
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  }
})();

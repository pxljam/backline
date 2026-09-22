#!/usr/bin/env node
/**
 * `backline` — la CLI de rendu (§10.3).
 *
 * Elle s'authentifie aupres de l'instance, reclame un job, telecharge la
 * description et les medias, rend en local avec Remotion, renvoie le fichier
 * fini, et remonte sa progression. **Le VPS n'encode jamais.**
 *
 *   backline login                  associe la machine au compte, via un jeton
 *   backline jobs                   liste les rendus en attente
 *   backline render                 prend un job, le rend, renvoie le resultat
 *   backline render --watch         demon : traite les jobs au fil de l'eau
 *   backline render <id> --preview  rendu rapide basse definition
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
    (await ask(`Adresse de l'instance [${fallbackUrl}] : `)) ||
    fallbackUrl;

  // Le jeton se cree dans l'ecran « Rendus » de l'application, et n'y est
  // montre qu'une fois. Il est revocable a tout moment.
  const token =
    option("token") ?? (await ask("Jeton de machine (ecran Rendus de l'application) : "));
  if (!token) {
    console.error("Aucun jeton fourni.");
    process.exit(1);
  }

  const caps = await detect();
  const machineName =
    option("name") || (await ask(`Nom de cette machine [${hostname()}] : `)) || hostname();

  const api = new Api({ apiUrl, token, machineName });
  const { job } = await api.claim(caps).catch((e: Error) => {
    console.error(`Connexion refusee : ${e.message}`);
    process.exit(1);
  });

  const file = await config.save({ apiUrl, token, machineName });
  console.log(`Machine associee. Configuration : ${file}`);
  console.log(
    `Materiel : ${caps.gpu ? `GPU ${caps.gpuKind}` : "processeur seul"}, ${caps.concurrency} tache(s) en parallele.`,
  );
  if (job) {
    console.log(`Un job attend deja (${job.id}). Lancer \`backline render\`.`);
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
    console.log("Aucun rendu en attente.");
    return;
  }
  // Reclamer puis relacher fausserait l'etat : on a pris le job, autant le
  // dire clairement.
  console.log(`1 rendu reclame : ${job.id} (${job.kind}). Lancer \`backline render\`.`);
}

async function renderOnce(api: Api, caps: Awaited<ReturnType<typeof detect>>): Promise<boolean> {
  const { job } = await api.claim(caps);
  if (!job) return false;

  console.log(`→ job ${job.id} (${job.kind})`);
  try {
    const bundle = await api.bundle(job.id);
    console.log(`  « ${bundle.name} » — ${Object.keys(bundle.media).length} media(s) a telecharger`);

    let last = -1;
    const result = await renderJob(bundle, caps, (ratio) => {
      const pct = Math.floor(ratio * 100);
      if (pct !== last && pct % 5 === 0) {
        last = pct;
        stdout.write(`\r  rendu ${pct}%   `);
        void api.progress(job.id, ratio);
      }
    });
    stdout.write("\r  rendu 100%   \n");

    await api.complete(job.id, result.file, result.filename);
    console.log(`  ✓ ${result.filename} renvoye (${(result.file.length / 1e6).toFixed(1)} Mo)`);
    return true;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`  ✗ echec : ${message}`);
    // L'echec remonte a l'application : un job qui echoue en silence est pire
    // qu'un job qui echoue.
    await api.fail(job.id, message);
    return true;
  }
}

async function render(): Promise<void> {
  const cfg = await config.require_();
  const api = new Api(cfg);
  const caps = await detect();

  console.log(
    `${cfg.machineName} — ${caps.gpu ? `GPU ${caps.gpuKind}` : "processeur seul (plus lent, mais ca passe)"}`,
  );

  if (!flag("watch")) {
    const did = await renderOnce(api, caps);
    if (!did) console.log("Aucun rendu en attente.");
    return;
  }

  console.log("Mode demon : traitement des jobs au fil de l'eau. Ctrl-C pour arreter.");
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
    // Attente progressive : on ne martele pas l'API quand il n'y a rien.
    idle = Math.min(idle + 1, 6);
    await new Promise((r) => setTimeout(r, 5000 * idle));
  }
}

function help(): void {
  console.log(`backline — rendu video pour Backline

  backline login                  associe la machine au compte, via un jeton
  backline jobs                   liste les rendus en attente
  backline render                 prend un job, le rend, renvoie le resultat
  backline render --watch         demon : traite les jobs au fil de l'eau
  backline render --preview       rendu rapide basse definition

Options : --url <instance>  --token <jeton>  --name <nom de machine>
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

import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

/**
 * The CLI stores one thing only: the instance address and the **machine
 * token**, revocable from the application (§10.3). No password, no user
 * session.
 */
export interface Config {
  apiUrl: string;
  token: string;
  machineName: string;
}

const dir = path.join(os.homedir(), ".backline");
const file = path.join(dir, "config.json");

export async function load(): Promise<Config | null> {
  try {
    return JSON.parse(await fs.readFile(file, "utf8")) as Config;
  } catch {
    return null;
  }
}

export async function save(config: Config): Promise<string> {
  await fs.mkdir(dir, { recursive: true });
  // The token grants access to job media: it has no business being
  // world-readable.
  await fs.writeFile(file, JSON.stringify(config, null, 2), { mode: 0o600 });
  return file;
}

export async function require_(): Promise<Config> {
  const config = await load();
  if (!config) {
    throw new Error("machine not linked — run `backline login` first");
  }
  return config;
}

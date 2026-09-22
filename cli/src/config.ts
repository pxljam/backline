import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

/**
 * La CLI ne garde qu'une chose : l'adresse de l'instance et le **jeton de
 * machine**, revocable depuis l'application (§10.3). Aucun mot de passe,
 * aucune session utilisateur.
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
  // Le jeton donne acces aux medias des jobs : il n'a rien a faire en
  // lecture pour tout le monde.
  await fs.writeFile(file, JSON.stringify(config, null, 2), { mode: 0o600 });
  return file;
}

export async function require_(): Promise<Config> {
  const config = await load();
  if (!config) {
    throw new Error("machine non associee — lancer `backline login` d'abord");
  }
  return config;
}

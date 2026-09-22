import os from "node:os";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);

/**
 * Capacites de la machine.
 *
 * « La CLI utilise le GPU quand la machine en a un, et retombe sur le CPU
 * sinon. Aucune machine n'est exclue : un rendu sur processeur est simplement
 * plus lent. » (§10.3)
 */
export interface Capabilities {
  gpu: boolean;
  gpuKind: string | null;
  concurrency: number;
  platform: string;
  cpus: number;
}

export async function detect(): Promise<Capabilities> {
  const cpus = os.cpus().length || 2;
  const { gpu, gpuKind } = await detectGpu();
  return {
    gpu,
    gpuKind,
    // Le nombre de taches paralleles s'adapte a la machine : on laisse deux
    // coeurs a son proprietaire, qui s'en sert peut-etre pendant ce temps.
    concurrency: Math.max(1, Math.min(8, cpus - 2)),
    platform: `${os.platform()}-${os.arch()}`,
    cpus,
  };
}

async function detectGpu(): Promise<{ gpu: boolean; gpuKind: string | null }> {
  if (os.platform() === "darwin") {
    // Tout Mac recent a un GPU utilisable par Chromium (Metal via ANGLE).
    return { gpu: true, gpuKind: "metal" };
  }
  try {
    const { stdout } = await run("nvidia-smi", ["--query-gpu=name", "--format=csv,noheader"]);
    const name = stdout.trim().split("\n")[0];
    if (name) return { gpu: true, gpuKind: name };
  } catch {
    // nvidia-smi absent : ce n'est pas une erreur, c'est une machine sans GPU
    // NVIDIA. Le rendu se fera sur processeur.
  }
  return { gpu: false, gpuKind: null };
}

/**
 * Backend OpenGL passe a Chromium. `angle` exploite le GPU ; `swangle` est le
 * rendu logiciel, plus lent mais universel — et **deterministe**, ce qui sert
 * l'exigence « deux rendus donnent deux fichiers identiques » (§18).
 */
export function glBackend(caps: Capabilities): "angle" | "swangle" {
  return caps.gpu ? "angle" : "swangle";
}

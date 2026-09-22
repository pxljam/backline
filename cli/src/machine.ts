import os from "node:os";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);

/**
 * What the machine can do.
 *
 * "The CLI uses the GPU when the machine has one, and falls back to the CPU
 * otherwise. No machine is excluded: a CPU render is simply slower." (§10.3)
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
    // Parallelism follows the machine: leave two cores to its owner, who may
    // well be using it in the meantime.
    concurrency: Math.max(1, Math.min(8, cpus - 2)),
    platform: `${os.platform()}-${os.arch()}`,
    cpus,
  };
}

async function detectGpu(): Promise<{ gpu: boolean; gpuKind: string | null }> {
  if (os.platform() === "darwin") {
    // Every recent Mac has a GPU Chromium can use (Metal through ANGLE).
    return { gpu: true, gpuKind: "metal" };
  }
  try {
    const { stdout } = await run("nvidia-smi", ["--query-gpu=name", "--format=csv,noheader"]);
    const name = stdout.trim().split("\n")[0];
    if (name) return { gpu: true, gpuKind: name };
  } catch {
    // No nvidia-smi: not an error, just a machine without an NVIDIA GPU. The
    // render will run on the CPU.
  }
  return { gpu: false, gpuKind: null };
}

/**
 * OpenGL backend handed to Chromium. `angle` uses the GPU; `swangle` is
 * software rendering, slower but universal — and **deterministic**, which
 * serves the "two renders produce two identical files" requirement (§18).
 */
export function glBackend(caps: Capabilities): "angle" | "swangle" {
  return caps.gpu ? "angle" : "swangle";
}

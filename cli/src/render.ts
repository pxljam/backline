import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { bundle } from "@remotion/bundler";
import { renderMedia, selectComposition } from "@remotion/renderer";
import type { JobBundle } from "./api";
import { glBackend, type Capabilities } from "./machine";

const here = path.dirname(fileURLToPath(import.meta.url));

let bundlePromise: Promise<string> | null = null;

function serveUrl(): Promise<string> {
  bundlePromise ??= bundle({ entryPoint: path.join(here, "remotion", "index.ts") });
  return bundlePromise;
}

export interface RenderResult {
  file: Buffer;
  filename: string;
}

/**
 * Renders a composition locally. The GPU is used when present, the CPU
 * otherwise — no machine is excluded (§10.3).
 */
export async function renderJob(
  job: JobBundle,
  caps: Capabilities,
  onProgress: (ratio: number) => void,
): Promise<RenderResult> {
  const inputProps = {
    spec: job.spec,
    brand: job.brand,
    data: job.data,
    media: job.media,
  };

  const url = await serveUrl();
  const composition = await selectComposition({ serveUrl: url, id: "video", inputProps });

  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "backline-render-"));
  const output = path.join(dir, "out.mp4");
  const preview = job.kind === "preview";

  try {
    await renderMedia({
      composition: preview
        ? // Fast low-resolution render, just to check the pacing.
          { ...composition, width: even(composition.width / 2), height: even(composition.height / 2) }
        : composition,
      serveUrl: url,
      codec: "h264",
      outputLocation: output,
      inputProps,
      concurrency: caps.concurrency,
      chromiumOptions: { gl: glBackend(caps) },
      crf: preview ? 32 : 18,
      onProgress: ({ progress }) => onProgress(progress),
    });
    return {
      file: await fs.readFile(output),
      filename: `${slug(job.name)}${preview ? "-apercu" : ""}.mp4`,
    };
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

/** h264 rejects odd dimensions. */
function even(n: number): number {
  return Math.max(2, Math.round(n / 2) * 2);
}

function slug(name: string): string {
  return (
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "rendu"
  );
}

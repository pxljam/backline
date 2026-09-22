/**
 * Still visual rendering service.
 *
 * The Rust API sends it a JSON description, the brand and the automatic
 * fields; it returns a PNG. It knows nothing of the database or the domain:
 * it is a render engine, nothing more.
 */

import http from "node:http";
import os from "node:os";
import path from "node:path";
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { bundle } from "@remotion/bundler";
import { renderStill, selectComposition } from "@remotion/renderer";

const PORT = Number(process.env.PORT ?? 3000);
const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * The Remotion bundle is built **once**, on the first render, then reused:
 * rebuilding it per request would cost several seconds each time.
 */
let bundlePromise: Promise<string> | null = null;

function serveUrl(): Promise<string> {
  bundlePromise ??= bundle({
    entryPoint: path.join(here, "remotion", "index.ts"),
    onProgress: (p) => {
      if (p === 100) console.log("[stills] Remotion bundle ready");
    },
  });
  return bundlePromise;
}

interface RenderRequest {
  layout: unknown;
  brand?: unknown;
  data?: unknown;
  media?: unknown;
  width?: number;
  height?: number;
}

async function render(body: RenderRequest): Promise<Buffer> {
  const inputProps = {
    layout: body.layout,
    brand: body.brand ?? {},
    data: body.data ?? {},
    media: body.media ?? {},
  };

  const url = await serveUrl();
  const composition = await selectComposition({
    serveUrl: url,
    id: "still",
    inputProps,
  });

  const output = path.join(
    await fs.mkdtemp(path.join(os.tmpdir(), "backline-still-")),
    "out.png",
  );

  try {
    await renderStill({
      composition,
      serveUrl: url,
      output,
      inputProps,
      imageFormat: "png",
      // A render must be reproducible: same description, same file (§18).
      chromiumOptions: { gl: "swangle" },
    });
    return await fs.readFile(output);
  } finally {
    await fs.rm(path.dirname(output), { recursive: true, force: true });
  }
}

function readBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

const server = http.createServer(async (req, res) => {
  if (req.method === "GET" && req.url === "/health") {
    res.writeHead(200, { "content-type": "text/plain" }).end("ok");
    return;
  }

  if (req.method !== "POST" || req.url !== "/render") {
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "unknown route" }));
    return;
  }

  try {
    const body = JSON.parse(await readBody(req)) as RenderRequest;
    if (!body.layout) {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "missing layout description" }));
      return;
    }
    const png = await render(body);
    res.writeHead(200, { "content-type": "image/png", "content-length": png.length });
    res.end(png);
  } catch (err) {
    // A failed visual is reported, never swallowed: the API surfaces it in
    // the comms plan.
    const message = err instanceof Error ? err.message : String(err);
    console.error("[stills] render failed:", message);
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: message }));
  }
});

server.listen(PORT, () => {
  console.log(`[stills] listening on :${PORT}`);
  // Warm the bundle at startup: the first visual should not foot the bill.
  serveUrl().catch((e) => console.error("[stills] initial bundle:", e));
});

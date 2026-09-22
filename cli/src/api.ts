import type { Config } from "./config";

/** Client de l'API de rendu. Une machine ne voit que ce qu'elle a reclame. */
export class Api {
  constructor(private config: Config) {}

  private url(path: string): string {
    return `${this.config.apiUrl.replace(/\/$/, "")}${path}`;
  }

  private headers(): Record<string, string> {
    return { "X-Machine-Token": this.config.token };
  }

  private async json<T>(res: Response, what: string): Promise<T> {
    if (!res.ok) {
      const body = await res.text().catch(() => "");
      throw new Error(`${what} : ${res.status} ${body}`);
    }
    return (await res.json()) as T;
  }

  async claim(capabilities: unknown): Promise<{ job: ClaimedJob | null }> {
    const res = await fetch(this.url("/api/render/claim"), {
      method: "POST",
      headers: { ...this.headers(), "content-type": "application/json" },
      body: JSON.stringify({ capabilities }),
    });
    return this.json(res, "reclamation d'un job");
  }

  async bundle(jobId: string): Promise<JobBundle> {
    const res = await fetch(this.url(`/api/render/jobs/${jobId}/bundle`), {
      headers: this.headers(),
    });
    return this.json(res, "recuperation de la recette");
  }

  async progress(jobId: string, progress: number): Promise<void> {
    await fetch(this.url(`/api/render/jobs/${jobId}/progress`), {
      method: "PUT",
      headers: { ...this.headers(), "content-type": "application/json" },
      body: JSON.stringify({ progress }),
    }).catch(() => {
      // Une progression perdue n'est pas une raison d'interrompre un rendu.
    });
  }

  async complete(jobId: string, file: Buffer, filename: string): Promise<void> {
    const form = new FormData();
    form.append("file", new Blob([new Uint8Array(file)], { type: "video/mp4" }), filename);
    const res = await fetch(this.url(`/api/render/jobs/${jobId}/complete`), {
      method: "POST",
      headers: this.headers(),
      body: form,
    });
    await this.json(res, "envoi du fichier rendu");
  }

  async fail(jobId: string, error: string): Promise<void> {
    await fetch(this.url(`/api/render/jobs/${jobId}/fail`), {
      method: "POST",
      headers: { ...this.headers(), "content-type": "application/json" },
      body: JSON.stringify({ error }),
    }).catch(() => undefined);
  }
}

export interface ClaimedJob {
  id: string;
  composition_id: string | null;
  kind: "video" | "preview";
}

export interface JobBundle {
  id: string;
  kind: "video" | "preview";
  name: string;
  fps: number;
  spec: unknown;
  brand: unknown;
  data: unknown;
  media: Record<string, { url: string; mime?: string }>;
}

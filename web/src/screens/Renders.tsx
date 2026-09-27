import React, { useState } from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase } from "../lib/session";
import type { RenderJob, RenderMachine } from "../lib/types";
import { dateAndTime, relativeTime } from "../lib/format";
import { Icon } from "../components/icons";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle,
} from "../components/ui";

/**
 * The "Rendus" screen (§14). The application shows **at all times** which
 * machines are connected: without them, no video comes out.
 */
export const Renders: React.FC = () => {
  const base = useCollectiveBase();
  const jobs = useResource<{ jobs: RenderJob[]; machines: RenderMachine[] }>(
    `${base}/render-jobs`,
  );
  const { run, busy, error } = useAction();
  const [name, setName] = useState("");
  const [token, setToken] = useState<string | null>(null);

  if (jobs.loading) return <Loading />;
  if (jobs.error) return <ErrorNote>{jobs.error}</ErrorNote>;

  const online = jobs.data?.machines.filter((m) => m.online) ?? [];

  return (
    <>
      <PageTitle
        title="Rendus"
        subtitle="Le VPS n'encode jamais : les videos sont fabriquées sur les machines des membres."
        action={<Button onClick={() => void jobs.reload()}>Rafraîchir</Button>}
      />

      <ErrorNote>{error}</ErrorNote>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="Machines">
          {online.length === 0 && (
            <p className="mb-3 flex items-start gap-2 rounded-control border border-warn/30 bg-warn-quiet px-3 py-2 text-sm text-warn">
              <Icon name="alert" className="mt-0.5" />
              <span className="min-w-0">
                Aucune machine connectée — aucune vidéo ne peut sortir. Les tâches de com restent
                livrables avec leur visuel fixe.
              </span>
            </p>
          )}
          {!jobs.data || jobs.data.machines.length === 0 ? (
            <Empty
              icon="render"
              hint="Une machine, c'est un ordinateur qui fait tourner la CLI. Crée-lui un jeton ci-dessous, puis lance « backline login » dessus."
            >
              Aucune machine enregistrée.
            </Empty>
          ) : (
            <ul className="divide-y divide-line">
              {jobs.data.machines.map((m) => (
                <li key={m.id} className="flex items-center justify-between gap-3 py-2">
                  <div>
                    <p className="text-sm font-medium">{m.name}</p>
                    <p className="text-xs text-ink-soft">
                      {m.capabilities.gpu
                        ? `GPU ${m.capabilities.gpuKind ?? ""}`
                        : "processeur seul"}
                      {m.capabilities.concurrency && ` · ${m.capabilities.concurrency} tâche(s)`}
                      {m.last_seen_at && ` · vue ${relativeTime(m.last_seen_at)}`}
                    </p>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge tone={m.online ? "good" : "neutral"}>
                      {m.online ? "en ligne" : "hors ligne"}
                    </Badge>
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        void run(async () => {
                          await api.del(`/api/render/machines/${m.id}`);
                          await jobs.reload();
                        })
                      }
                    >
                      revoquer
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}

          <form
            className="mt-4 flex flex-wrap items-end gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                const res = await api.post<{ token: string }>("/api/render/machines", {
                  name,
                });
                setToken(res.token);
                setName("");
                await jobs.reload();
              });
            }}
          >
            <div className="min-w-44 flex-1">
              <Field label="Associer une machine" hint="Le jeton n'est montre qu'une fois.">
                <Input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="MacBook de Romain"
                  required
                />
              </Field>
            </div>
            <Button type="submit" variant="primary" disabled={busy}>
              Créer un jeton
            </Button>
          </form>

          {token && (
            <div className="mt-3 rounded-lg border border-line bg-raised px-3 py-3">
              <p className="text-xs uppercase tracking-wide text-ink-soft">Jeton de machine</p>
              <code className="mt-1 block break-all text-sm">{token}</code>
              <p className="mt-2 text-xs text-ink-soft">
                Sur la machine :{" "}
                <code>backline login --url {window.location.origin} --token …</code>
              </p>
            </div>
          )}
        </Card>

        <Card title="File des rendus">
          {!jobs.data || jobs.data.jobs.length === 0 ? (
            <Empty>Aucun rendu pour l'instant.</Empty>
          ) : (
            <ul className="divide-y divide-line">
              {jobs.data.jobs.map((j) => (
                <li key={j.id} className="py-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium">
                      {j.composition_name ?? "composition supprimée"}
                    </span>
                    <Badge
                      tone={
                        j.status === "done"
                          ? "good"
                          : j.status === "failed"
                            ? "bad"
                            : j.status === "queued"
                              ? "warn"
                              : "neutral"
                      }
                    >
                      {j.status}
                    </Badge>
                    <span className="ml-auto text-xs text-ink-soft">
                      {dateAndTime(j.created_at)}
                    </span>
                  </div>
                  {j.status === "running" && (
                    <div className="mt-2 h-1.5 w-full overflow-hidden rounded bg-line">
                      <div
                        className="h-full bg-accent transition-all"
                        style={{ width: `${Math.round(j.progress * 100)}%` }}
                      />
                    </div>
                  )}
                  <p className="mt-1 text-xs text-ink-soft">
                    {j.kind === "preview" ? "aperçu basse définition" : "rendu complet"}
                    {j.claimed_by && ` · ${j.claimed_by}`}
                  </p>
                  {j.error && <p className="mt-1 text-xs text-accent">{j.error}</p>}
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>
    </>
  );
};

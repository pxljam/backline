import React, { useState } from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useSession } from "../lib/session";
import { fileSize } from "../lib/format";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle,
} from "../components/ui";

/**
 * Instance administration (§14). A single shared instance hosts every
 * collective; creating one is **manual**, so it lives here.
 */

interface CollectiveRow {
  id: string;
  slug: string;
  name: string;
  members: number;
  groups: number;
  events: number;
}

interface TechnicalHealth {
  jobs: { pending: number; failed: number };
  render: { machines_online: number; queued: number };
  storage: { assets: number; bytes: number };
  telegram: boolean;
  pdf: boolean;
}

export const Instance: React.FC = () => {
  const { me } = useSession();
  const collectives = useResource<CollectiveRow[]>(
    me?.is_instance_admin ? "/api/instance/collectives" : null,
  );
  const health = useResource<TechnicalHealth>(me?.is_instance_admin ? "/api/instance/health" : null);
  const { run, busy, error } = useAction();
  const [slug, setSlug] = useState("");
  const [name, setName] = useState("");

  if (!me?.is_instance_admin) {
    return (
      <>
        <PageTitle title="Instance" />
        <Card>
          <Empty>Reserve aux administrateurs d'instance.</Empty>
        </Card>
      </>
    );
  }

  if (collectives.loading) return <Loading />;

  return (
    <>
      <PageTitle
        title="Instance"
        subtitle="Collectifs heberges et sante technique de la machine."
      />

      <ErrorNote>{collectives.error ?? health.error ?? error}</ErrorNote>

      <div className="mb-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Indicator
          label="Jobs en attente"
          value={health.data?.jobs.pending ?? 0}
          alert={(health.data?.jobs.failed ?? 0) > 0}
          detail={`${health.data?.jobs.failed ?? 0} en echec`}
        />
        <Indicator
          label="Machines de rendu"
          value={health.data?.render.machines_online ?? 0}
          alert={
            (health.data?.render.queued ?? 0) > 0 && (health.data?.render.machines_online ?? 0) === 0
          }
          detail={`${health.data?.render.queued ?? 0} rendu(s) en file`}
        />
        <Indicator
          label="Medias"
          value={health.data?.storage.assets ?? 0}
          detail={fileSize(health.data?.storage.bytes ?? 0)}
        />
        <Card title="Services">
          <ul className="space-y-1 text-sm">
            <li>
              Telegram{" "}
              <Badge tone={health.data?.telegram ? "good" : "warn"}>
                {health.data?.telegram ? "actif" : "journalise"}
              </Badge>
            </li>
            <li>
              PDF (Typst){" "}
              <Badge tone={health.data?.pdf ? "good" : "bad"}>
                {health.data?.pdf ? "disponible" : "absent"}
              </Badge>
            </li>
          </ul>
        </Card>
      </div>

      <div className="mb-5">
        <Card title="Nouveau collectif">
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                await api.post("/api/instance/collectives", { slug, name });
                setSlug("");
                setName("");
                await collectives.reload();
              });
            }}
          >
            <div className="min-w-44 flex-1">
              <Field label="Nom">
                <Input value={name} onChange={(e) => setName(e.target.value)} required />
              </Field>
            </div>
            <div className="min-w-44 flex-1">
              <Field label="Identifiant" hint="En minuscules, sans espace.">
                <Input
                  value={slug}
                  onChange={(e) => setSlug(e.target.value)}
                  pattern="[a-z0-9-]+"
                  required
                />
              </Field>
            </div>
            <Button type="submit" variant="primary" disabled={busy}>
              Creer
            </Button>
          </form>
          <p className="mt-2 text-xs text-ink-soft">
            Le collectif nait avec ses types d'evenements, son catalogue de formats et une charte
            d'exemple. Lui donner un first admin se fait ensuite depuis ses membres.
          </p>
        </Card>
      </div>

      <Card title="Collectifs">
        {!collectives.data || collectives.data.length === 0 ? (
          <Empty>Aucun collectif.</Empty>
        ) : (
          <ul className="divide-y divide-line">
            {collectives.data.map((c) => (
              <li key={c.id} className="flex flex-wrap items-center justify-between gap-3 py-3">
                <div>
                  <p className="font-medium">{c.name}</p>
                  <p className="text-xs text-ink-soft">{c.slug}</p>
                </div>
                <div className="flex gap-2 text-xs text-ink-soft">
                  <Badge>{c.members} membre(s)</Badge>
                  <Badge>{c.groups} groupe(s)</Badge>
                  <Badge>{c.events} evenement(s)</Badge>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  );
};

const Indicator: React.FC<{
  label: string;
  value: number;
  detail?: string;
  alert?: boolean;
}> = ({ label, value, detail, alert }) => (
  <Card title={label}>
    <p className={`text-2xl font-semibold ${alert ? "text-accent" : ""}`}>{value}</p>
    {detail && <p className="mt-1 text-xs text-ink-soft">{detail}</p>}
  </Card>
);

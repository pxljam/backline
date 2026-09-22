import React, { useRef, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Asset, BacklineEvent, Format, Group, Template, VideoCompositionRow } from "../lib/types";
import { poids } from "../lib/format";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select,
} from "../components/ui";

type Onglet = "gabarits" | "videos" | "medias";

export const Studio: React.FC = () => {
  const [onglet, setOnglet] = useState<Onglet>("gabarits");

  return (
    <>
      <PageTitle
        title="Studio"
        subtitle="Gabarits, videos et bibliotheque de medias. Aucune competence technique requise."
      />
      <nav className="mb-5 flex flex-wrap gap-1.5">
        {(
          [
            ["gabarits", "Gabarits"],
            ["videos", "Videos"],
            ["medias", "Medias"],
          ] as [Onglet, string][]
        ).map(([key, label]) => (
          <button
            key={key}
            onClick={() => setOnglet(key)}
            className={`rounded-lg px-3 py-1.5 text-sm ${
              onglet === key ? "bg-ink text-paper" : "border border-line"
            }`}
          >
            {label}
          </button>
        ))}
      </nav>

      {onglet === "gabarits" && <Templates />}
      {onglet === "videos" && <Videos />}
      {onglet === "medias" && <Medias />}
    </>
  );
};

const Templates: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const templates = useResource<Template[]>(`${base}/studio/templates`);
  const formats = useResource<Format[]>(`${base}/studio/formats`);
  const { run, busy, error } = useAction();
  const [name, setName] = useState("");
  const [formatId, setFormatId] = useState("");

  if (templates.loading) return <Loading />;

  return (
    <>
      <ErrorNote>{templates.error ?? error}</ErrorNote>

      {isAdmin && (
        <div className="mb-5">
          <Card title="Nouveau gabarit">
            <form
              className="flex flex-wrap items-end gap-3"
              onSubmit={(e) => {
                e.preventDefault();
                void run(async () => {
                  await api.post(`${base}/studio/templates`, {
                    name,
                    master_format_id: formatId || formats.data?.[0]?.id,
                  });
                  setName("");
                  await templates.reload();
                });
              }}
            >
              <div className="min-w-48 flex-1">
                <Field label="Nom">
                  <Input value={name} onChange={(e) => setName(e.target.value)} required />
                </Field>
              </div>
              <div className="min-w-48 flex-1">
                <Field label="Format maitre" hint="Le ratio sera verrouille.">
                  <Select value={formatId} onChange={(e) => setFormatId(e.target.value)}>
                    {formats.data?.map((f) => (
                      <option key={f.id} value={f.id}>
                        {f.platform} — {f.label} ({f.ratio})
                      </option>
                    ))}
                  </Select>
                </Field>
              </div>
              <Button type="submit" variant="primary" disabled={busy}>
                Creer
              </Button>
            </form>
          </Card>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        {!templates.data || templates.data.length === 0 ? (
          <Card>
            <Empty>Aucun gabarit.</Empty>
          </Card>
        ) : (
          templates.data.map((t) => (
            <Card
              key={t.id}
              title={t.name}
              action={<Badge>v{t.version}</Badge>}
            >
              <ul className="mb-3 flex flex-wrap gap-1.5">
                {t.variants.map((v) => (
                  <li key={v.format_id}>
                    <Badge tone={v.is_master ? "good" : "neutral"}>
                      {v.format_key} {v.ratio}
                    </Badge>
                  </li>
                ))}
              </ul>
              {t.milestone_key && (
                <p className="mb-3 text-xs text-ink-soft">jalon : {t.milestone_key}</p>
              )}
              <div className="flex gap-2">
                <Link to={`/studio/gabarits/${t.id}`}>
                  <Button variant="primary" size="sm">
                    Ouvrir l'editeur
                  </Button>
                </Link>
                {isAdmin && (
                  <Button
                    size="sm"
                    disabled={busy}
                    onClick={() =>
                      void run(async () => {
                        await api.post(`${base}/studio/templates/${t.id}/duplicate`);
                        await templates.reload();
                      })
                    }
                  >
                    dupliquer
                  </Button>
                )}
              </div>
            </Card>
          ))
        )}
      </div>
    </>
  );
};

const Videos: React.FC = () => {
  const base = useCollectiveBase();
  const comps = useResource<VideoCompositionRow[]>(`${base}/video-compositions`);
  const formats = useResource<Format[]>(`${base}/studio/formats`);
  const events = useResource<BacklineEvent[]>(`${base}/events`);
  const { run, busy, error } = useAction();
  const [name, setName] = useState("");
  const [formatId, setFormatId] = useState("");
  const [eventId, setEventId] = useState("");

  if (comps.loading) return <Loading />;

  const formatsVideo = formats.data?.filter((f) => f.kind === "video") ?? [];

  return (
    <>
      <ErrorNote>{comps.error ?? error}</ErrorNote>

      <div className="mb-5">
        <Card title="Nouvelle composition">
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                await api.post(`${base}/video-compositions`, {
                  name,
                  format_id: formatId || formatsVideo[0]?.id,
                  event_id: eventId || undefined,
                });
                setName("");
                await comps.reload();
              });
            }}
          >
            <div className="min-w-44 flex-1">
              <Field label="Nom">
                <Input value={name} onChange={(e) => setName(e.target.value)} required />
              </Field>
            </div>
            <div className="min-w-44 flex-1">
              <Field label="Format">
                <Select value={formatId} onChange={(e) => setFormatId(e.target.value)}>
                  {formatsVideo.map((f) => (
                    <option key={f.id} value={f.id}>
                      {f.platform} — {f.label} ({f.ratio})
                    </option>
                  ))}
                </Select>
              </Field>
            </div>
            <div className="min-w-44 flex-1">
              <Field label="Evenement" hint="Pour remplir les champs automatiques.">
                <Select value={eventId} onChange={(e) => setEventId(e.target.value)}>
                  <option value="">aucun</option>
                  {events.data?.map((ev) => (
                    <option key={ev.id} value={ev.id}>
                      {ev.title}
                    </option>
                  ))}
                </Select>
              </Field>
            </div>
            <Button type="submit" variant="primary" disabled={busy}>
              Creer
            </Button>
          </form>
        </Card>
      </div>

      <Card>
        {!comps.data || comps.data.length === 0 ? (
          <Empty>Aucune composition video.</Empty>
        ) : (
          <ul className="divide-y divide-line">
            {comps.data.map((c) => (
              <li key={c.id} className="flex flex-wrap items-center justify-between gap-3 py-3">
                <div>
                  <p className="font-medium">{c.name}</p>
                  <p className="text-xs text-ink-soft">
                    {c.spec.scenes?.length ?? 0} plan(s) · {c.fps} img/s · v{c.version}
                  </p>
                </div>
                <Link to={`/studio/videos/${c.id}`}>
                  <Button size="sm" variant="primary">
                    Ouvrir
                  </Button>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  );
};

/** Televersement uniquement — aucune generation par IA (§9.5). */
const Medias: React.FC = () => {
  const base = useCollectiveBase();
  const assets = useResource<Asset[]>(`${base}/studio/assets`);
  const { data: groups } = useResource<Group[]>(`${base}/groups`);
  const { run, busy, error } = useAction();
  const fileRef = useRef<HTMLInputElement>(null);
  const [groupId, setGroupId] = useState("");
  const [tags, setTags] = useState("");

  return (
    <>
      <ErrorNote>{assets.error ?? error}</ErrorNote>

      <div className="mb-5">
        <Card title="Televerser">
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              const file = fileRef.current?.files?.[0];
              if (!file) return;
              const form = new FormData();
              form.append("file", file);
              if (groupId) form.append("group_id", groupId);
              if (tags) form.append("tags", tags);
              void run(async () => {
                await api.upload(`${base}/studio/assets`, form);
                if (fileRef.current) fileRef.current.value = "";
                setTags("");
                await assets.reload();
              });
            }}
          >
            <div className="min-w-48 flex-1">
              <Field label="Fichier" hint="Photos, captations, visuels produits ailleurs.">
                <input ref={fileRef} type="file" className="text-sm" required />
              </Field>
            </div>
            <div className="min-w-40">
              <Field label="Groupe">
                <Select value={groupId} onChange={(e) => setGroupId(e.target.value)}>
                  <option value="">collectif</option>
                  {groups?.map((g) => (
                    <option key={g.id} value={g.id}>
                      {g.name}
                    </option>
                  ))}
                </Select>
              </Field>
            </div>
            <div className="min-w-40">
              <Field label="Etiquettes" hint="Separees par des virgules.">
                <Input value={tags} onChange={(e) => setTags(e.target.value)} />
              </Field>
            </div>
            <Button type="submit" variant="primary" disabled={busy}>
              Televerser
            </Button>
          </form>
        </Card>
      </div>

      <Card>
        {!assets.data || assets.data.length === 0 ? (
          <Empty>Aucun media.</Empty>
        ) : (
          <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            {assets.data.map((a) => (
              <li key={a.id} className="rounded-lg border border-line p-2">
                {a.kind === "image" ? (
                  <AssetPreview assetId={a.id} />
                ) : (
                  <div className="flex h-28 items-center justify-center rounded bg-paper text-xs uppercase text-ink-soft">
                    {a.kind}
                  </div>
                )}
                <p className="mt-2 truncate text-xs font-medium" title={a.filename}>
                  {a.filename}
                </p>
                <p className="text-[11px] text-ink-soft">
                  {poids(a.bytes)}
                  {a.tags.length > 0 && ` · ${a.tags.join(", ")}`}
                </p>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  );
};

const AssetPreview: React.FC<{ assetId: string }> = ({ assetId }) => {
  const base = useCollectiveBase();
  const { data } = useResource<{ url: string }>(`${base}/studio/assets/${assetId}/url`);
  if (!data) return <div className="h-28 rounded bg-paper" />;
  return <img src={data.url} alt="" className="h-28 w-full rounded object-cover" />;
};

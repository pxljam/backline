import React, { useState } from "react";
import { useParams } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type {
  BacklineEvent, Member, PublicationTask, RunSheet, SocialAccount,
} from "../lib/types";
import {
  dateEtHeure, dateLongue, heure, PRESENCE_RESIDENCE, relatif, STATUT_EVENEMENT, STATUT_TACHE,
} from "../lib/format";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select, Textarea,
} from "../components/ui";

type Onglet = "apercu" | "logistique" | "com" | "feuille";

export const EventDetail: React.FC = () => {
  const { id = "" } = useParams();
  const base = useCollectiveBase();
  const { isAdmin, me } = useSession();
  const [onglet, setOnglet] = useState<Onglet>("apercu");

  const ev = useResource<BacklineEvent>(`${base}/events/${id}`);
  const { run, busy, error } = useAction();

  if (ev.loading) return <Loading />;
  if (ev.error) return <ErrorNote>{ev.error}</ErrorNote>;
  const e = ev.data;
  if (!e) return null;

  const onglets: { key: Onglet; label: string }[] = [
    { key: "apercu", label: "Apercu" },
    { key: "logistique", label: "Logistique" },
    { key: "com", label: "Plan de com" },
    { key: "feuille", label: "Feuille de route" },
  ];

  return (
    <>
      <PageTitle
        title={e.title}
        subtitle={
          <>
            {e.type_label} · {dateLongue(e.starts_at)} a {heure(e.starts_at)}
            {e.ends_at && e.type_key === "residency" && ` → ${dateLongue(e.ends_at)}`}
            {e.venue && ` · ${e.venue.name}${e.venue.city ? `, ${e.venue.city}` : ""}`}
            {" · "}
            <Badge tone={e.status === "confirmed" ? "good" : "neutral"}>
              {STATUT_EVENEMENT[e.status]}
            </Badge>
          </>
        }
      />

      <ErrorNote>{error}</ErrorNote>

      <nav className="mb-5 flex flex-wrap gap-1.5">
        {onglets.map((o) => (
          <button
            key={o.key}
            onClick={() => setOnglet(o.key)}
            className={`rounded-lg px-3 py-1.5 text-sm ${
              onglet === o.key ? "bg-ink text-paper" : "border border-line"
            }`}
          >
            {o.label}
          </button>
        ))}
      </nav>

      {onglet === "apercu" && (
        <div className="grid gap-4 lg:grid-cols-2">
          <Card
            title="Line-up"
            action={
              isAdmin && (
                <SetTimesToggle event={e} onDone={() => void ev.reload()} />
              )
            }
          >
            {e.participations.length === 0 ? (
              <Empty>Aucun line-up.</Empty>
            ) : (
              <ul className="divide-y divide-line">
                {e.participations.map((p) => (
                  <li key={p.id} className="flex items-center justify-between gap-3 py-2">
                    <span className="text-sm">
                      <span className="font-medium">{p.label}</span>
                      {p.stage_role && <span className="text-ink-soft"> — {p.stage_role}</span>}
                    </span>
                    <span className="text-xs text-ink-soft">
                      {p.slot_start
                        ? `${p.slot_start.slice(0, 5)}${p.slot_end ? `–${p.slot_end.slice(0, 5)}` : ""}`
                        : "creneau non defini"}
                      {!e.set_times_public && p.slot_start && " · non publiable"}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </Card>

          <Card title="Fiches techniques">
            {e.tech_riders.length === 0 ? (
              <Empty>Aucun groupe au line-up.</Empty>
            ) : (
              <ul className="space-y-2">
                {e.tech_riders.map((r) => (
                  <li key={r.group_id} className="flex items-center justify-between gap-3">
                    <span className="text-sm">{r.group_name}</span>
                    {r.tech_rider_id ? (
                      <Badge tone="good">v{r.version}{r.sent_at ? " · envoyee" : ""}</Badge>
                    ) : (
                      <Badge tone="warn">manquante</Badge>
                    )}
                  </li>
                ))}
              </ul>
            )}
            {isAdmin && e.tech_riders.length > 0 && (
              <Button
                className="mt-3"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    const res = await api.post<{ missing: { name: string }[] }>(
                      `${base}/events/${id}/tech-riders/send`,
                      {},
                    );
                    await ev.reload();
                    if (res.missing.length > 0) {
                      alert(
                        `Envoye. Sans fiche technique : ${res.missing.map((m) => m.name).join(", ")}`,
                      );
                    }
                  })
                }
              >
                Envoyer les fiches au lieu
              </Button>
            )}
            <p className="mt-2 text-xs text-ink-soft">
              Une fiche manquante est signalee, jamais bloquante.
            </p>
          </Card>

          {e.venue && (
            <Card title="Lieu">
              <p className="text-sm font-medium">{e.venue.name}</p>
              {e.venue.address && <p className="text-sm text-ink-soft">{e.venue.address}</p>}
              {e.venue.city && <p className="text-sm text-ink-soft">{e.venue.city}</p>}
            </Card>
          )}

          {e.stream && <StreamCard event={e} onDone={() => void ev.reload()} />}

          {e.residency && (
            <Card title="Presences">
              <ResidencyPresence event={e} onDone={() => void ev.reload()} />
            </Card>
          )}

          {isAdmin && (
            <Card title="Horaires">
              <EventTimes event={e} onDone={() => void ev.reload()} />
            </Card>
          )}
        </div>
      )}

      {onglet === "logistique" && (
        <Logistics event={e} onDone={() => void ev.reload()} meId={me?.id ?? ""} />
      )}

      {onglet === "com" && <Comms eventId={id} eventTypeKey={e.type_key} />}

      {onglet === "feuille" && <RunSheetView eventId={id} />}
    </>
  );
};

/** Les creneaux sont facultatifs, et leur publication est un reglage distinct (§6). */
const SetTimesToggle: React.FC<{ event: BacklineEvent; onDone: () => void }> = ({
  event,
  onDone,
}) => {
  const base = useCollectiveBase();
  const { run, busy } = useAction();

  return (
    <div className="flex items-center gap-2 text-xs">
      <Select
        value={event.set_times_state}
        disabled={busy}
        className="w-auto py-1 text-xs"
        onChange={(ev2) =>
          void run(async () => {
            await api.patch(`${base}/events/${event.id}`, { set_times_state: ev2.target.value });
            onDone();
          })
        }
      >
        <option value="undefined">creneaux non definis</option>
        <option value="to_confirm">a confirmer</option>
        <option value="defined">definis</option>
      </Select>
      <label className="flex items-center gap-1">
        <input
          type="checkbox"
          checked={event.set_times_public}
          disabled={busy}
          onChange={(ev2) =>
            void run(async () => {
              await api.patch(`${base}/events/${event.id}`, {
                set_times_public: ev2.target.checked,
              });
              onDone();
            })
          }
        />
        publiables
      </label>
    </div>
  );
};

const EventTimes: React.FC<{ event: BacklineEvent; onDone: () => void }> = ({ event, onDone }) => {
  const base = useCollectiveBase();
  const { run, busy, error } = useAction();
  const local = (iso: string | null) => (iso ? new Date(iso).toISOString().slice(0, 16) : "");
  const [soundcheck, setSoundcheck] = useState(local(event.soundcheck_at));
  const [doors, setDoors] = useState(local(event.doors_at));

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        void run(async () => {
          await api.patch(`${base}/events/${event.id}`, {
            soundcheck_at: soundcheck ? new Date(soundcheck).toISOString() : undefined,
            doors_at: doors ? new Date(doors).toISOString() : undefined,
          });
          onDone();
        });
      }}
    >
      <Field label="Balance">
        <Input type="datetime-local" value={soundcheck} onChange={(e) => setSoundcheck(e.target.value)} />
      </Field>
      <Field label="Ouverture">
        <Input type="datetime-local" value={doors} onChange={(e) => setDoors(e.target.value)} />
      </Field>
      <ErrorNote>{error}</ErrorNote>
      <Button type="submit" disabled={busy}>
        Enregistrer
      </Button>
    </form>
  );
};

const StreamCard: React.FC<{ event: BacklineEvent; onDone: () => void }> = ({ event, onDone }) => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { run, busy } = useAction();
  const [replay, setReplay] = useState(event.stream?.replay_url ?? "");

  return (
    <Card title="Stream">
      <ul className="space-y-1 text-sm">
        {event.stream?.platforms.map((p, i) => (
          <li key={i}>
            <span className="font-medium capitalize">{p.platform}</span>
            {p.url && (
              <a href={p.url} target="_blank" rel="noreferrer" className="ml-2 underline text-ink-soft">
                {p.url}
              </a>
            )}
          </li>
        ))}
      </ul>
      {event.stream?.capture_location && (
        <p className="mt-2 text-sm text-ink-soft">Captation : {event.stream.capture_location}</p>
      )}
      <p className="mt-2 text-xs text-ink-soft">
        {event.stream?.live_alert_sent_at
          ? `Alerte « on est en ligne » envoyee ${relatif(event.stream.live_alert_sent_at)}.`
          : "L'alerte « on est en ligne » partira 15 min avant, a tous les membres."}
      </p>

      {isAdmin && (
        <form
          className="mt-3 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            void run(async () => {
              await api.patch(`${base}/events/${event.id}/stream`, { replay_url: replay });
              onDone();
            });
          }}
        >
          <Input
            placeholder="Lien du replay"
            value={replay}
            onChange={(e) => setReplay(e.target.value)}
          />
          <Button type="submit" disabled={busy}>
            Enregistrer
          </Button>
        </form>
      )}
    </Card>
  );
};

/** « Tu viens ? », pas « quand exactement ? » (§6). */
const ResidencyPresence: React.FC<{ event: BacklineEvent; onDone: () => void }> = ({
  event,
  onDone,
}) => {
  const base = useCollectiveBase();
  const { me } = useSession();
  const { run, busy } = useAction();
  const moi = event.residency?.presences.find((p) => p.user_id === me?.id);

  return (
    <>
      <div className="mb-4 flex flex-wrap gap-2">
        {(["coming", "unsure", "not_coming"] as const).map((r) => (
          <Button
            key={r}
            size="sm"
            variant={moi?.answer === r ? "primary" : "ghost"}
            disabled={busy}
            onClick={() =>
              void run(async () => {
                await api.post(`${base}/events/${event.id}/presence`, { answer: r });
                onDone();
              })
            }
          >
            {PRESENCE_RESIDENCE[r]}
          </Button>
        ))}
      </div>
      <p className="mb-3 text-xs text-ink-soft">
        Les jours precis sont facultatifs : l'app ne les reclame jamais.
      </p>
      <ul className="divide-y divide-line">
        {event.residency?.presences.map((p) => (
          <li key={p.user_id} className="flex items-center justify-between gap-3 py-2 text-sm">
            <span>{p.name}</span>
            <span className="text-ink-soft">
              {PRESENCE_RESIDENCE[p.answer] ?? p.answer}
              {p.answer === "coming" &&
                (p.days.length > 0 ? ` · ${p.days.length} jour(s) coche(s)` : " · dates libres")}
            </span>
          </li>
        ))}
      </ul>
    </>
  );
};

const Logistics: React.FC<{ event: BacklineEvent; onDone: () => void; meId: string }> = ({
  event,
  onDone,
  meId,
}) => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { run, busy, error } = useAction();
  const { data: suggestions } = useResource<string[]>(`${base}/events/${event.id}/labels`);
  const [label, setLabel] = useState("");
  const [quantity, setQuantity] = useState(1);

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card title="Postes">
        {event.logistics.length === 0 ? (
          <Empty>Aucun poste.</Empty>
        ) : (
          <ul className="divide-y divide-line">
            {event.logistics.map((s) => {
              const jySuis = s.assignees.some((a) => a.user_id === meId);
              return (
                <li key={s.id} className="flex flex-wrap items-center justify-between gap-3 py-3">
                  <div className="min-w-0">
                    <p className="text-sm font-medium">{s.label}</p>
                    <p className="text-xs text-ink-soft">
                      {s.assignees.length}/{s.quantity}
                      {s.assignees.length > 0 &&
                        ` — ${s.assignees.map((a) => a.name).join(", ")}`}
                    </p>
                  </div>
                  <div className="flex items-center gap-2">
                    {s.vacant > 0 ? <Badge tone="warn">{s.vacant} libre(s)</Badge> : <Badge tone="good">complet</Badge>}
                    <Button
                      size="sm"
                      variant={jySuis ? "danger" : "primary"}
                      disabled={busy || (!jySuis && s.vacant === 0)}
                      onClick={() =>
                        void run(async () => {
                          const url = `${base}/events/${event.id}/logistics/${s.id}/take`;
                          if (jySuis) await api.del(url);
                          else await api.post(url);
                          onDone();
                        })
                      }
                    >
                      {jySuis ? "je me retire" : "je le prends"}
                    </Button>
                    {isAdmin && (
                      <Button
                        size="sm"
                        disabled={busy}
                        onClick={() =>
                          void run(async () => {
                            await api.del(`${base}/events/${event.id}/logistics/${s.id}`);
                            onDone();
                          })
                        }
                      >
                        ✕
                      </Button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        <p className="mt-3 text-xs text-ink-soft">
          Relances automatiques a J-14, J-7 et J-2 sur les postes vacants.
        </p>
      </Card>

      {isAdmin && (
        <Card title="Ajouter un poste">
          <form
            className="space-y-3"
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                await api.post(`${base}/events/${event.id}/logistics`, { label, quantity });
                setLabel("");
                setQuantity(1);
                onDone();
              });
            }}
          >
            <Field label="Libelle" hint="Texte libre : « transport backline », « photo »…">
              <Input
                list="labels-logistique"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                required
              />
              <datalist id="labels-logistique">
                {suggestions?.map((s) => (
                  <option key={s} value={s} />
                ))}
              </datalist>
            </Field>
            <Field label="Quantite">
              <Input
                type="number"
                min={1}
                value={quantity}
                onChange={(e) => setQuantity(Number(e.target.value))}
              />
            </Field>
            <ErrorNote>{error}</ErrorNote>
            <Button type="submit" variant="primary" disabled={busy}>
              Ajouter
            </Button>
          </form>
        </Card>
      )}
    </div>
  );
};

/** Mode assiste : le visuel, la legende, un responsable, une confirmation (§11.2). */
const Comms: React.FC<{ eventId: string; eventTypeKey: string }> = ({ eventId }) => {
  const base = useCollectiveBase();
  const { isAdmin, me } = useSession();
  const tasks = useResource<PublicationTask[]>(`${base}/events/${eventId}/comms`);
  const accounts = useResource<SocialAccount[]>(`${base}/social-accounts`);
  const members = useResource<Member[]>(`${base}/members`);
  const { run, busy, error } = useAction();

  if (tasks.loading) return <Loading />;

  const nomMembre = (uid: string | null) =>
    members.data?.find((m) => m.user_id === uid)?.display_name ?? "—";

  return (
    <>
      {isAdmin && (
        <div className="mb-4 flex flex-wrap items-center gap-3">
          <Button
            variant="primary"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                await api.post(`${base}/events/${eventId}/comms/generate`);
                // Le rendu part dans la file : on laisse le temps au premier
                // passage avant de recharger.
                setTimeout(() => void tasks.reload(), 2500);
              })
            }
          >
            Generer les visuels
          </Button>
          <span className="text-xs text-ink-soft">
            Tous les formats du plan, en une action, champs remplis.
          </span>
        </div>
      )}

      <ErrorNote>{error}</ErrorNote>

      <Card>
        {!tasks.data || tasks.data.length === 0 ? (
          <Empty>Aucun plan de com.</Empty>
        ) : (
          <ul className="divide-y divide-line">
            {tasks.data.map((t) => {
              const aMoi = t.assignee_id === me?.id || t.backup_assignee_id === me?.id;
              const compte = accounts.data?.find((a) => a.id === t.social_account?.id);
              const eligibles = members.data?.filter((m) => compte?.access.includes(m.user_id)) ?? [];

              return (
                <li key={t.id} className="py-4">
                  <div className="flex flex-wrap items-center gap-2">
                    <Badge>{t.milestone_key}</Badge>
                    <span className="font-medium">{t.label}</span>
                    <Badge
                      tone={
                        t.status === "published" ? "good" : t.status === "missed" ? "bad" : "neutral"
                      }
                    >
                      {STATUT_TACHE[t.status]}
                    </Badge>
                    <span className="ml-auto text-xs text-ink-soft">
                      {dateEtHeure(t.scheduled_at)} · {relatif(t.scheduled_at)}
                    </span>
                  </div>

                  <p className="mt-1 text-xs text-ink-soft">
                    {t.social_account
                      ? `${t.social_account.platform} ${t.social_account.handle}`
                      : "aucun compte vise"}
                    {" · responsable : "}
                    {nomMembre(t.assignee_id)}
                    {t.reminder_count > 0 && ` · ${t.reminder_count} relance(s)`}
                  </p>

                  {t.visuals.length > 0 && (
                    <ul className="mt-2 flex flex-wrap gap-2">
                      {t.visuals.map((v) => (
                        <li key={v.format_id}>
                          {v.status === "ready" && v.asset_id ? (
                            <AssetThumb assetId={v.asset_id} label={v.format_key} />
                          ) : (
                            <span
                              className="inline-block rounded-lg border border-line px-2 py-1 text-xs text-ink-soft"
                              title={v.error ?? undefined}
                            >
                              {v.format_key} — {v.status === "failed" ? "echec" : v.status}
                            </span>
                          )}
                        </li>
                      ))}
                    </ul>
                  )}

                  {(t.caption || t.hashtags) && (
                    <pre className="mt-2 whitespace-pre-wrap rounded-lg bg-paper px-3 py-2 text-xs">
                      {t.caption}
                      {t.hashtags ? `\n\n${t.hashtags}` : ""}
                    </pre>
                  )}

                  <div className="mt-3 flex flex-wrap items-center gap-2">
                    {isAdmin && t.social_account && (
                      <Select
                        className="w-auto py-1 text-xs"
                        value={t.assignee_id ?? ""}
                        disabled={busy}
                        onChange={(e) =>
                          void run(async () => {
                            await api.post(`${base}/publication-tasks/${t.id}/assign`, {
                              assignee_id: e.target.value || undefined,
                            });
                            await tasks.reload();
                          })
                        }
                      >
                        <option value="">— responsable —</option>
                        {eligibles.map((m) => (
                          <option key={m.user_id} value={m.user_id}>
                            {m.display_name}
                          </option>
                        ))}
                      </Select>
                    )}

                    {t.status !== "published" && (aMoi || isAdmin) && (
                      <Button
                        size="sm"
                        variant="primary"
                        disabled={busy}
                        onClick={() => {
                          const url = prompt("Lien du post (optionnel)") ?? undefined;
                          void run(async () => {
                            await api.post(`${base}/publication-tasks/${t.id}/published`, {
                              published_url: url || undefined,
                            });
                            await tasks.reload();
                          });
                        }}
                      >
                        ✅ publie
                      </Button>
                    )}

                    {t.published_url && (
                      <a
                        href={t.published_url}
                        target="_blank"
                        rel="noreferrer"
                        className="text-xs underline text-ink-soft"
                      >
                        voir le post
                      </a>
                    )}
                  </div>

                  {isAdmin && <CaptionEditor task={t} onDone={() => void tasks.reload()} />}
                </li>
              );
            })}
          </ul>
        )}
      </Card>
      <p className="mt-3 text-xs text-ink-soft">
        La publication automatique par API est impossible sans comptes professionnels : l'outil
        fonctionne en mode assiste, avec confirmation humaine.
      </p>
    </>
  );
};

const CaptionEditor: React.FC<{ task: PublicationTask; onDone: () => void }> = ({
  task,
  onDone,
}) => {
  const base = useCollectiveBase();
  const { run, busy } = useAction();
  const [open, setOpen] = useState(false);
  const [caption, setCaption] = useState(task.caption);
  const [hashtags, setHashtags] = useState(task.hashtags);

  if (!open)
    return (
      <button className="mt-2 text-xs underline text-ink-soft" onClick={() => setOpen(true)}>
        modifier le texte
      </button>
    );

  return (
    <form
      className="mt-3 space-y-2"
      onSubmit={(e) => {
        e.preventDefault();
        void run(async () => {
          await api.patch(`${base}/publication-tasks/${task.id}`, { caption, hashtags });
          setOpen(false);
          onDone();
        });
      }}
    >
      <Textarea rows={3} value={caption} onChange={(e) => setCaption(e.target.value)} placeholder="Legende" />
      <Input value={hashtags} onChange={(e) => setHashtags(e.target.value)} placeholder="#hashtags" />
      <div className="flex gap-2">
        <Button type="submit" size="sm" variant="primary" disabled={busy}>
          Enregistrer
        </Button>
        <Button type="button" size="sm" onClick={() => setOpen(false)}>
          Annuler
        </Button>
      </div>
    </form>
  );
};

const AssetThumb: React.FC<{ assetId: string; label: string }> = ({ assetId, label }) => {
  const base = useCollectiveBase();
  const { data } = useResource<{ url: string }>(`${base}/studio/assets/${assetId}/url`);
  if (!data) return <span className="text-xs text-ink-soft">{label}…</span>;
  return (
    <a href={data.url} target="_blank" rel="noreferrer" className="block">
      <img
        src={data.url}
        alt={label}
        className="h-24 w-auto rounded-lg border border-line object-cover"
      />
      <span className="mt-1 block text-center text-[10px] text-ink-soft">{label}</span>
    </a>
  );
};

const RunSheetView: React.FC<{ eventId: string }> = ({ eventId }) => {
  const base = useCollectiveBase();
  const { data, loading, error } = useResource<RunSheet>(`${base}/events/${eventId}/run-sheet`);

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;
  if (!data) return null;

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card title="Jour J">
        <p className="text-sm">{dateLongue(data.starts_at)}</p>
        <ul className="mt-2 space-y-1 text-sm text-ink-soft">
          {data.soundcheck_at && <li>Balance : {heure(data.soundcheck_at)}</li>}
          {data.doors_at && <li>Ouverture : {heure(data.doors_at)}</li>}
          <li>Debut : {heure(data.starts_at)}</li>
        </ul>
        {data.venue && (
          <div className="mt-4">
            <p className="text-sm font-medium">{data.venue.name}</p>
            {data.venue.address && <p className="text-sm text-ink-soft">{data.venue.address}</p>}
            {data.venue.city && <p className="text-sm text-ink-soft">{data.venue.city}</p>}
            <ul className="mt-2 space-y-1 text-sm">
              {data.venue.contacts.map((c, i) => (
                <li key={i}>
                  {c.name}
                  {c.role && <span className="text-ink-soft"> ({c.role})</span>}
                  {c.phone ? ` — ${c.phone}` : ""}
                </li>
              ))}
            </ul>
          </div>
        )}
      </Card>

      <Card title="Line-up">
        <ul className="divide-y divide-line">
          {data.line_up.map((l, i) => (
            <li key={i} className="py-2">
              <p className="text-sm font-medium">
                {l.label}
                {l.slot && <span className="ml-2 text-xs text-ink-soft">{l.slot}</span>}
              </p>
              <p className="text-xs text-ink-soft">
                {l.members.map((m) => `${m.name}${m.phone ? ` ${m.phone}` : ""}`).join(" · ")}
              </p>
            </li>
          ))}
        </ul>
      </Card>

      <Card title="Qui tient quel poste">
        <ul className="divide-y divide-line">
          {data.logistics.map((l, i) => (
            <li key={i} className="flex items-center justify-between gap-3 py-2 text-sm">
              <span>{l.label}</span>
              <span className={l.vacant > 0 ? "text-accent" : "text-ink-soft"}>
                {l.assignees.length === 0
                  ? "VACANT"
                  : l.assignees.map((a) => `${a.name}${a.phone ? ` ${a.phone}` : ""}`).join(", ")}
              </span>
            </li>
          ))}
        </ul>
      </Card>

      <Card title="Fiches techniques">
        <ul className="space-y-1 text-sm">
          {data.tech_riders.map((r) => (
            <li key={r.group_id} className="flex items-center justify-between">
              <span>{r.group_name}</span>
              {r.tech_rider_id ? (
                <a
                  href={`${base}/groups/${r.group_id}/tech-riders/${r.tech_rider_id}/pdf`}
                  className="text-xs underline"
                >
                  PDF v{r.version}
                </a>
              ) : (
                <Badge tone="warn">manquante</Badge>
              )}
            </li>
          ))}
        </ul>
      </Card>
    </div>
  );
};

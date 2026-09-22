import React, { useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { BacklineEvent, CollectiveDetail, Group, Venue } from "../lib/types";
import { dateAndTime, relativeTime, EVENT_STATUS_LABELS } from "../lib/format";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select,
} from "../components/ui";

export const Events: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { data, loading, error, reload } = useResource<BacklineEvent[]>(`${base}/events`);
  const [creating, setCreating] = useState(false);

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;

  const upcoming = (data ?? []).filter((e) => e.status === "confirmed" || e.status === "draft");
  const past = (data ?? []).filter((e) => e.status === "past" || e.status === "cancelled");

  return (
    <>
      <PageTitle
        title="Evenements"
        subtitle="Soirees, concerts, residences et streams."
        action={
          isAdmin && (
            <Button variant="primary" onClick={() => setCreating((v) => !v)}>
              {creating ? "Annuler" : "Nouvel evenement"}
            </Button>
          )
        }
      />

      {creating && (
        <div className="mb-5">
          <NewEvent
            onDone={() => {
              setCreating(false);
              void reload();
            }}
          />
        </div>
      )}

      <div className="grid gap-4">
        <Card title="A venir">
          {upcoming.length === 0 ? <Empty>Rien de prevu.</Empty> : <EventList events={upcoming} />}
        </Card>
        {past.length > 0 && (
          <Card title="Passes">
            <EventList events={past} />
          </Card>
        )}
      </div>
    </>
  );
};

const EventList: React.FC<{ events: BacklineEvent[] }> = ({ events }) => (
  <ul className="divide-y divide-line">
    {events.map((e) => {
      const vacant = e.logistics.reduce((n, s) => n + s.vacant, 0);
      const missingRider = e.tech_riders.filter((r) => !r.tech_rider_id).length;
      return (
        <li key={e.id}>
          <Link to={`/evenements/${e.id}`} className="flex flex-wrap items-center gap-3 py-3">
            <div className="min-w-0 flex-1">
              <p className="font-medium">{e.title}</p>
              <p className="text-xs text-ink-soft">
                {e.type_label} · {dateAndTime(e.starts_at)}
                {e.venue && ` · ${e.venue.name}`}
                {e.status === "confirmed" && ` · ${relativeTime(e.starts_at)}`}
              </p>
            </div>
            <div className="flex flex-wrap gap-1.5">
              <Badge tone={e.status === "confirmed" ? "good" : "neutral"}>
                {EVENT_STATUS_LABELS[e.status]}
              </Badge>
              {vacant > 0 && <Badge tone="warn">{vacant} poste(s) vacant(s)</Badge>}
              {e.comms_summary.late > 0 && (
                <Badge tone="bad">{e.comms_summary.late} com en retard</Badge>
              )}
              {missingRider > 0 && <Badge tone="neutral">{missingRider} sans fiche technique</Badge>}
            </div>
          </Link>
        </li>
      );
    })}
  </ul>
);

const NewEvent: React.FC<{ onDone: () => void }> = ({ onDone }) => {
  const base = useCollectiveBase();
  const { data: collective } = useResource<CollectiveDetail>(base);
  const { data: venues } = useResource<Venue[]>(`${base}/venues`);
  const { data: groups } = useResource<Group[]>(`${base}/groups`);
  const { run, busy, error } = useAction();

  const [typeKey, setTypeKey] = useState("stream");
  const [title, setTitle] = useState("");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [venueId, setVenueId] = useState("");
  const [hostGroupId, setHostGroupId] = useState("");
  const [platforms, setPlatforms] = useState<{ platform: string; url: string }[]>([
    { platform: "twitch", url: "" },
  ]);

  const type = collective?.event_types.find((t) => t.key === typeKey);
  const isStream = typeKey === "stream";

  return (
    <Card
      title="Nouvel evenement"
      action={
        <span className="text-xs text-ink-soft">
          Un concert se cree normalement depuis une opportunite.
        </span>
      }
    >
      <form
        className="grid gap-4 md:grid-cols-2"
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            await api.post(`${base}/events`, {
              event_type_key: typeKey,
              title,
              starts_at: new Date(startsAt).toISOString(),
              ends_at: endsAt ? new Date(endsAt).toISOString() : undefined,
              venue_id: venueId || undefined,
              host_group_id: hostGroupId || undefined,
              platforms: isStream ? platforms.filter((p) => p.platform) : [],
            });
            onDone();
          });
        }}
      >
        <Field label="Type">
          <Select value={typeKey} onChange={(e) => setTypeKey(e.target.value)}>
            {collective?.event_types.map((t) => (
              <option key={t.key} value={t.key}>
                {t.label}
              </option>
            ))}
          </Select>
        </Field>

        <Field label="Titre">
          <Input value={title} onChange={(e) => setTitle(e.target.value)} required />
        </Field>

        <Field label="Debut">
          <Input
            type="datetime-local"
            value={startsAt}
            onChange={(e) => setStartsAt(e.target.value)}
            required
          />
        </Field>

        <Field
          label={type?.is_range ? "Fin (obligatoire)" : "Fin"}
          hint={type?.is_range ? "Une residence porte une plage fixe." : undefined}
        >
          <Input
            type="datetime-local"
            value={endsAt}
            onChange={(e) => setEndsAt(e.target.value)}
            required={type?.is_range}
          />
        </Field>

        <Field label="Lieu">
          <Select value={venueId} onChange={(e) => setVenueId(e.target.value)}>
            <option value="">— sans lieu —</option>
            {venues?.map((v) => (
              <option key={v.id} value={v.id}>
                {v.name}
              </option>
            ))}
          </Select>
        </Field>

        <Field label="Porteur" hint="Vide = le collectif porte l'evenement.">
          <Select value={hostGroupId} onChange={(e) => setHostGroupId(e.target.value)}>
            <option value="">le collectif</option>
            {groups?.map((g) => (
              <option key={g.id} value={g.id}>
                {g.name}
              </option>
            ))}
          </Select>
        </Field>

        {isStream && (
          <div className="md:col-span-2">
            <Field label="Plateformes de diffusion" hint="Multi-diffusion possible.">
              <div className="space-y-2">
                {platforms.map((p, i) => (
                  <div key={i} className="flex gap-2">
                    <Select
                      value={p.platform}
                      onChange={(e) =>
                        setPlatforms((ps) =>
                          ps.map((x, j) => (i === j ? { ...x, platform: e.target.value } : x)),
                        )
                      }
                      className="max-w-40"
                    >
                      <option value="twitch">Twitch</option>
                      <option value="youtube">YouTube Live</option>
                      <option value="instagram">Instagram Live</option>
                      <option value="tiktok">TikTok Live</option>
                    </Select>
                    <Input
                      placeholder="https://…"
                      value={p.url}
                      onChange={(e) =>
                        setPlatforms((ps) =>
                          ps.map((x, j) => (i === j ? { ...x, url: e.target.value } : x)),
                        )
                      }
                    />
                    <Button
                      type="button"
                      size="sm"
                      onClick={() => setPlatforms((ps) => ps.filter((_, j) => j !== i))}
                    >
                      ✕
                    </Button>
                  </div>
                ))}
                <Button
                  type="button"
                  size="sm"
                  onClick={() => setPlatforms((ps) => [...ps, { platform: "youtube", url: "" }])}
                >
                  + une plateforme
                </Button>
              </div>
            </Field>
          </div>
        )}

        <div className="md:col-span-2">
          <ErrorNote>{error}</ErrorNote>
          <Button type="submit" variant="primary" disabled={busy} className="mt-2">
            {busy ? "…" : "Creer"}
          </Button>
        </div>
      </form>
    </Card>
  );
};

import React, { useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Group, Opportunity, Venue } from "../lib/types";
import { dateCourte, STATUT_OPPORTUNITE } from "../lib/format";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select, Textarea,
} from "../components/ui";

const TON: Record<string, "neutral" | "good" | "warn" | "bad"> = {
  discussing: "neutral",
  poll_open: "warn",
  date_chosen: "warn",
  confirmed: "good",
  abandoned: "neutral",
};

export const Opportunities: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { data, loading, error, reload } = useResource<Opportunity[]>(`${base}/opportunities`);
  const [creating, setCreating] = useState(false);

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;

  return (
    <>
      <PageTitle
        title="Opportunites"
        subtitle="Un lieu, des dates possibles, un sondage. C'est ici que la date se decide."
        action={
          isAdmin && (
            <Button variant="primary" onClick={() => setCreating((v) => !v)}>
              {creating ? "Annuler" : "Nouvelle opportunite"}
            </Button>
          )
        }
      />

      {creating && (
        <div className="mb-5">
          <NewOpportunity
            onDone={() => {
              setCreating(false);
              void reload();
            }}
          />
        </div>
      )}

      <Card>
        {!data || data.length === 0 ? (
          <Empty>Aucune opportunite pour l'instant.</Empty>
        ) : (
          <ul className="divide-y divide-line">
            {data.map((o) => (
              <li key={o.id}>
                <Link
                  to={`/opportunites/${o.id}`}
                  className="flex flex-wrap items-center justify-between gap-3 py-3"
                >
                  <div className="min-w-0">
                    <p className="font-medium">{o.title}</p>
                    <p className="text-xs text-ink-soft">
                      {o.venue ? `${o.venue.name}${o.venue.city ? ` — ${o.venue.city}` : ""}` : "sans lieu"}
                      {" · "}
                      {o.candidate_dates.length} date{o.candidate_dates.length > 1 ? "s" : ""} candidate
                      {o.candidate_dates.length > 1 ? "s" : ""}
                      {o.candidate_dates.length > 0 &&
                        ` (${o.candidate_dates.map((d) => dateCourte(d.day)).join(", ")})`}
                    </p>
                  </div>
                  <Badge tone={TON[o.status]}>{STATUT_OPPORTUNITE[o.status]}</Badge>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  );
};

const NewOpportunity: React.FC<{ onDone: () => void }> = ({ onDone }) => {
  const base = useCollectiveBase();
  const { data: venues } = useResource<Venue[]>(`${base}/venues`);
  const { data: groups } = useResource<Group[]>(`${base}/groups`);
  const { run, busy, error } = useAction();

  const [title, setTitle] = useState("");
  const [venueId, setVenueId] = useState("");
  const [conditions, setConditions] = useState("");
  const [hosts, setHosts] = useState<string[]>([]);
  // N dates candidates proposees par le lieu (§5.1).
  const [dates, setDates] = useState<{ day: string; start_time: string }[]>([
    { day: "", start_time: "22:00" },
  ]);

  return (
    <Card title="Nouvelle opportunite">
      <form
        className="grid gap-4 md:grid-cols-2"
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            await api.post(`${base}/opportunities`, {
              title,
              venue_id: venueId || undefined,
              conditions: conditions || undefined,
              host_group_ids: hosts,
              candidate_dates: dates
                .filter((d) => d.day)
                .map((d) => ({
                  day: d.day,
                  start_time: d.start_time ? `${d.start_time}:00` : undefined,
                })),
            });
            onDone();
          });
        }}
      >
        <Field label="Intitule">
          <Input value={title} onChange={(e) => setTitle(e.target.value)} required />
        </Field>

        <Field label="Lieu">
          <Select value={venueId} onChange={(e) => setVenueId(e.target.value)}>
            <option value="">— sans lieu —</option>
            {venues?.map((v) => (
              <option key={v.id} value={v.id}>
                {v.name}
                {v.city ? ` (${v.city})` : ""}
              </option>
            ))}
          </Select>
        </Field>

        <div className="md:col-span-2">
          <Field label="Porteurs pressentis" hint="Aucun coche = le collectif porte la date.">
            <div className="flex flex-wrap gap-2">
              {groups?.map((g) => (
                <label
                  key={g.id}
                  className={`cursor-pointer rounded-lg border px-3 py-1.5 text-sm ${
                    hosts.includes(g.id) ? "border-ink bg-ink text-paper" : "border-line"
                  }`}
                >
                  <input
                    type="checkbox"
                    className="sr-only"
                    checked={hosts.includes(g.id)}
                    onChange={() =>
                      setHosts((h) =>
                        h.includes(g.id) ? h.filter((x) => x !== g.id) : [...h, g.id],
                      )
                    }
                  />
                  {g.name}
                </label>
              ))}
            </div>
          </Field>
        </div>

        <div className="md:col-span-2">
          <Field label="Dates candidates" hint="Celles que le lieu propose.">
            <div className="space-y-2">
              {dates.map((d, i) => (
                <div key={i} className="flex gap-2">
                  <Input
                    type="date"
                    value={d.day}
                    onChange={(e) =>
                      setDates((ds) => ds.map((x, j) => (i === j ? { ...x, day: e.target.value } : x)))
                    }
                  />
                  <Input
                    type="time"
                    value={d.start_time}
                    onChange={(e) =>
                      setDates((ds) =>
                        ds.map((x, j) => (i === j ? { ...x, start_time: e.target.value } : x)),
                      )
                    }
                  />
                  <Button
                    type="button"
                    size="sm"
                    onClick={() => setDates((ds) => ds.filter((_, j) => j !== i))}
                  >
                    ✕
                  </Button>
                </div>
              ))}
              <Button
                type="button"
                size="sm"
                onClick={() => setDates((ds) => [...ds, { day: "", start_time: "22:00" }])}
              >
                + une date
              </Button>
            </div>
          </Field>
        </div>

        <div className="md:col-span-2">
          <Field label="Conditions" hint="Texte libre. L'argent est hors perimetre.">
            <Textarea rows={2} value={conditions} onChange={(e) => setConditions(e.target.value)} />
          </Field>
        </div>

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

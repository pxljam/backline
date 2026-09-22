import React, { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useResource } from "../lib/hooks";
import { useCollectiveBase } from "../lib/session";
import type { CalendarEntry, Group } from "../lib/types";
import { shortDate, time } from "../lib/format";
import { Badge, Button, Card, Empty, ErrorNote, Loading, PageTitle, Select } from "../components/ui";

type View = "mois" | "semaine" | "liste";

/** Month / week / list view, filterable (§7). The application is the master. */
export const Calendar: React.FC = () => {
  const base = useCollectiveBase();
  const [view, setView] = useState<View>("mois");
  const [groupId, setGroupId] = useState("");
  const [mine, setMine] = useState(false);
  const [anchor, setAnchor] = useState(() => new Date());

  const { data: groups } = useResource<Group[]>(`${base}/groups`);
  const params = new URLSearchParams();
  if (groupId) params.set("group_id", groupId);
  if (mine) params.set("mine", "true");
  const { data, loading, error } = useResource<CalendarEntry[]>(
    `${base}/calendar?${params.toString()}`,
    [groupId, mine],
  );
  const feeds = useResource<{ scope: string; label: string; url: string }[]>(
    `${base}/calendar/feeds`,
  );

  const entries = data ?? [];

  const shift = (n: number) => {
    const d = new Date(anchor);
    if (view === "semaine") d.setDate(d.getDate() + n * 7);
    else d.setMonth(d.getMonth() + n);
    setAnchor(d);
  };

  return (
    <>
      <PageTitle
        title="Calendrier"
        subtitle="Evenements confirmes, dates candidates en arbitrage (en pointilles)."
      />

      <div className="mb-4 flex flex-wrap items-center gap-2">
        {(["mois", "semaine", "liste"] as View[]).map((v) => (
          <button
            key={v}
            onClick={() => setView(v)}
            className={`rounded-lg px-3 py-1.5 text-sm capitalize ${
              view === v ? "bg-ink text-paper" : "border border-line"
            }`}
          >
            {v}
          </button>
        ))}
        <Select
          className="w-auto"
          value={groupId}
          onChange={(e) => setGroupId(e.target.value)}
        >
          <option value="">tous les groupes</option>
          {groups?.map((g) => (
            <option key={g.id} value={g.id}>
              {g.name}
            </option>
          ))}
        </Select>
        <label className="flex items-center gap-1.5 text-sm">
          <input type="checkbox" checked={mine} onChange={(e) => setMine(e.target.checked)} />
          mes evenements
        </label>
        {view !== "liste" && (
          <div className="ml-auto flex items-center gap-2">
            <Button size="sm" onClick={() => shift(-1)}>
              ‹
            </Button>
            <span className="text-sm">{anchorTitle(anchor, view)}</span>
            <Button size="sm" onClick={() => shift(1)}>
              ›
            </Button>
          </div>
        )}
      </div>

      {loading && <Loading />}
      <ErrorNote>{error}</ErrorNote>

      {!loading && (
        <Card>
          {view === "liste" ? (
            <ListView entries={entries} />
          ) : (
            <GridView entries={entries} anchor={anchor} week={view === "semaine"} />
          )}
        </Card>
      )}

      <div className="mt-5">
        <Card title="Flux iCal">
          <p className="mb-3 text-sm text-ink-soft">
            A coller dans Google Calendar ou Apple Calendrier. L'URL contient un jeton secret :
            elle vaut mot de passe.
          </p>
          <ul className="space-y-2">
            {feeds.data?.map((f) => (
              <li key={f.url} className="flex flex-wrap items-center gap-2">
                <Badge>{f.scope === "user" ? "moi" : f.scope === "group" ? "groupe" : "collectif"}</Badge>
                <span className="text-sm">{f.label}</span>
                <code className="min-w-0 flex-1 truncate rounded bg-paper px-2 py-1 text-xs">
                  {new URL(f.url, window.location.origin).href}
                </code>
                <Button
                  size="sm"
                  onClick={() =>
                    void navigator.clipboard.writeText(new URL(f.url, window.location.origin).href)
                  }
                >
                  copier
                </Button>
              </li>
            ))}
          </ul>
        </Card>
      </div>
    </>
  );
};

function anchorTitle(d: Date, view: View): string {
  const months = [
    "janvier", "fevrier", "mars", "avril", "mai", "juin",
    "juillet", "aout", "septembre", "octobre", "novembre", "decembre",
  ];
  if (view === "semaine") {
    const start = startOfWeek(d);
    const end = new Date(start);
    end.setDate(end.getDate() + 6);
    return `${start.getDate()}–${end.getDate()} ${months[end.getMonth()]}`;
  }
  return `${months[d.getMonth()]} ${d.getFullYear()}`;
}

function startOfWeek(d: Date): Date {
  const copy = new Date(d);
  // French week: Monday first.
  const weekday = (copy.getDay() + 6) % 7;
  copy.setDate(copy.getDate() - weekday);
  copy.setHours(0, 0, 0, 0);
  return copy;
}

const ListView: React.FC<{ entries: CalendarEntry[] }> = ({ entries }) =>
  entries.length === 0 ? (
    <Empty>Rien a afficher.</Empty>
  ) : (
    <ul className="divide-y divide-line">
      {entries.map((e) => (
        <li key={`${e.kind}-${e.id}`}>
          <Link
            to={e.kind === "event" ? `/evenements/${e.id}` : `/opportunites/${e.opportunity_id}`}
            className="flex flex-wrap items-center justify-between gap-3 py-3"
          >
            <div>
              <p className="text-sm font-medium">{e.title}</p>
              <p className="text-xs text-ink-soft">
                {shortDate(e.starts_at)} a {time(e.starts_at)}
                {e.venue && ` · ${e.venue}`}
              </p>
            </div>
            {e.kind === "candidate_date" && <Badge tone="warn">date candidate</Badge>}
          </Link>
        </li>
      ))}
    </ul>
  );

const GridView: React.FC<{ entries: CalendarEntry[]; anchor: Date; week: boolean }> = ({
  entries,
  anchor,
  week,
}) => {
  const days = useMemo(() => {
    if (week) {
      const start = startOfWeek(anchor);
      return Array.from({ length: 7 }, (_, i) => {
        const d = new Date(start);
        d.setDate(d.getDate() + i);
        return d;
      });
    }
    const firstOfMonth = new Date(anchor.getFullYear(), anchor.getMonth(), 1);
    const start = startOfWeek(firstOfMonth);
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start);
      d.setDate(d.getDate() + i);
      return d;
    });
  }, [anchor, week]);

  const byDay = useMemo(() => {
    const map = new Map<string, CalendarEntry[]>();
    for (const e of entries) {
      const key = new Date(e.starts_at).toDateString();
      map.set(key, [...(map.get(key) ?? []), e]);
    }
    return map;
  }, [entries]);

  return (
    <div className="overflow-x-auto">
      <div className="grid min-w-[42rem] grid-cols-7 gap-px bg-line">
        {["lun", "mar", "mer", "jeu", "ven", "sam", "dim"].map((j) => (
          <div key={j} className="bg-panel px-2 py-1 text-center text-xs uppercase text-ink-soft">
            {j}
          </div>
        ))}
        {days.map((d) => {
          const dayEntries = byDay.get(d.toDateString()) ?? [];
          const outsideMonth = !week && d.getMonth() !== anchor.getMonth();
          const isToday = d.toDateString() === new Date().toDateString();
          return (
            <div
              key={d.toISOString()}
              className={`min-h-24 bg-panel p-1.5 ${outsideMonth ? "opacity-40" : ""}`}
            >
              <span
                className={`text-xs ${isToday ? "rounded bg-ink px-1.5 text-paper" : "text-ink-soft"}`}
              >
                {d.getDate()}
              </span>
              <ul className="mt-1 space-y-1">
                {dayEntries.map((e) => (
                  <li key={`${e.kind}-${e.id}`}>
                    <Link
                      to={
                        e.kind === "event" ? `/evenements/${e.id}` : `/opportunites/${e.opportunity_id}`
                      }
                      className={`block truncate rounded px-1 py-0.5 text-[11px] ${
                        e.kind === "candidate_date"
                          ? "border border-dashed border-ink-soft text-ink-soft"
                          : "bg-ink text-paper"
                      }`}
                      title={e.title}
                    >
                      {e.title}
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          );
        })}
      </div>
    </div>
  );
};

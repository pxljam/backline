import React, { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useResource } from "../lib/hooks";
import { useCollectiveBase } from "../lib/session";
import type { CalendarEntry, Group } from "../lib/types";
import { dateCourte, heure } from "../lib/format";
import { Badge, Button, Card, Empty, ErrorNote, Loading, PageTitle, Select } from "../components/ui";

type Vue = "mois" | "semaine" | "liste";

/** Vue mois / semaine / liste, filtrable (§7). L'application est maitre. */
export const Calendar: React.FC = () => {
  const base = useCollectiveBase();
  const [vue, setVue] = useState<Vue>("mois");
  const [groupId, setGroupId] = useState("");
  const [mine, setMine] = useState(false);
  const [ancre, setAncre] = useState(() => new Date());

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

  const entrees = data ?? [];

  const decaler = (n: number) => {
    const d = new Date(ancre);
    if (vue === "semaine") d.setDate(d.getDate() + n * 7);
    else d.setMonth(d.getMonth() + n);
    setAncre(d);
  };

  return (
    <>
      <PageTitle
        title="Calendrier"
        subtitle="Evenements confirmes, dates candidates en arbitrage (en pointilles)."
      />

      <div className="mb-4 flex flex-wrap items-center gap-2">
        {(["mois", "semaine", "liste"] as Vue[]).map((v) => (
          <button
            key={v}
            onClick={() => setVue(v)}
            className={`rounded-lg px-3 py-1.5 text-sm capitalize ${
              vue === v ? "bg-ink text-paper" : "border border-line"
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
        {vue !== "liste" && (
          <div className="ml-auto flex items-center gap-2">
            <Button size="sm" onClick={() => decaler(-1)}>
              ‹
            </Button>
            <span className="text-sm">{titreAncre(ancre, vue)}</span>
            <Button size="sm" onClick={() => decaler(1)}>
              ›
            </Button>
          </div>
        )}
      </div>

      {loading && <Loading />}
      <ErrorNote>{error}</ErrorNote>

      {!loading && (
        <Card>
          {vue === "liste" ? (
            <ListeVue entrees={entrees} />
          ) : (
            <GrilleVue entrees={entrees} ancre={ancre} semaine={vue === "semaine"} />
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

function titreAncre(d: Date, vue: Vue): string {
  const mois = [
    "janvier", "fevrier", "mars", "avril", "mai", "juin",
    "juillet", "aout", "septembre", "octobre", "novembre", "decembre",
  ];
  if (vue === "semaine") {
    const debut = debutSemaine(d);
    const fin = new Date(debut);
    fin.setDate(fin.getDate() + 6);
    return `${debut.getDate()}–${fin.getDate()} ${mois[fin.getMonth()]}`;
  }
  return `${mois[d.getMonth()]} ${d.getFullYear()}`;
}

function debutSemaine(d: Date): Date {
  const copie = new Date(d);
  // Semaine francaise : lundi en premier.
  const jour = (copie.getDay() + 6) % 7;
  copie.setDate(copie.getDate() - jour);
  copie.setHours(0, 0, 0, 0);
  return copie;
}

const ListeVue: React.FC<{ entrees: CalendarEntry[] }> = ({ entrees }) =>
  entrees.length === 0 ? (
    <Empty>Rien a afficher.</Empty>
  ) : (
    <ul className="divide-y divide-line">
      {entrees.map((e) => (
        <li key={`${e.kind}-${e.id}`}>
          <Link
            to={e.kind === "event" ? `/evenements/${e.id}` : `/opportunites/${e.opportunity_id}`}
            className="flex flex-wrap items-center justify-between gap-3 py-3"
          >
            <div>
              <p className="text-sm font-medium">{e.title}</p>
              <p className="text-xs text-ink-soft">
                {dateCourte(e.starts_at)} a {heure(e.starts_at)}
                {e.venue && ` · ${e.venue}`}
              </p>
            </div>
            {e.kind === "candidate_date" && <Badge tone="warn">date candidate</Badge>}
          </Link>
        </li>
      ))}
    </ul>
  );

const GrilleVue: React.FC<{ entrees: CalendarEntry[]; ancre: Date; semaine: boolean }> = ({
  entrees,
  ancre,
  semaine,
}) => {
  const jours = useMemo(() => {
    if (semaine) {
      const debut = debutSemaine(ancre);
      return Array.from({ length: 7 }, (_, i) => {
        const d = new Date(debut);
        d.setDate(d.getDate() + i);
        return d;
      });
    }
    const premier = new Date(ancre.getFullYear(), ancre.getMonth(), 1);
    const debut = debutSemaine(premier);
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(debut);
      d.setDate(d.getDate() + i);
      return d;
    });
  }, [ancre, semaine]);

  const parJour = useMemo(() => {
    const map = new Map<string, CalendarEntry[]>();
    for (const e of entrees) {
      const cle = new Date(e.starts_at).toDateString();
      map.set(cle, [...(map.get(cle) ?? []), e]);
    }
    return map;
  }, [entrees]);

  return (
    <div className="overflow-x-auto">
      <div className="grid min-w-[42rem] grid-cols-7 gap-px bg-line">
        {["lun", "mar", "mer", "jeu", "ven", "sam", "dim"].map((j) => (
          <div key={j} className="bg-panel px-2 py-1 text-center text-xs uppercase text-ink-soft">
            {j}
          </div>
        ))}
        {jours.map((d) => {
          const du = parJour.get(d.toDateString()) ?? [];
          const horsMois = !semaine && d.getMonth() !== ancre.getMonth();
          const aujourdhui = d.toDateString() === new Date().toDateString();
          return (
            <div
              key={d.toISOString()}
              className={`min-h-24 bg-panel p-1.5 ${horsMois ? "opacity-40" : ""}`}
            >
              <span
                className={`text-xs ${aujourdhui ? "rounded bg-ink px-1.5 text-paper" : "text-ink-soft"}`}
              >
                {d.getDate()}
              </span>
              <ul className="mt-1 space-y-1">
                {du.map((e) => (
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

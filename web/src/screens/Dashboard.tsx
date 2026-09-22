import React from "react";
import { Link } from "react-router-dom";
import { useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Dashboard as DashboardData } from "../lib/types";
import { dateEtHeure, joursAvant, relatif } from "../lib/format";
import { Badge, Card, Empty, ErrorNote, Loading, PageTitle } from "../components/ui";

/** « Ce qui bloque » (§14) — pas une liste d'informations, une liste d'actions. */
export const Dashboard: React.FC = () => {
  const base = useCollectiveBase();
  const { me } = useSession();
  const { data, loading, error } = useResource<DashboardData>(`${base}/dashboard`);

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;
  if (!data) return null;

  const rienNeBloque =
    data.pending_polls.length === 0 &&
    data.vacant_slots.length === 0 &&
    data.late_tasks.length === 0 &&
    data.missing_tech_riders.length === 0;

  return (
    <>
      <PageTitle
        title={`Bonsoir, ${me?.display_name ?? ""}`}
        subtitle={rienNeBloque ? "Rien ne bloque." : "Ce qui demande une decision."}
      />

      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="Sondages en attente">
          {data.pending_polls.length === 0 ? (
            <Empty>Aucun sondage ouvert.</Empty>
          ) : (
            <ul className="space-y-2">
              {data.pending_polls.map((p) => (
                <li key={p.opportunity_id}>
                  <Link
                    to={`/opportunites/${p.opportunity_id}`}
                    className="flex items-center justify-between gap-3 rounded-lg border border-line px-3 py-2 hover:border-ink"
                  >
                    <span className="text-sm">{p.title}</span>
                    {p.mine_missing ? (
                      <Badge tone="warn">a toi de repondre</Badge>
                    ) : (
                      <Badge tone="good">tu as repondu</Badge>
                    )}
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Postes a pourvoir">
          {data.vacant_slots.length === 0 ? (
            <Empty>Tous les postes sont tenus.</Empty>
          ) : (
            <ul className="space-y-2">
              {data.vacant_slots.slice(0, 8).map((s, i) => (
                <li key={`${s.event_id}-${s.label}-${i}`}>
                  <Link
                    to={`/evenements/${s.event_id}`}
                    className="flex items-center justify-between gap-3 rounded-lg border border-line px-3 py-2 hover:border-ink"
                  >
                    <span className="text-sm">
                      <span className="font-medium">{s.label}</span>
                      <span className="text-ink-soft"> — {s.event_title}</span>
                    </span>
                    <Badge tone={joursAvant(s.starts_at) <= 7 ? "bad" : "warn"}>
                      {s.vacant} place{s.vacant > 1 ? "s" : ""} · {relatif(s.starts_at)}
                    </Badge>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Com en retard">
          {data.late_tasks.length === 0 ? (
            <Empty>La com est a jour.</Empty>
          ) : (
            <ul className="space-y-2">
              {data.late_tasks.slice(0, 8).map((t) => (
                <li key={t.task_id}>
                  <Link
                    to={`/evenements/${t.task_id && ""}`}
                    className="pointer-events-none block rounded-lg border border-line px-3 py-2"
                  >
                    <span className="text-sm font-medium">{t.label}</span>
                    <span className="block text-xs text-ink-soft">
                      {t.event_title} · prevu {relatif(t.scheduled_at)}
                      {!t.assignee_id && " · personne d'assigne"}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="30 prochains jours">
          {data.upcoming.length === 0 ? (
            <Empty>Rien de prevu.</Empty>
          ) : (
            <ul className="space-y-2">
              {data.upcoming.map((e) => (
                <li key={e.id}>
                  <Link
                    to={`/evenements/${e.id}`}
                    className="flex items-center justify-between gap-3 rounded-lg border border-line px-3 py-2 hover:border-ink"
                  >
                    <span className="text-sm">
                      <span className="font-medium">{e.title}</span>
                      {e.venue && <span className="text-ink-soft"> — {e.venue}</span>}
                    </span>
                    <span className="shrink-0 text-xs text-ink-soft">
                      {dateEtHeure(e.starts_at)}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Card>

        {data.missing_tech_riders.length > 0 && (
          <Card title="Fiches techniques manquantes">
            {/* Signale, jamais bloquant (§12). */}
            <ul className="space-y-1 text-sm">
              {data.missing_tech_riders.map((m, i) => (
                <li key={i} className="text-ink-soft">
                  <span className="font-medium text-ink">{m.group_name}</span> — {m.event_title}
                </li>
              ))}
            </ul>
            <p className="mt-3 text-xs text-ink-soft">
              Rien n'est bloque : un concert se joue tres bien sans PDF.
            </p>
          </Card>
        )}

        <Card title="Machines de rendu">
          <p className="text-sm">
            {data.render_machines_online === 0 ? (
              <>
                <Badge tone="bad">aucune machine connectee</Badge>
                <span className="mt-2 block text-ink-soft">
                  Aucune video ne peut sortir. Les taches de com restent livrables avec leur
                  visuel fixe.
                </span>
              </>
            ) : (
              <Badge tone="good">
                {data.render_machines_online} machine
                {data.render_machines_online > 1 ? "s" : ""} connectee
                {data.render_machines_online > 1 ? "s" : ""}
              </Badge>
            )}
          </p>
        </Card>
      </div>
    </>
  );
};

import React from "react";
import { Link, useSearchParams } from "react-router-dom";
import { useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Dashboard as DashboardData } from "../lib/types";
import { dateAndTime, daysUntil, relativeTime } from "../lib/format";
import { Badge, Button, Card, Empty, ErrorNote, Loading, Note, PageTitle } from "../components/ui";
import { FirstSteps } from "../components/FirstSteps";
import { Tour } from "../components/Tour";

/** A list entry that leads somewhere: the border firms up and the slab lifts
 *  a shade, the same answer the shell's navigation gives to a pointer. */
const ROW =
  "flex items-center justify-between gap-3 rounded-control border border-line px-3 py-2 " +
  "transition-colors duration-200 hover:border-line-strong hover:bg-raised " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent";

/** Evening is when a collective's admin opens this, but not the only time. */
const greeting = (hour = new Date().getHours()) =>
  hour >= 5 && hour < 18 ? "Bonjour" : "Bonsoir";

/** "What is blocking" (§14) — not a list of information, a list of actions. */
export const Dashboard: React.FC = () => {
  const base = useCollectiveBase();
  const { me, isAdmin, collectiveId } = useSession();
  // Set by the invitation screen, so a first sign-in lands on a greeting rather
  // than on a wall of empty cards.
  const [params, setParams] = useSearchParams();
  const bienvenue = params.get("bienvenue") === "1";
  const [tour, setTour] = React.useState(false);
  const { data, loading, error } = useResource<DashboardData>(`${base}/dashboard`);

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;
  if (!data) return null;

  const nothingBlocking =
    data.pending_polls.length === 0 &&
    data.vacant_slots.length === 0 &&
    data.late_tasks.length === 0 &&
    data.missing_tech_riders.length === 0;

  return (
    <>
      <PageTitle
        title={`${greeting()}, ${me?.display_name ?? ""}`}
        subtitle={nothingBlocking ? "Rien ne bloque." : "Ce qui demande une décision."}
        action={
          <Button size="sm" icon="spark" onClick={() => setTour(true)}>
            Visite guidée
          </Button>
        }
      />

      <Tour open={tour} onClose={() => setTour(false)} />

      {bienvenue && (
        <Note icon="spark" className="mb-4">
          <p className="font-medium">Bienvenue{me?.display_name ? `, ${me.display_name}` : ""}.</p>
          <p className="text-ink-soft">
            Ce tableau de bord ne liste pas tout : il liste ce qui bloque.{" "}
            <button className="underline" onClick={() => setParams({}, { replace: true })}>
              j'ai compris
            </button>
          </p>
        </Note>
      )}

      {collectiveId && <FirstSteps data={data} isAdmin={isAdmin} collectiveId={collectiveId} />}

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
                    className={ROW}
                  >
                    <span className="text-sm">{p.title}</span>
                    {p.mine_missing ? (
                      <Badge tone="warn">à toi de répondre</Badge>
                    ) : (
                      <Badge tone="good">tu as répondu</Badge>
                    )}
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Postes à pourvoir">
          {data.vacant_slots.length === 0 ? (
            <Empty>Tous les postes sont tenus.</Empty>
          ) : (
            <ul className="space-y-2">
              {data.vacant_slots.slice(0, 8).map((s, i) => (
                <li key={`${s.event_id}-${s.label}-${i}`}>
                  <Link
                    to={`/evenements/${s.event_id}`}
                    className={ROW}
                  >
                    <span className="text-sm">
                      <span className="font-medium">{s.label}</span>
                      <span className="text-ink-soft"> — {s.event_title}</span>
                    </span>
                    <Badge tone={daysUntil(s.starts_at) <= 7 ? "bad" : "warn"}>
                      {s.vacant} place{s.vacant > 1 ? "s" : ""} · {relativeTime(s.starts_at)}
                    </Badge>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Com en retard">
          {data.late_tasks.length === 0 ? (
            <Empty>La com est à jour.</Empty>
          ) : (
            <ul className="space-y-2">
              {data.late_tasks.slice(0, 8).map((t) => (
                <li key={t.task_id}>
                  {/* No event id comes with a late task, so the row informs
                      rather than pretending to be a link. */}
                  <div className="rounded-control border border-line px-3 py-2">
                    <span className="text-sm font-medium">{t.label}</span>
                    <span className="block text-xs text-ink-soft">
                      {t.event_title} · prévu {relativeTime(t.scheduled_at)}
                      {!t.assignee_id && " · personne d'assigné"}
                    </span>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="30 prochains jours">
          {data.upcoming.length === 0 ? (
            <Empty>Rien de prévu.</Empty>
          ) : (
            <ul className="space-y-2">
              {data.upcoming.map((e) => (
                <li key={e.id}>
                  <Link
                    to={`/evenements/${e.id}`}
                    className={ROW}
                  >
                    <span className="text-sm">
                      <span className="font-medium">{e.title}</span>
                      {e.venue && <span className="text-ink-soft"> — {e.venue}</span>}
                    </span>
                    <span className="shrink-0 text-xs text-ink-soft">
                      {dateAndTime(e.starts_at)}
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
              Rien n'est bloqué : un concert se joue très bien sans PDF.
            </p>
          </Card>
        )}

        <Card title="Machines de rendu">
          <p className="text-sm">
            {data.render_machines_online === 0 ? (
              <>
                <Badge tone="bad">aucune machine connectée</Badge>
                <span className="mt-2 block text-ink-soft">
                  Aucune vidéo ne peut sortir. Les tâches de com restent livrables avec leur
                  visuel fixe.
                </span>
              </>
            ) : (
              <Badge tone="good">
                {data.render_machines_online} machine
                {data.render_machines_online > 1 ? "s" : ""} connectée
                {data.render_machines_online > 1 ? "s" : ""}
              </Badge>
            )}
          </p>
        </Card>
      </div>
    </>
  );
};

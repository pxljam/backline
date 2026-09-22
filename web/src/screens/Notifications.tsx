import React from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import type { Notification } from "../lib/types";
import { relatif } from "../lib/format";
import { Badge, Button, Card, Empty, ErrorNote, Loading, PageTitle } from "../components/ui";

const LIBELLE: Record<string, string> = {
  poll_open: "sondage",
  lineup_retained: "line-up",
  lineup_not_retained: "line-up",
  logistics_vacant: "logistique",
  run_sheet: "feuille de route",
  stream_live: "live",
  publication_due: "publication",
  publication_assigned: "publication",
  tech_rider_missing: "fiche technique",
  admin_alert: "alerte",
};

/** Le centre de notifications web est le **doublon complet** du bot (§19). */
export const Notifications: React.FC = () => {
  const { data, loading, error, reload } = useResource<Notification[]>("/api/notifications");
  const { run, busy } = useAction();

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;

  const nonLues = (data ?? []).filter((n) => !n.read_at).length;

  return (
    <>
      <PageTitle
        title="Notifications"
        subtitle={nonLues > 0 ? `${nonLues} non lue${nonLues > 1 ? "s" : ""}` : "Tout est lu."}
        action={
          nonLues > 0 && (
            <Button
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  await api.post("/api/notifications/read");
                  await reload();
                })
              }
            >
              Tout marquer comme lu
            </Button>
          )
        }
      />

      <Card>
        {!data || data.length === 0 ? (
          <Empty>Rien pour l'instant.</Empty>
        ) : (
          <ul className="divide-y divide-line">
            {data.map((n) => (
              <li key={n.id} className={`py-3 ${n.read_at ? "opacity-60" : ""}`}>
                <div className="flex flex-wrap items-center gap-2">
                  <Badge tone={n.kind === "admin_alert" ? "bad" : "neutral"}>
                    {LIBELLE[n.kind] ?? n.kind}
                  </Badge>
                  <span className="text-sm font-medium">{n.title}</span>
                  <span className="ml-auto text-xs text-ink-soft">{relatif(n.created_at)}</span>
                </div>
                {n.body && (
                  <p className="mt-1 whitespace-pre-wrap text-sm text-ink-soft">
                    {n.body.replace(/<[^>]+>/g, "")}
                  </p>
                )}
                {!n.read_at && (
                  <button
                    className="mt-2 text-xs underline text-ink-soft"
                    onClick={() =>
                      void run(async () => {
                        await api.post(`/api/notifications/${n.id}/read`);
                        await reload();
                      })
                    }
                  >
                    marquer comme lue
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  );
};

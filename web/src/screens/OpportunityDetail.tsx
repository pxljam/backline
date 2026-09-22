import React, { useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { CollectiveDetail, Matrix, Opportunity } from "../lib/types";
import { shortDate, OPPORTUNITY_STATUS_LABELS } from "../lib/format";
import {
  Badge, Button, Card, ErrorNote, Field, Loading, PageTitle, Select,
} from "../components/ui";

type Answer = "yes" | "maybe" | "no";

const BUTTONS: { value: Answer; label: string; title: string }[] = [
  { value: "yes", label: "✅", title: "dispo" },
  { value: "maybe", label: "❔", title: "peut-etre" },
  { value: "no", label: "❌", title: "non" },
];

/**
 * The `members x candidate dates` matrix (§5.2) — the screen that removes the
 * back and forth. It shows at a glance which groups are **complete**, and who
 * is missing from the others.
 */
export const OpportunityDetail: React.FC = () => {
  const { id = "" } = useParams();
  const base = useCollectiveBase();
  const { isAdmin, me } = useSession();
  const navigate = useNavigate();

  const opp = useResource<Opportunity>(`${base}/opportunities/${id}`);
  const matrix = useResource<Matrix>(`${base}/opportunities/${id}/matrix`);
  const { run, busy, error } = useAction();

  const o = opp.data;
  const m = matrix.data;

  const myRow = useMemo(
    () => m?.members.find((x) => x.user_id === me?.id) ?? null,
    [m, me],
  );

  if (opp.loading || matrix.loading) return <Loading />;
  if (opp.error) return <ErrorNote>{opp.error}</ErrorNote>;
  if (!o || !m) return null;

  const refresh = async () => {
    await Promise.all([opp.reload(), matrix.reload()]);
  };

  const answer = (dateId: string, patch: { status?: Answer; wants_to_play?: boolean }) => {
    const current = myRow?.answers[dateId];
    void run(async () => {
      await api.put(`${base}/opportunities/${id}/availabilities`, {
        user_id: me?.id,
        answers: [
          {
            candidate_date_id: dateId,
            status: patch.status ?? current?.status ?? "no",
            wants_to_play: patch.wants_to_play ?? current?.wants_to_play ?? false,
          },
        ],
      });
      await refresh();
    });
  };

  const groupName = (gid: string) => m.groups.find((g) => g.id === gid)?.name ?? "?";
  const memberName = (uid: string) =>
    m.members.find((x) => x.user_id === uid)?.display_name ?? "?";

  return (
    <>
      <PageTitle
        title={o.title}
        subtitle={
          <>
            {o.venue ? `${o.venue.name}${o.venue.city ? ` — ${o.venue.city}` : ""}` : "sans lieu"}
            {" · "}
            <Badge>{OPPORTUNITY_STATUS_LABELS[o.status]}</Badge>
          </>
        }
        action={
          isAdmin &&
          o.status !== "confirmed" && (
            <div className="flex gap-2">
              {!o.poll_open ? (
                <Button
                  variant="primary"
                  disabled={busy || o.candidate_dates.length === 0}
                  onClick={() =>
                    void run(async () => {
                      await api.post(`${base}/opportunities/${id}/poll`);
                      await refresh();
                    })
                  }
                >
                  Ouvrir le sondage
                </Button>
              ) : (
                <Button
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      await api.del(`${base}/opportunities/${id}/poll`);
                      await refresh();
                    })
                  }
                >
                  Clore le sondage
                </Button>
              )}
            </div>
          )
        }
      />

      <ErrorNote>{error}</ErrorNote>

      {o.conditions && (
        <p className="mb-5 rounded-lg border border-line bg-panel px-4 py-3 text-sm text-ink-soft">
          {o.conditions}
        </p>
      )}

      {o.event_id && (
        <div className="mb-5">
          <Card title="Date arretee">
            <Button variant="primary" onClick={() => navigate(`/evenements/${o.event_id}`)}>
              Voir l'evenement
            </Button>
          </Card>
        </div>
      )}

      {/* Ma reponse : seule la personne concernee ecrit sa disponibilite. */}
      {o.poll_open && myRow && (
        <div className="mb-5">
          <Card title="Mes disponibilites">
            <div className="space-y-2">
              {m.dates.map((d) => {
                const reply = myRow.answers[d.id];
                return (
                  <div
                    key={d.id}
                    className="flex flex-wrap items-center gap-3 rounded-lg border border-line px-3 py-2"
                  >
                    <span className="min-w-28 text-sm font-medium">{shortDate(d.day)}</span>
                    {d.start_time && (
                      <span className="text-xs text-ink-soft">{d.start_time.slice(0, 5)}</span>
                    )}
                    <div className="flex gap-1">
                      {BUTTONS.map((b) => (
                        <button
                          key={b.value}
                          title={b.title}
                          disabled={busy}
                          onClick={() => answer(d.id, { status: b.value })}
                          className={`rounded-lg border px-2.5 py-1 text-sm ${
                            reply?.status === b.value ? "border-ink bg-ink text-paper" : "border-line"
                          }`}
                        >
                          {b.label}
                        </button>
                      ))}
                    </div>
                    <button
                      disabled={busy}
                      onClick={() => answer(d.id, { wants_to_play: !reply?.wants_to_play })}
                      className={`rounded-lg border px-2.5 py-1 text-sm ${
                        reply?.wants_to_play ? "border-ink bg-ink text-paper" : "border-line"
                      }`}
                    >
                      🎸 je veux jouer
                    </button>
                    {d.notes && <span className="text-xs text-ink-soft">{d.notes}</span>}
                  </div>
                );
              })}
            </div>
          </Card>
        </div>
      )}

      {/* La matrice. Visible par tous les membres du collectif (§5.2). */}
      <Card title="Matrice des disponibilites">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[32rem] border-collapse text-sm">
            <thead>
              <tr>
                <th className="border-b border-line px-2 py-2 text-left font-medium text-ink-soft">
                  Membre
                </th>
                {m.dates.map((d) => (
                  <th key={d.id} className="border-b border-line px-2 py-2 text-center">
                    <div className="font-medium">{shortDate(d.day)}</div>
                    <div className="text-xs font-normal text-ink-soft">
                      {d.yes} dispo{d.yes > 1 ? "s" : ""}
                    </div>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {m.members.map((membre) => (
                <tr key={membre.user_id}>
                  <td className="border-b border-line px-2 py-2">
                    {membre.stage_name ?? membre.display_name}
                    {membre.group_ids.length > 0 && (
                      <span className="ml-1 text-xs text-ink-soft">
                        ({membre.group_ids.map(groupName).join(", ")})
                      </span>
                    )}
                  </td>
                  {m.dates.map((d) => {
                    const a = membre.answers[d.id];
                    const background =
                      a?.status === "yes"
                        ? "bg-emerald-100"
                        : a?.status === "maybe"
                          ? "bg-amber-100"
                          : a?.status === "no"
                            ? "bg-accent-soft"
                            : "";
                    return (
                      <td
                        key={d.id}
                        className={`border-b border-line px-2 py-2 text-center ${background}`}
                        title={a ? a.status : "sans reponse"}
                      >
                        {a?.status === "yes" ? "✅" : a?.status === "maybe" ? "❔" : a?.status === "no" ? "❌" : "·"}
                        {a?.wants_to_play && <span className="ml-0.5">🎸</span>}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Ce que la matrice sert vraiment a dire : quels line-up sont possibles. */}
        <div className="mt-5 grid gap-3 md:grid-cols-3">
          {m.dates.map((d) => (
            <div key={d.id} className="rounded-lg border border-line px-3 py-3">
              <p className="font-medium">{shortDate(d.day)}</p>
              <p className="mt-1 text-xs text-ink-soft">
                {d.yes} dispo · {d.maybe} peut-etre · {d.no} non · {d.no_answer} sans reponse
              </p>
              {d.complete_groups.length > 0 ? (
                <p className="mt-2 text-sm">
                  <Badge tone="good">groupe complet</Badge>{" "}
                  {d.complete_groups.map(groupName).join(", ")}
                </p>
              ) : (
                <p className="mt-2 text-sm text-ink-soft">Aucun groupe complet.</p>
              )}
              {d.partial_groups.length > 0 && (
                <ul className="mt-2 space-y-0.5 text-xs text-ink-soft">
                  {d.partial_groups.map((p) => (
                    <li key={p.group_id}>
                      {groupName(p.group_id)} — manque {p.missing.map(memberName).join(", ")}
                    </li>
                  ))}
                </ul>
              )}
              {d.volunteers.length > 0 && (
                <p className="mt-2 text-xs">
                  🎸 {d.volunteers.map(memberName).join(", ")}
                </p>
              )}
            </div>
          ))}
        </div>
      </Card>

      {isAdmin && o.status !== "confirmed" && m.dates.length > 0 && (
        <div className="mt-5">
          <Arbitration
            opportunityId={id}
            matrix={m}
            onDone={(eventId) => navigate(`/evenements/${eventId}`)}
          />
        </div>
      )}
    </>
  );
};

/** Arbitration: pick the date, build the line-up, confirm (§5.3). */
const Arbitration: React.FC<{
  opportunityId: string;
  matrix: Matrix;
  onDone: (eventId: string) => void;
}> = ({ opportunityId, matrix, onDone }) => {
  const base = useCollectiveBase();
  const { data: collective } = useResource<CollectiveDetail>(base);
  const { run, busy, error } = useAction();

  const [dateId, setDateId] = useState(matrix.dates[0]?.id ?? "");
  const [typeKey, setTypeKey] = useState("dj_night");
  const [lineUp, setLineUp] = useState<string[]>([]);

  const date = matrix.dates.find((d) => d.id === dateId);
  // Complete groups come first: those are the line-ups that can actually play.
  const groups = matrix.groups.filter((g) => g.member_ids.length > 0);

  return (
    <Card title="Arbitrer et confirmer">
      <div className="grid gap-4 md:grid-cols-3">
        <Field label="Date retenue">
          <Select value={dateId} onChange={(e) => setDateId(e.target.value)}>
            {matrix.dates.map((d) => (
              <option key={d.id} value={d.id}>
                {shortDate(d.day)} — {d.yes} dispo
              </option>
            ))}
          </Select>
        </Field>

        <Field label="Type d'evenement">
          <Select value={typeKey} onChange={(e) => setTypeKey(e.target.value)}>
            {collective?.event_types
              .filter((t) => t.key !== "stream")
              .map((t) => (
                <option key={t.key} value={t.key}>
                  {t.label}
                </option>
              ))}
          </Select>
        </Field>

        <Field label="Line-up" hint="Les groupes complets a cette date sont marques.">
          <div className="flex flex-wrap gap-2">
            {groups.map((g) => {
              const full = date?.complete_groups.includes(g.id);
              const chosen = lineUp.includes(g.id);
              return (
                <label
                  key={g.id}
                  className={`cursor-pointer rounded-lg border px-3 py-1.5 text-sm ${
                    chosen ? "border-ink bg-ink text-paper" : full ? "border-emerald-500" : "border-line"
                  }`}
                >
                  <input
                    type="checkbox"
                    className="sr-only"
                    checked={chosen}
                    onChange={() =>
                      setLineUp((l) => (l.includes(g.id) ? l.filter((x) => x !== g.id) : [...l, g.id]))
                    }
                  />
                  {g.name}
                  {full && " ✓"}
                </label>
              );
            })}
          </div>
        </Field>
      </div>

      <ErrorNote>{error}</ErrorNote>

      <p className="mt-3 text-xs text-ink-soft">
        Confirmer cree l'evenement, les postes logistiques, le plan de com, rattache les fiches
        techniques, et previent les retenus comme les non-retenus.
      </p>

      <Button
        variant="primary"
        className="mt-3"
        disabled={busy || !dateId}
        onClick={() =>
          void run(async () => {
            const res = await api.post<{ event_id: string }>(
              `${base}/opportunities/${opportunityId}/convert`,
              {
                candidate_date_id: dateId,
                event_type_key: typeKey,
                line_up: lineUp.map((group_id) => ({ group_id })),
              },
            );
            onDone(res.event_id);
          })
        }
      >
        {busy ? "…" : "Confirmer la date"}
      </Button>
    </Card>
  );
};

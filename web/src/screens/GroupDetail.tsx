import React, { useState } from "react";
import { useParams } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Group, Member, SocialAccount, TechRider } from "../lib/types";
import { dateCourte } from "../lib/format";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select, Textarea,
} from "../components/ui";

export const GroupDetail: React.FC = () => {
  const { id = "" } = useParams();
  const base = useCollectiveBase();
  const { me, isAdmin } = useSession();

  const group = useResource<Group>(`${base}/groups/${id}`);
  const members = useResource<Member[]>(`${base}/members`);
  const riders = useResource<TechRider[]>(`${base}/groups/${id}/tech-riders`);
  const accounts = useResource<SocialAccount[]>(`${base}/social-accounts`);
  const pressKit = useResource<{
    bio_short: string | null;
    bio_long: string | null;
    links: { label: string; url: string }[];
  }>(`${base}/groups/${id}/press-kit`);

  const { run, busy, error } = useAction();
  const [nouveauMembre, setNouveauMembre] = useState("");
  const [roleLibre, setRoleLibre] = useState("");

  if (group.loading) return <Loading />;
  if (group.error) return <ErrorNote>{group.error}</ErrorNote>;
  const g = group.data;
  if (!g) return null;

  const peutEditer =
    isAdmin || g.members.some((m) => m.user_id === me?.id && m.is_admin);
  const comptesDuGroupe = accounts.data?.filter((a) => a.group_id === id) ?? [];

  return (
    <>
      <PageTitle title={g.name} subtitle={g.description ?? undefined} />
      <ErrorNote>{error}</ErrorNote>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="Membres">
          <ul className="divide-y divide-line">
            {g.members.map((m) => (
              <li key={m.user_id} className="flex items-center justify-between gap-3 py-2">
                <span className="text-sm">
                  {m.stage_name ?? m.display_name}
                  {m.role_label && <span className="text-ink-soft"> — {m.role_label}</span>}
                </span>
                <div className="flex items-center gap-2">
                  {m.is_admin && <Badge>referent</Badge>}
                  {peutEditer && (
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        void run(async () => {
                          await api.del(`${base}/groups/${id}/members/${m.user_id}`);
                          await group.reload();
                        })
                      }
                    >
                      ✕
                    </Button>
                  )}
                </div>
              </li>
            ))}
          </ul>

          {peutEditer && (
            <form
              className="mt-4 flex flex-wrap items-end gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                void run(async () => {
                  await api.post(`${base}/groups/${id}/members`, {
                    user_id: nouveauMembre,
                    role_label: roleLibre || undefined,
                  });
                  setNouveauMembre("");
                  setRoleLibre("");
                  await group.reload();
                });
              }}
            >
              <div className="min-w-40 flex-1">
                <Field label="Ajouter">
                  <Select
                    value={nouveauMembre}
                    onChange={(e) => setNouveauMembre(e.target.value)}
                    required
                  >
                    <option value="">—</option>
                    {members.data
                      ?.filter((m) => !g.members.some((x) => x.user_id === m.user_id))
                      .map((m) => (
                        <option key={m.user_id} value={m.user_id}>
                          {m.display_name}
                        </option>
                      ))}
                  </Select>
                </Field>
              </div>
              <div className="min-w-40 flex-1">
                <Field label="Role" hint="Texte libre : « MAO », « batterie »…">
                  <Input value={roleLibre} onChange={(e) => setRoleLibre(e.target.value)} />
                </Field>
              </div>
              <Button type="submit" disabled={busy || !nouveauMembre}>
                Ajouter
              </Button>
            </form>
          )}
        </Card>

        <Card title="Comptes sociaux">
          {comptesDuGroupe.length === 0 ? (
            <Empty>Aucun compte declare.</Empty>
          ) : (
            <ul className="divide-y divide-line">
              {comptesDuGroupe.map((a) => (
                <li key={a.id} className="py-2">
                  <p className="text-sm font-medium capitalize">
                    {a.platform} <span className="font-normal text-ink-soft">{a.handle}</span>
                  </p>
                  <p className="text-xs text-ink-soft">
                    acces :{" "}
                    {a.access
                      .map((u) => members.data?.find((m) => m.user_id === u)?.display_name ?? "?")
                      .join(", ") || "personne"}
                  </p>
                </li>
              ))}
            </ul>
          )}
          <p className="mt-3 text-xs text-ink-soft">
            L'application recense **qui a acces**, jamais les mots de passe.
          </p>
        </Card>

        <Card title="Fiches techniques">
          {!riders.data || riders.data.length === 0 ? (
            <Empty>Aucune fiche. Rien n'est bloque pour autant.</Empty>
          ) : (
            <ul className="divide-y divide-line">
              {riders.data.map((r) => (
                <li key={r.id} className="flex items-center justify-between gap-3 py-2">
                  <span className="text-sm">
                    v{r.version}{" "}
                    <span className="text-ink-soft">· {dateCourte(r.created_at)}</span>
                  </span>
                  <div className="flex items-center gap-2">
                    <Badge tone={r.status === "published" ? "good" : "neutral"}>
                      {r.status === "published" ? "publiee" : "brouillon"}
                    </Badge>
                    <a
                      className="text-xs underline"
                      href={`${base}/groups/${id}/tech-riders/${r.id}/pdf`}
                    >
                      PDF
                    </a>
                    {peutEditer && r.status === "draft" && (
                      <Button
                        size="sm"
                        disabled={busy}
                        onClick={() =>
                          void run(async () => {
                            await api.post(`${base}/groups/${id}/tech-riders/${r.id}/publish`);
                            await riders.reload();
                          })
                        }
                      >
                        publier
                      </Button>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          )}
          {peutEditer && (
            <Button
              className="mt-3"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const derniere = riders.data?.[0]?.version;
                  await api.post(`${base}/groups/${id}/tech-riders`, {
                    from_version: derniere,
                  });
                  await riders.reload();
                })
              }
            >
              Nouvelle version
            </Button>
          )}
        </Card>

        {peutEditer && riders.data?.[0]?.status === "draft" && (
          <RiderEditor
            groupId={id}
            rider={riders.data[0]}
            onDone={() => void riders.reload()}
          />
        )}

        <Card title="Press kit">
          <PressKitForm
            groupId={id}
            initial={pressKit.data}
            editable={peutEditer}
            onDone={() => void pressKit.reload()}
          />
        </Card>
      </div>
    </>
  );
};

/** Saisie structuree de la fiche technique (§12). */
const RiderEditor: React.FC<{ groupId: string; rider: TechRider; onDone: () => void }> = ({
  groupId,
  rider,
  onDone,
}) => {
  const base = useCollectiveBase();
  const { run, busy, error } = useAction();
  const [json, setJson] = useState(() => JSON.stringify(rider.data, null, 2));
  const [erreurJson, setErreurJson] = useState<string | null>(null);

  return (
    <Card title={`Fiche technique — brouillon v${rider.version}`}>
      <p className="mb-2 text-xs text-ink-soft">
        Sections : identite, line-up scene, plan de scene, input list, backline, son, lumiere,
        loges, arrivee, contacts.
      </p>
      <Textarea
        rows={16}
        className="font-mono text-xs"
        value={json}
        onChange={(e) => {
          setJson(e.target.value);
          try {
            JSON.parse(e.target.value);
            setErreurJson(null);
          } catch (err) {
            setErreurJson(err instanceof Error ? err.message : "JSON invalide");
          }
        }}
      />
      <ErrorNote>{erreurJson ?? error}</ErrorNote>
      <Button
        className="mt-2"
        variant="primary"
        disabled={busy || !!erreurJson}
        onClick={() =>
          void run(async () => {
            await api.patch(`${base}/groups/${groupId}/tech-riders/${rider.id}`, {
              data: JSON.parse(json),
            });
            onDone();
          })
        }
      >
        Enregistrer
      </Button>
    </Card>
  );
};

const PressKitForm: React.FC<{
  groupId: string;
  initial: { bio_short: string | null; bio_long: string | null; links: unknown } | null;
  editable: boolean;
  onDone: () => void;
}> = ({ groupId, initial, editable, onDone }) => {
  const base = useCollectiveBase();
  const { run, busy, error } = useAction();
  const [bioShort, setBioShort] = useState(initial?.bio_short ?? "");
  const [bioLong, setBioLong] = useState(initial?.bio_long ?? "");

  if (!editable) {
    return (
      <>
        <p className="text-sm">{initial?.bio_short ?? "—"}</p>
        <p className="mt-2 whitespace-pre-wrap text-sm text-ink-soft">{initial?.bio_long}</p>
      </>
    );
  }

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        void run(async () => {
          await api.put(`${base}/groups/${groupId}/press-kit`, {
            bio_short: bioShort,
            bio_long: bioLong,
            links: (initial as { links?: unknown })?.links ?? [],
          });
          onDone();
        });
      }}
    >
      <Field label="Bio courte">
        <Input value={bioShort} onChange={(e) => setBioShort(e.target.value)} />
      </Field>
      <Field label="Bio longue">
        <Textarea rows={4} value={bioLong} onChange={(e) => setBioLong(e.target.value)} />
      </Field>
      <ErrorNote>{error}</ErrorNote>
      <Button type="submit" disabled={busy}>
        Enregistrer
      </Button>
    </form>
  );
};

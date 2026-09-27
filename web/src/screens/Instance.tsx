import React, { useState } from "react";
import { api } from "../lib/api";
import { InvitationLink } from "../components/InvitationLink";
import { useAction, useResource } from "../lib/hooks";
import { useSession } from "../lib/session";
import { fileSize } from "../lib/format";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select,
} from "../components/ui";

/**
 * Instance administration (§14). A single shared instance hosts every
 * collective; creating one is **manual**, so it lives here.
 *
 * This is also the only place an access can be created for someone who belongs
 * to no collective yet — the first admin of a brand new collective, or the
 * second instance admin (§3). Every other path goes through a collective's own
 * members screen.
 */

interface CollectiveRow {
  id: string;
  slug: string;
  name: string;
  members: number;
  groups: number;
  events: number;
}

interface UserRow {
  id: string;
  display_name: string;
  stage_name: string | null;
  email: string | null;
  is_instance_admin: boolean;
  telegram_linked: boolean;
  has_password: boolean;
  pending_invitation: boolean;
  collectives: string[];
}

interface TechnicalHealth {
  jobs: { pending: number; failed: number };
  render: { machines_online: number; queued: number };
  storage: { assets: number; bytes: number };
  telegram: boolean;
  pdf: boolean;
}

export const Instance: React.FC = () => {
  const { me } = useSession();
  const admin = !!me?.is_instance_admin;
  const collectives = useResource<CollectiveRow[]>(admin ? "/api/instance/collectives" : null);
  const users = useResource<UserRow[]>(admin ? "/api/instance/users" : null);
  const health = useResource<TechnicalHealth>(admin ? "/api/instance/health" : null);
  const { run, busy, error } = useAction();

  const [slug, setSlug] = useState("");
  const [name, setName] = useState("");
  const [adminUserId, setAdminUserId] = useState("");

  const [userForm, setUserForm] = useState(false);
  const [displayName, setDisplayName] = useState("");
  const [stageName, setStageName] = useState("");
  const [phone, setPhone] = useState("");
  const [email, setEmail] = useState("");
  const [instanceAdmin, setInstanceAdmin] = useState(false);
  const [lien, setLien] = useState<string | null>(null);

  if (!admin) {
    return (
      <>
        <PageTitle title="Instance" />
        <Card>
          <Empty>Réservé aux administrateurs d'instance.</Empty>
        </Card>
      </>
    );
  }

  if (collectives.loading) return <Loading />;

  return (
    <>
      <PageTitle
        title="Instance"
        subtitle="Collectifs hébergés, accès, et santé technique de la machine."
      />

      <ErrorNote>{collectives.error ?? users.error ?? health.error ?? error}</ErrorNote>

      <div className="mb-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Indicator
          label="Jobs en attente"
          value={health.data?.jobs.pending ?? 0}
          alert={(health.data?.jobs.failed ?? 0) > 0}
          detail={`${health.data?.jobs.failed ?? 0} en échec`}
        />
        <Indicator
          label="Machines de rendu"
          value={health.data?.render.machines_online ?? 0}
          alert={
            (health.data?.render.queued ?? 0) > 0 && (health.data?.render.machines_online ?? 0) === 0
          }
          detail={`${health.data?.render.queued ?? 0} rendu(s) en file`}
        />
        <Indicator
          label="Médias"
          value={health.data?.storage.assets ?? 0}
          detail={fileSize(health.data?.storage.bytes ?? 0)}
        />
        <Card title="Services">
          <ul className="space-y-1 text-sm">
            <li>
              Telegram{" "}
              <Badge tone={health.data?.telegram ? "good" : "warn"}>
                {health.data?.telegram ? "actif" : "journalise"}
              </Badge>
            </li>
            <li>
              PDF (Typst){" "}
              <Badge tone={health.data?.pdf ? "good" : "bad"}>
                {health.data?.pdf ? "disponible" : "absent"}
              </Badge>
            </li>
          </ul>
        </Card>
      </div>

      <div className="mb-5">
        <Card title="Nouveau collectif">
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              void run(async () => {
                await api.post("/api/instance/collectives", {
                  slug,
                  name,
                  admin_user_id: adminUserId || null,
                });
                setSlug("");
                setName("");
                setAdminUserId("");
                await collectives.reload();
                await users.reload();
              });
            }}
          >
            <div className="min-w-44 flex-1">
              <Field label="Nom">
                <Input value={name} onChange={(e) => setName(e.target.value)} required />
              </Field>
            </div>
            <div className="min-w-44 flex-1">
              <Field label="Identifiant" hint="En minuscules, sans espace.">
                <Input
                  value={slug}
                  onChange={(e) => setSlug(e.target.value)}
                  pattern="[a-z0-9-]+"
                  required
                />
              </Field>
            </div>
            <div className="min-w-44 flex-1">
              <Field label="Premier admin" hint="Sans lui, le collectif est ingérable.">
                <Select value={adminUserId} onChange={(e) => setAdminUserId(e.target.value)}>
                  <option value="">plus tard</option>
                  {(users.data ?? []).map((u) => (
                    <option key={u.id} value={u.id}>
                      {u.display_name}
                      {u.email ? ` — ${u.email}` : ""}
                    </option>
                  ))}
                </Select>
              </Field>
            </div>
            <Button type="submit" variant="primary" disabled={busy}>
              Créer
            </Button>
          </form>
          <p className="mt-2 text-xs text-ink-soft">
            Le collectif nait avec ses types d'événements, son catalogue de formats et une charte
            d'exemple.
          </p>
        </Card>
      </div>

      <div className="mb-5">
        <Card
          title="Accès"
          action={
            <Button size="sm" onClick={() => setUserForm((v) => !v)}>
              {userForm ? "fermer" : "Créer un accès"}
            </Button>
          }
        >
          {userForm && (
            <form
              className="mb-4 grid gap-3 md:grid-cols-2"
              onSubmit={(e) => {
                e.preventDefault();
                void run(async () => {
                  const res = await api.post<{ invitation_url: string }>("/api/instance/users", {
                    display_name: displayName,
                    stage_name: stageName || null,
                    phone: phone || null,
                    email: email || null,
                    is_instance_admin: instanceAdmin,
                  });
                  setLien(res.invitation_url);
                  setDisplayName("");
                  setStageName("");
                  setPhone("");
                  setEmail("");
                  setInstanceAdmin(false);
                  await users.reload();
                });
              }}
            >
              <Field label="Nom">
                <Input
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                  required
                />
              </Field>
              <Field label="Nom de scène" hint="Optionnel.">
                <Input value={stageName} onChange={(e) => setStageName(e.target.value)} />
              </Field>
              <Field label="Téléphone">
                <Input value={phone} onChange={(e) => setPhone(e.target.value)} />
              </Field>
              <Field label="Email" hint="Nécessaire pour se connecter sans Telegram.">
                <Input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </Field>
              <div className="md:col-span-2">
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={instanceAdmin}
                    onChange={(e) => setInstanceAdmin(e.target.checked)}
                  />
                  Super admin de l'instance
                </label>
                <Button type="submit" variant="primary" disabled={busy} className="mt-3">
                  Créer et generer le lien
                </Button>
              </div>
            </form>
          )}

          {!users.data || users.data.length === 0 ? (
            <Empty>Aucun accès.</Empty>
          ) : (
            <ul className="divide-y divide-line">
              {users.data.map((u) => (
                <li key={u.id} className="flex flex-wrap items-center gap-3 py-3">
                  <div className="min-w-0 flex-1">
                    <p className="font-medium">
                      {u.display_name}
                      {u.stage_name && u.stage_name !== u.display_name && (
                        <span className="text-ink-soft"> — {u.stage_name}</span>
                      )}
                    </p>
                    <p className="text-xs text-ink-soft">
                      {u.email ?? "sans email"}
                      {u.collectives.length > 0
                        ? ` · ${u.collectives.join(", ")}`
                        : " · aucun collectif"}
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    {u.is_instance_admin && <Badge tone="accent">super admin</Badge>}
                    {!u.telegram_linked && !u.has_password && (
                      <Badge tone="warn">jamais connecté</Badge>
                    )}
                    {u.pending_invitation && <Badge>invitation en attente</Badge>}
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        void run(async () => {
                          const res = await api.post<{ invitation_url: string }>(
                            `/api/instance/users/${u.id}/invitation`,
                            {},
                          );
                          setLien(res.invitation_url);
                          await users.reload();
                        })
                      }
                    >
                      nouveau lien
                    </Button>
                    <Button
                      size="sm"
                      disabled={busy || u.id === me?.id}
                      onClick={() =>
                        void run(async () => {
                          await api.patch(`/api/instance/users/${u.id}`, {
                            is_instance_admin: !u.is_instance_admin,
                          });
                          await users.reload();
                        })
                      }
                    >
                      {u.is_instance_admin ? "retirer le super admin" : "passer super admin"}
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
          <p className="mt-3 text-xs text-ink-soft">
            Un accès se crée ici uniquement quand la personne n'a encore aucun collectif. Au
            quotidien, c'est l'admin du collectif qui invite depuis ses membres.
          </p>
        </Card>
      </div>

      <InvitationLink url={lien} onClose={() => setLien(null)} />

      <Card title="Collectifs">
        {!collectives.data || collectives.data.length === 0 ? (
          <Empty icon="instance" hint="Un collectif naît vide mais utilisable : ses types d'événements, ses formats, une charte d'exemple.">
            Aucun collectif.
          </Empty>
        ) : (
          <ul className="divide-y divide-line">
            {collectives.data.map((c) => (
              <li key={c.id} className="flex flex-wrap items-center justify-between gap-3 py-3">
                <div>
                  <p className="font-medium">{c.name}</p>
                  <p className="text-xs text-ink-soft">{c.slug}</p>
                </div>
                <div className="flex gap-2 text-xs text-ink-soft">
                  <Badge>{c.members} membre(s)</Badge>
                  <Badge>{c.groups} groupe(s)</Badge>
                  <Badge>{c.events} événement(s)</Badge>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  );
};

const Indicator: React.FC<{
  label: string;
  value: number;
  detail?: string;
  alert?: boolean;
}> = ({ label, value, detail, alert }) => (
  <Card title={label}>
    <p className={`text-2xl font-semibold ${alert ? "text-accent" : ""}`}>{value}</p>
    {detail && <p className="mt-1 text-xs text-ink-soft">{detail}</p>}
  </Card>
);

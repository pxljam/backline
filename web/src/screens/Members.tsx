import React, { useState } from "react";
import { api } from "../lib/api";
import { InvitationLink } from "../components/InvitationLink";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Group, Member } from "../lib/types";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, Modal, PageTitle, Select,
} from "../components/ui";

/** Member administration (§3). No public sign-up. */
export const Members: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin, me } = useSession();
  const { data, loading, error, reload } = useResource<Member[]>(`${base}/members`);
  const { data: groups } = useResource<Group[]>(`${base}/groups`);
  const { run, busy, error: actionError } = useAction();
  const [open, setOpen] = useState(false);
  const [lien, setLien] = useState<string | null>(null);

  const [form, setForm] = useState({ display_name: "", phone: "", email: "", role: "member" });
  const [groupIds, setGroupIds] = useState<string[]>([]);
  const [editing, setEditing] = useState<Member | null>(null);

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;

  return (
    <>
      <PageTitle
        title="Membres"
        subtitle="Un compte par personne, transverse aux collectifs. Le rôle d'admin est un droit, attribuable et retirable."
        action={
          isAdmin && (
            <Button variant="primary" onClick={() => setOpen((v) => !v)}>
              {open ? "Annuler" : "Ajouter un membre"}
            </Button>
          )
        }
      />

      {open && (
        <div className="mb-5">
          <Card title="Nouveau membre">
            <form
              className="grid gap-4 md:grid-cols-2"
              onSubmit={(e) => {
                e.preventDefault();
                void run(async () => {
                  const res = await api.post<{ invitation_url: string }>(`${base}/members`, {
                    ...form,
                    email: form.email || undefined,
                    group_ids: groupIds,
                  });
                  setLien(res.invitation_url);
                  setForm({ display_name: "", phone: "", email: "", role: "member" });
                  setGroupIds([]);
                  setOpen(false);
                  await reload();
                });
              }}
            >
              <Field label="Nom">
                <Input
                  value={form.display_name}
                  onChange={(e) => setForm({ ...form, display_name: e.target.value })}
                  required
                />
              </Field>
              <Field label="Téléphone" hint="Visible des admins et des membres du même événement.">
                <Input
                  value={form.phone}
                  onChange={(e) => setForm({ ...form, phone: e.target.value })}
                />
              </Field>
              <Field label="E-mail (optionnel)">
                <Input
                  type="email"
                  value={form.email}
                  onChange={(e) => setForm({ ...form, email: e.target.value })}
                />
              </Field>
              <Field label="Rôle">
                <Select
                  value={form.role}
                  onChange={(e) => setForm({ ...form, role: e.target.value })}
                >
                  <option value="member">membre</option>
                  <option value="admin">admin du collectif</option>
                </Select>
              </Field>
              <div className="md:col-span-2">
                <Field label="Groupes">
                  <div className="flex flex-wrap gap-2">
                    {groups?.map((g) => (
                      <label
                        key={g.id}
                        className={`cursor-pointer rounded-lg border px-3 py-1.5 text-sm ${
                          groupIds.includes(g.id) ? "border-accent bg-accent text-on-accent" : "border-line"
                        }`}
                      >
                        <input
                          type="checkbox"
                          className="sr-only"
                          checked={groupIds.includes(g.id)}
                          onChange={() =>
                            setGroupIds((v) =>
                              v.includes(g.id) ? v.filter((x) => x !== g.id) : [...v, g.id],
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
                <ErrorNote>{actionError}</ErrorNote>
                <Button type="submit" variant="primary" disabled={busy} className="mt-2">
                  Créer et générer le lien
                </Button>
              </div>
            </form>
          </Card>
        </div>
      )}

      <Card>
        {!data || data.length === 0 ? (
          <Empty
            icon="member"
            hint="Chaque personne reçoit un lien d'invitation à usage unique. Il n'y a pas d'inscription libre."
            action={
              isAdmin && (
                <Button variant="primary" icon="plus" onClick={() => setOpen(true)}>
                  Inviter quelqu'un
                </Button>
              )
            }
          >
            Aucun membre.
          </Empty>
        ) : (
          <ul className="divide-y divide-line">
            {data.map((m) => (
              <li key={m.user_id} className="flex flex-wrap items-center gap-3 py-3">
                <div className="min-w-0 flex-1">
                  <p className="font-medium">
                    {m.display_name}
                    {m.stage_name && m.stage_name !== m.display_name && (
                      <span className="text-ink-soft"> — {m.stage_name}</span>
                    )}
                  </p>
                  <p className="text-xs text-ink-soft">
                    {m.phone ?? "sans téléphone"}
                    {m.email && ` · ${m.email}`}
                    {m.groups.length > 0 &&
                      ` · ${m.groups.map((gid) => groups?.find((g) => g.id === gid)?.name ?? "?").join(", ")}`}
                  </p>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  {m.is_instance_admin && <Badge tone="accent">super admin</Badge>}
                  {!m.telegram_linked && <Badge tone="warn">Telegram non lié</Badge>}
                  {m.pending_invitation && <Badge tone="neutral">invitation en attente</Badge>}
                  {isAdmin ? (
                    <Select
                      size="sm"
                      className="w-auto"
                      value={m.role}
                      disabled={busy || m.user_id === me?.id}
                      onChange={(e) =>
                        void run(async () => {
                          await api.patch(`${base}/members/${m.user_id}`, {
                            role: e.target.value,
                          });
                          await reload();
                        })
                      }
                    >
                      <option value="member">membre</option>
                      <option value="admin">admin</option>
                    </Select>
                  ) : (
                    <Badge>{m.role === "admin" ? "admin" : "membre"}</Badge>
                  )}
                  {isAdmin && (
                    <Button size="sm" disabled={busy} onClick={() => setEditing(m)}>
                      Modifier
                    </Button>
                  )}
                  {isAdmin && (
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        void run(async () => {
                          const res = await api.post<{ invitation_url: string }>(
                            `${base}/members/${m.user_id}/invitation`,
                          );
                          setLien(res.invitation_url);
                          await reload();
                        })
                      }
                    >
                      nouveau lien
                    </Button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <InvitationLink url={lien} onClose={() => setLien(null)} />

      <EditMember
        member={editing}
        groups={groups ?? []}
        base={base}
        onClose={() => setEditing(null)}
        onSaved={async () => {
          setEditing(null);
          await reload();
        }}
      />
    </>
  );
};

/**
 * Editing a person, not just promoting them.
 *
 * Until now the only thing an admin could change was the role: a name typed
 * wrong at invitation time, or an address that locks someone out of the
 * password fallback, had to be fixed in the database.
 */
const EditMember: React.FC<{
  member: Member | null;
  groups: Group[];
  base: string;
  onClose: () => void;
  onSaved: () => void | Promise<void>;
}> = ({ member, groups, base, onClose, onSaved }) => {
  const { run, busy, error } = useAction();
  const [form, setForm] = useState({ display_name: "", stage_name: "", phone: "", email: "" });
  const [groupIds, setGroupIds] = useState<string[]>([]);

  // Re-seeded whenever another member is opened.
  React.useEffect(() => {
    if (!member) return;
    setForm({
      display_name: member.display_name,
      stage_name: member.stage_name ?? "",
      phone: member.phone ?? "",
      email: member.email ?? "",
    });
    setGroupIds(member.groups);
  }, [member]);

  if (!member) return null;

  return (
    <Modal open={!!member} onClose={onClose} title={`Modifier ${member.display_name}`}>
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            await api.patch(`${base}/members/${member.user_id}`, {
              display_name: form.display_name,
              stage_name: form.stage_name || null,
              phone: form.phone || null,
              email: form.email || null,
              group_ids: groupIds,
            });
            await onSaved();
          });
        }}
      >
        <Field label="Nom">
          <Input
            value={form.display_name}
            onChange={(e) => setForm({ ...form, display_name: e.target.value })}
            required
          />
        </Field>
        <Field label="Nom de scène" hint="Optionnel.">
          <Input
            value={form.stage_name}
            onChange={(e) => setForm({ ...form, stage_name: e.target.value })}
          />
        </Field>
        <Field label="Téléphone" hint="Visible des admins et des membres du même événement (§3).">
          <Input value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} />
        </Field>
        <Field label="E-mail" hint="Sert à la connexion de secours, sans Telegram.">
          <Input
            type="email"
            value={form.email}
            onChange={(e) => setForm({ ...form, email: e.target.value })}
          />
        </Field>
        {groups.length > 0 && (
          <Field label="Groupes">
            <div className="flex flex-wrap gap-2">
              {groups.map((g) => (
                <label
                  key={g.id}
                  className={`cursor-pointer rounded-control border px-3 py-1.5 text-sm ${
                    groupIds.includes(g.id)
                      ? "border-accent bg-accent text-on-accent"
                      : "border-line-strong"
                  }`}
                >
                  <input
                    type="checkbox"
                    className="sr-only"
                    checked={groupIds.includes(g.id)}
                    onChange={() =>
                      setGroupIds((v) =>
                        v.includes(g.id) ? v.filter((x) => x !== g.id) : [...v, g.id],
                      )
                    }
                  />
                  {g.name}
                </label>
              ))}
            </div>
          </Field>
        )}
        <ErrorNote>{error}</ErrorNote>
        <div className="flex justify-end gap-2 pt-1">
          <Button type="button" onClick={onClose}>
            Annuler
          </Button>
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? "…" : "Enregistrer"}
          </Button>
        </div>
      </form>
    </Modal>
  );
};

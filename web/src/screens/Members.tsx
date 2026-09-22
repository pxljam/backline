import React, { useState } from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Group, Member } from "../lib/types";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select,
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

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;

  return (
    <>
      <PageTitle
        title="Membres"
        subtitle="Un compte par personne, transverse aux collectifs. Le role d'admin est un droit, attribuable et retirable."
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
              <Field label="Telephone" hint="Visible des admins et des membres du meme evenement.">
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
              <Field label="Role">
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
                          groupIds.includes(g.id) ? "border-ink bg-ink text-paper" : "border-line"
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
                  Creer et generer le lien
                </Button>
              </div>
            </form>

            {lien && (
              <div className="mt-4 rounded-lg border border-line bg-paper px-3 py-3">
                <p className="text-xs uppercase tracking-wide text-ink-soft">
                  Lien d'invitation — a usage unique
                </p>
                <code className="mt-1 block break-all text-sm">{lien}</code>
                <Button size="sm" className="mt-2" onClick={() => void navigator.clipboard.writeText(lien)}>
                  copier
                </Button>
              </div>
            )}
          </Card>
        </div>
      )}

      <Card>
        {!data || data.length === 0 ? (
          <Empty>Aucun membre.</Empty>
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
                    {m.phone ?? "sans telephone"}
                    {m.email && ` · ${m.email}`}
                    {m.groups.length > 0 &&
                      ` · ${m.groups.map((gid) => groups?.find((g) => g.id === gid)?.name ?? "?").join(", ")}`}
                  </p>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  {!m.telegram_linked && <Badge tone="warn">Telegram non lie</Badge>}
                  {m.pending_invitation && <Badge tone="neutral">invitation en attente</Badge>}
                  {isAdmin ? (
                    <Select
                      className="w-auto py-1 text-xs"
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
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        void run(async () => {
                          const res = await api.post<{ invitation_url: string }>(
                            `${base}/members/${m.user_id}/invitation`,
                          );
                          setLien(res.invitation_url);
                          setOpen(true);
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
    </>
  );
};

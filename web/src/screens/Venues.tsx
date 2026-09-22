import React, { useState } from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Venue } from "../lib/types";
import { Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Textarea } from "../components/ui";

export const Venues: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { data, loading, error, reload } = useResource<Venue[]>(`${base}/venues`);
  const { run, busy, error: actionError } = useAction();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState({ name: "", address: "", city: "", capacity: "", notes: "" });

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;

  return (
    <>
      <PageTitle
        title="Lieux"
        subtitle="Carnet d'adresses, contacts, historique."
        action={
          isAdmin && (
            <Button variant="primary" onClick={() => setOpen((v) => !v)}>
              {open ? "Annuler" : "Nouveau lieu"}
            </Button>
          )
        }
      />

      {open && (
        <div className="mb-5">
          <Card title="Nouveau lieu">
            <form
              className="grid gap-4 md:grid-cols-2"
              onSubmit={(e) => {
                e.preventDefault();
                void run(async () => {
                  await api.post(`${base}/venues`, {
                    ...form,
                    capacity: form.capacity ? Number(form.capacity) : undefined,
                  });
                  setForm({ name: "", address: "", city: "", capacity: "", notes: "" });
                  setOpen(false);
                  await reload();
                });
              }}
            >
              <Field label="Nom">
                <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required />
              </Field>
              <Field label="Ville">
                <Input value={form.city} onChange={(e) => setForm({ ...form, city: e.target.value })} />
              </Field>
              <Field label="Adresse">
                <Input value={form.address} onChange={(e) => setForm({ ...form, address: e.target.value })} />
              </Field>
              <Field label="Jauge">
                <Input
                  type="number"
                  value={form.capacity}
                  onChange={(e) => setForm({ ...form, capacity: e.target.value })}
                />
              </Field>
              <div className="md:col-span-2">
                <Field label="Notes">
                  <Textarea rows={2} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} />
                </Field>
              </div>
              <div className="md:col-span-2">
                <ErrorNote>{actionError}</ErrorNote>
                <Button type="submit" variant="primary" disabled={busy} className="mt-2">
                  Creer
                </Button>
              </div>
            </form>
          </Card>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2">
        {!data || data.length === 0 ? (
          <Card>
            <Empty>Aucun lieu.</Empty>
          </Card>
        ) : (
          data.map((v) => (
            <Card key={v.id} title={v.name}>
              <p className="text-sm text-ink-soft">
                {[v.address, v.city].filter(Boolean).join(", ") || "adresse non renseignee"}
                {v.capacity ? ` · jauge ${v.capacity}` : ""}
              </p>
              {v.notes && <p className="mt-2 text-sm">{v.notes}</p>}
              <p className="mt-2 text-xs text-ink-soft">
                {v.past_events} evenement{v.past_events > 1 ? "s" : ""} dans ce lieu
              </p>

              <ul className="mt-3 divide-y divide-line">
                {v.contacts.map((c) => (
                  <li key={c.id} className="py-2 text-sm">
                    <span className="font-medium">{c.name}</span>
                    {c.role && <span className="text-ink-soft"> — {c.role}</span>}
                    <span className="block text-xs text-ink-soft">
                      {[c.phone, c.email].filter(Boolean).join(" · ")}
                    </span>
                  </li>
                ))}
              </ul>

              {isAdmin && <AddContact venueId={v.id} onDone={() => void reload()} />}
            </Card>
          ))
        )}
      </div>
    </>
  );
};

const AddContact: React.FC<{ venueId: string; onDone: () => void }> = ({ venueId, onDone }) => {
  const base = useCollectiveBase();
  const { run, busy } = useAction();
  const [form, setForm] = useState({ name: "", role: "", phone: "", email: "" });
  const [open, setOpen] = useState(false);

  if (!open)
    return (
      <button className="mt-3 text-xs underline text-ink-soft" onClick={() => setOpen(true)}>
        + un contact
      </button>
    );

  return (
    <form
      className="mt-3 grid gap-2 sm:grid-cols-2"
      onSubmit={(e) => {
        e.preventDefault();
        void run(async () => {
          await api.post(`${base}/venues/${venueId}/contacts`, form);
          setForm({ name: "", role: "", phone: "", email: "" });
          setOpen(false);
          onDone();
        });
      }}
    >
      <Input placeholder="Nom" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required />
      <Input placeholder="Role" value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value })} />
      <Input placeholder="Telephone" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} />
      <Input placeholder="E-mail" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} />
      <Button type="submit" size="sm" variant="primary" disabled={busy}>
        Ajouter
      </Button>
    </form>
  );
};

import React, { useState } from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Venue } from "../lib/types";
import {
  Button, Card, Empty, ErrorNote, Field, Input, Loading, Modal, PageTitle, Textarea,
} from "../components/ui";

export const Venues: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { data, loading, error, reload } = useResource<Venue[]>(`${base}/venues`);
  const { run, busy, error: actionError } = useAction();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState({ name: "", address: "", city: "", capacity: "", notes: "" });
  const [editing, setEditing] = useState<Venue | null>(null);

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
                  Créer
                </Button>
              </div>
            </form>
          </Card>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2">
        {!data || data.length === 0 ? (
          <Card>
            <Empty
              icon="venue"
              hint="Le carnet d'adresses : contacts, conditions, historique des événements joués là."
              action={
                isAdmin && (
                  <Button variant="primary" icon="plus" onClick={() => setOpen(true)}>
                    Ajouter un lieu
                  </Button>
                )
              }
            >
              Aucun lieu.
            </Empty>
          </Card>
        ) : (
          data.map((v) => (
            <Card
              key={v.id}
              title={v.name}
              action={
                isAdmin && (
                  <Button size="sm" onClick={() => setEditing(v)}>
                    Modifier
                  </Button>
                )
              }
            >
              <p className="text-sm text-ink-soft">
                {[v.address, v.city].filter(Boolean).join(", ") || "adresse non renseignée"}
                {v.capacity ? ` · jauge ${v.capacity}` : ""}
              </p>
              {v.notes && <p className="mt-2 text-sm">{v.notes}</p>}
              <p className="mt-2 text-xs text-ink-soft">
                {v.past_events} événement{v.past_events > 1 ? "s" : ""} dans ce lieu
              </p>

              <ul className="mt-3 divide-y divide-line">
                {v.contacts.map((c) => (
                  <li key={c.id} className="flex items-start justify-between gap-3 py-2 text-sm">
                    <span className="min-w-0">
                      <span className="font-medium">{c.name}</span>
                      {c.role && <span className="text-ink-soft"> — {c.role}</span>}
                      <span className="block text-xs text-ink-soft">
                        {[c.phone, c.email].filter(Boolean).join(" · ")}
                      </span>
                    </span>
                    {isAdmin && (
                      <Button
                        size="sm"
                        variant="danger"
                        disabled={busy}
                        onClick={() =>
                          void run(async () => {
                            await api.del(`${base}/venues/${v.id}/contacts/${c.id}`);
                            await reload();
                          })
                        }
                      >
                        retirer
                      </Button>
                    )}
                  </li>
                ))}
              </ul>

              {isAdmin && <AddContact venueId={v.id} onDone={() => void reload()} />}
            </Card>
          ))
        )}
      </div>

      <EditVenue
        venue={editing}
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

/** A venue learnt on the night is a venue corrected afterwards. */
const EditVenue: React.FC<{
  venue: Venue | null;
  base: string;
  onClose: () => void;
  onSaved: () => void | Promise<void>;
}> = ({ venue, base, onClose, onSaved }) => {
  const { run, busy, error } = useAction();
  const [form, setForm] = useState({ name: "", address: "", city: "", capacity: "", notes: "" });

  React.useEffect(() => {
    if (!venue) return;
    setForm({
      name: venue.name,
      address: venue.address ?? "",
      city: venue.city ?? "",
      capacity: venue.capacity ? String(venue.capacity) : "",
      notes: venue.notes ?? "",
    });
  }, [venue]);

  if (!venue) return null;

  return (
    <Modal open={!!venue} onClose={onClose} title={`Modifier ${venue.name}`}>
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            await api.patch(`${base}/venues/${venue.id}`, {
              name: form.name,
              address: form.address || null,
              city: form.city || null,
              capacity: form.capacity ? Number(form.capacity) : null,
              notes: form.notes || null,
            });
            await onSaved();
          });
        }}
      >
        <Field label="Nom">
          <Input
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            required
          />
        </Field>
        <Field label="Adresse">
          <Input
            value={form.address}
            onChange={(e) => setForm({ ...form, address: e.target.value })}
          />
        </Field>
        <Field label="Ville">
          <Input value={form.city} onChange={(e) => setForm({ ...form, city: e.target.value })} />
        </Field>
        <Field label="Jauge">
          <Input
            type="number"
            value={form.capacity}
            onChange={(e) => setForm({ ...form, capacity: e.target.value })}
          />
        </Field>
        <Field label="Notes" hint="Chargement, loges, contraintes : ce qu'on réapprend sinon.">
          <Textarea
            rows={3}
            value={form.notes}
            onChange={(e) => setForm({ ...form, notes: e.target.value })}
          />
        </Field>
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
      <Input placeholder="Rôle" value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value })} />
      <Input placeholder="Téléphone" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} />
      <Input placeholder="E-mail" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} />
      <Button type="submit" size="sm" variant="primary" disabled={busy}>
        Ajouter
      </Button>
    </form>
  );
};

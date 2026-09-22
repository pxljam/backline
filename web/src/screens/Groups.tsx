import React, { useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Group } from "../lib/types";
import { Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle } from "../components/ui";

export const Groups: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { data, loading, error, reload } = useResource<Group[]>(`${base}/groups`);
  const { run, busy, error: actionError } = useAction();
  const [name, setName] = useState("");
  const [open, setOpen] = useState(false);

  if (loading) return <Loading />;
  if (error) return <ErrorNote>{error}</ErrorNote>;

  return (
    <>
      <PageTitle
        title="Groupes"
        subtitle="Un seul concept, solo compris. Appartenances multiples et libres."
        action={
          isAdmin && (
            <Button variant="primary" onClick={() => setOpen((v) => !v)}>
              {open ? "Annuler" : "Nouveau groupe"}
            </Button>
          )
        }
      />

      {open && (
        <div className="mb-5">
          <Card title="Nouveau groupe">
            <form
              className="flex flex-wrap items-end gap-3"
              onSubmit={(e) => {
                e.preventDefault();
                void run(async () => {
                  await api.post(`${base}/groups`, {
                    name,
                    slug: name
                      .toLowerCase()
                      .normalize("NFD")
                      .replace(/[̀-ͯ]/g, "")
                      .replace(/[^a-z0-9]+/g, "-")
                      .replace(/^-|-$/g, ""),
                  });
                  setName("");
                  setOpen(false);
                  await reload();
                });
              }}
            >
              <div className="min-w-60 flex-1">
                <Field label="Nom">
                  <Input value={name} onChange={(e) => setName(e.target.value)} required />
                </Field>
              </div>
              <Button type="submit" variant="primary" disabled={busy}>
                Creer
              </Button>
            </form>
            <ErrorNote>{actionError}</ErrorNote>
            <p className="mt-2 text-xs text-ink-soft">
              Un groupe nait avec sa charte locale, qui surcharge celle du collectif.
            </p>
          </Card>
        </div>
      )}

      <Card>
        {!data || data.length === 0 ? (
          <Empty>Aucun groupe.</Empty>
        ) : (
          <ul className="divide-y divide-line">
            {data.map((g) => (
              <li key={g.id}>
                <Link to={`/groupes/${g.id}`} className="flex flex-wrap items-center gap-3 py-3">
                  <div className="min-w-0 flex-1">
                    <p className="font-medium">{g.name}</p>
                    <p className="text-xs text-ink-soft">
                      {g.members.length} membre{g.members.length > 1 ? "s" : ""}
                      {g.members.length > 0 &&
                        ` — ${g.members.map((m) => m.stage_name ?? m.display_name).join(", ")}`}
                    </p>
                  </div>
                  {g.has_tech_rider ? (
                    <Badge tone="good">fiche technique</Badge>
                  ) : (
                    <Badge tone="warn">sans fiche technique</Badge>
                  )}
                </Link>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  );
};

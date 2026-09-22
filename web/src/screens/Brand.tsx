import React, { useEffect, useState } from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { BrandToken, BrandView, Group } from "../lib/types";
import { Button, Card, ErrorNote, Field, Input, Loading, PageTitle, Select } from "../components/ui";

/**
 * La charte est **de la donnee, jamais du code** (§8). Saisir les vraies
 * valeurs de Bonsoir Techno ne demandera aucune modification de code.
 */
export const Brand: React.FC = () => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { data: groups } = useResource<Group[]>(`${base}/groups`);
  const [groupId, setGroupId] = useState("");
  const brand = useResource<BrandView>(
    `${base}/studio/brand${groupId ? `?group_id=${groupId}` : ""}`,
    [groupId],
  );
  const { run, busy, error } = useAction();
  const [tokens, setTokens] = useState<BrandToken[]>([]);

  useEffect(() => {
    if (brand.data) setTokens(brand.data.tokens);
  }, [brand.data]);

  if (brand.loading) return <Loading />;

  const modifier = (index: number, patch: Partial<BrandToken>) =>
    setTokens((t) => t.map((x, i) => (i === index ? { ...x, ...patch } : x)));

  const couleurs = tokens.filter((t) => t.kind === "color");
  const polices = tokens.filter((t) => t.kind === "font");
  const regles = tokens.filter((t) => t.kind === "rule");

  return (
    <>
      <PageTitle
        title="Charte graphique"
        subtitle="Une charte par collectif, surcharge partielle par groupe."
        action={
          <Select className="w-auto" value={groupId} onChange={(e) => setGroupId(e.target.value)}>
            <option value="">charte du collectif</option>
            {groups?.map((g) => (
              <option key={g.id} value={g.id}>
                charte de {g.name}
              </option>
            ))}
          </Select>
        }
      />

      <ErrorNote>{brand.error ?? error}</ErrorNote>

      {groupId && (
        <p className="mb-4 text-sm text-ink-soft">
          Ce que ce groupe ne redefinit pas est herite de la charte du collectif.
        </p>
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="Couleurs">
          <ul className="space-y-2">
            {couleurs.map((t) => {
              const index = tokens.indexOf(t);
              const hex = String((t.value as { hex?: string }).hex ?? "#000000");
              const herite = brand.data?.inherited?.find(
                (i) => i.kind === "color" && i.key === t.key,
              );
              return (
                <li key={t.key} className="flex items-center gap-3">
                  <input
                    type="color"
                    value={hex}
                    disabled={!isAdmin}
                    onChange={(e) => modifier(index, { value: { hex: e.target.value } })}
                    className="h-9 w-12 cursor-pointer rounded border border-line"
                  />
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium">{t.label}</p>
                    <p className="text-xs text-ink-soft">
                      {t.key} · {hex}
                      {herite &&
                        herite.value &&
                        (herite.value as { hex?: string }).hex !== hex &&
                        ` · collectif : ${(herite.value as { hex?: string }).hex}`}
                    </p>
                  </div>
                </li>
              );
            })}
          </ul>
        </Card>

        <Card title="Typographies">
          <ul className="space-y-3">
            {polices.map((t) => {
              const index = tokens.indexOf(t);
              const v = t.value as { family?: string; weight?: string; stack?: string };
              return (
                <li key={t.key}>
                  <Field label={t.label}>
                    <div className="flex gap-2">
                      <Input
                        value={v.family ?? ""}
                        disabled={!isAdmin}
                        onChange={(e) =>
                          modifier(index, {
                            value: { ...v, family: e.target.value, stack: `${e.target.value}, Helvetica, Arial, sans-serif` },
                          })
                        }
                      />
                      <Input
                        className="max-w-24"
                        value={v.weight ?? ""}
                        disabled={!isAdmin}
                        onChange={(e) => modifier(index, { value: { ...v, weight: e.target.value } })}
                      />
                    </div>
                  </Field>
                  <p
                    className="mt-1 truncate text-lg"
                    style={{ fontFamily: v.stack, fontWeight: v.weight }}
                  >
                    Bonsoir Techno — 0123456789
                  </p>
                </li>
              );
            })}
          </ul>
          <p className="mt-3 text-xs text-ink-soft">
            Les fichiers de police se televersent dans la bibliotheque de medias.
          </p>
        </Card>

        <Card title="Regles">
          <ul className="space-y-2 text-sm">
            {regles.map((t) => (
              <li key={t.key} className="flex items-start justify-between gap-3">
                <span>{t.label}</span>
                <code className="text-xs text-ink-soft">{JSON.stringify(t.value)}</code>
              </li>
            ))}
          </ul>
          <p className="mt-3 text-xs text-ink-soft">
            Zone de securite, placement du logo, casse des titres : l'editeur les applique.
          </p>
        </Card>
      </div>

      {isAdmin && (
        <Button
          variant="primary"
          className="mt-5"
          disabled={busy}
          onClick={() =>
            void run(async () => {
              await api.put(`${base}/studio/brand/tokens`, {
                group_id: groupId || undefined,
                tokens,
              });
              await brand.reload();
            })
          }
        >
          {busy ? "…" : "Enregistrer la charte"}
        </Button>
      )}
    </>
  );
};

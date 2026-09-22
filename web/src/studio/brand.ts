import { useEffect, useState } from "react";
import type { Brand, MediaMap } from "@backline/layout";
import { api } from "../lib/api";
import type { BrandToken } from "../lib/types";

/**
 * Tokens de charte tels que l'API les renvoie -> objet `Brand` du moteur de
 * rendu. L'heritage collectif -> groupe est **partiel** : le groupe ne
 * redefinit que ce qu'il redefinit (§8).
 */
export function toBrand(tokens: BrandToken[], inherited?: BrandToken[] | null): Brand {
  const brand: Brand = {};
  for (const token of [...(inherited ?? []), ...tokens]) {
    const bucket = (brand[token.kind] ?? {}) as Record<string, unknown>;
    bucket[token.key] = token.value;
    (brand as Record<string, unknown>)[token.kind] = bucket;
  }
  return brand;
}

/** Libelle lisible d'un token, pour les selecteurs de l'editeur. */
export function tokenLabels(tokens: BrandToken[], kind: BrandToken["kind"]) {
  return tokens.filter((t) => t.kind === kind).map((t) => ({ key: t.key, label: t.label }));
}

/**
 * URLs signees des medias references par un gabarit. Elles expirent : on les
 * redemande a chaque ouverture de l'editeur plutot que de les stocker.
 */
export function useMediaMap(base: string, assetIds: string[]): MediaMap {
  const [media, setMedia] = useState<MediaMap>({});
  const cle = assetIds.filter(Boolean).sort().join(",");

  useEffect(() => {
    const ids = cle ? cle.split(",") : [];
    let annule = false;
    void (async () => {
      const entries = await Promise.all(
        ids.map(async (id) => {
          try {
            const r = await api.get<{ url: string; mime: string }>(
              `${base}/studio/assets/${id}/url`,
            );
            return [id, { url: r.url, mime: r.mime }] as const;
          } catch {
            // Un media introuvable laisse un cadre vide visible, jamais un trou
            // silencieux.
            return null;
          }
        }),
      );
      if (annule) return;
      setMedia(Object.fromEntries(entries.filter((e): e is NonNullable<typeof e> => e !== null)));
    })();
    return () => {
      annule = true;
    };
  }, [base, cle]);

  return media;
}

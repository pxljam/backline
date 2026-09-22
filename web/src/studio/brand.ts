import { useEffect, useState } from "react";
import type { Brand, MediaMap } from "@backline/layout";
import { api } from "../lib/api";
import type { BrandToken } from "../lib/types";

/**
 * Brand tokens as the API returns them -> the render engine's `Brand` object.
 * Collective -> group inheritance is **partial**: the group only redefines
 * what it redefines (§8).
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

/** Readable label for a token, for the editor's selectors. */
export function tokenLabels(tokens: BrandToken[], kind: BrandToken["kind"]) {
  return tokens.filter((t) => t.kind === kind).map((t) => ({ key: t.key, label: t.label }));
}

/**
 * Signed URLs for the media a template references. They expire, so we ask for
 * them again each time the editor opens rather than storing them.
 */
export function useMediaMap(base: string, assetIds: string[]): MediaMap {
  const [media, setMedia] = useState<MediaMap>({});
  const key = assetIds.filter(Boolean).sort().join(",");

  useEffect(() => {
    const ids = key ? key.split(",") : [];
    let cancelled = false;
    void (async () => {
      const entries = await Promise.all(
        ids.map(async (id) => {
          try {
            const r = await api.get<{ url: string; mime: string }>(
              `${base}/studio/assets/${id}/url`,
            );
            return [id, { url: r.url, mime: r.mime }] as const;
          } catch {
            // Missing media leaves a visible empty frame, never a silent
            // hole.
            return null;
          }
        }),
      );
      if (cancelled) return;
      setMedia(Object.fromEntries(entries.filter((e): e is NonNullable<typeof e> => e !== null)));
    })();
    return () => {
      cancelled = true;
    };
  }, [base, key]);

  return media;
}

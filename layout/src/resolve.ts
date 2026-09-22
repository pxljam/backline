import type { Brand, FieldData, MediaMap } from "./types";

/**
 * Replaces automatic `{{path}}` fields with their value (§9.1).
 *
 * A missing field becomes an empty string rather than a rendered `undefined`:
 * a poster with a hole in it can be fixed, a poster carrying the word
 * "undefined" is a poster already in the bin.
 */
export function interpolate(template: string, data: FieldData): string {
  if (!template) return "";
  return template.replace(/\{\{\s*([\w.]+)\s*\}\}/g, (_match, path: string) => {
    const value = lookup(data, path);
    if (value === null || value === undefined) return "";
    if (Array.isArray(value)) return value.map(String).join("\n");
    return String(value);
  });
}

function lookup(data: unknown, path: string): unknown {
  return path.split(".").reduce<unknown>((acc, key) => {
    if (acc && typeof acc === "object") return (acc as Record<string, unknown>)[key];
    return undefined;
  }, data);
}

/**
 * Colour behind a brand token. **The editor only ever offers tokens**, so the
 * brand cannot be left by accident (§9.1). An unknown token resolves to a
 * visible neutral colour here, never to silent transparency.
 */
export function color(brand: Brand, token: string | undefined, fallback = "#111111"): string {
  if (!token) return fallback;
  return brand.color?.[token]?.hex ?? fallback;
}

export function fontStack(brand: Brand, token: string | undefined): string {
  const font = token ? brand.font?.[token] : undefined;
  return font?.stack ?? font?.family ?? "Inter, Helvetica, Arial, sans-serif";
}

export function fontWeight(brand: Brand, token: string | undefined): string {
  return (token ? brand.font?.[token]?.weight : undefined) ?? "400";
}

export function mediaUrl(
  media: MediaMap,
  assetId: string | undefined,
  src: string | undefined,
): string | undefined {
  if (assetId && media[assetId]) return media[assetId].url;
  return src;
}

/**
 * Safe areas for the format: the interface bars that cover content (§9.1).
 * Fractional values, displayed by the editor.
 */
export function safeArea(
  brand: Brand,
  width: number,
  height: number,
): { top: number; bottom: number; x: number } {
  const rule = brand.rule?.safe_area as
    | { top?: number; bottom?: number; x?: number }
    | undefined;
  if (rule) {
    return { top: rule.top ?? 0.06, bottom: rule.bottom ?? 0.06, x: rule.x ?? 0.06 };
  }
  // A 9:16 story loses far more room than a square post: the app's own
  // buttons eat into the top and the bottom.
  const tall = height / width > 1.5;
  return tall ? { top: 0.14, bottom: 0.18, x: 0.06 } : { top: 0.06, bottom: 0.06, x: 0.06 };
}

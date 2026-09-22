import type { Brand, FieldData, MediaMap } from "./types";

/**
 * Remplace les champs automatiques `{{chemin}}` par leur valeur (§9.1).
 *
 * Un champ absent devient une chaine vide plutot qu'un `undefined` affiche :
 * une affiche avec un trou est corrigeable, une affiche portant le mot
 * « undefined » est une affiche partie a la poubelle.
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
 * Couleur d'un token de charte. **Seuls les tokens sont proposes dans
 * l'editeur** : on ne peut pas sortir de la charte par accident (§9.1). Ici, un
 * token inconnu se resout en une couleur neutre visible, jamais en transparent
 * silencieux.
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
 * Zones de securite du format : les bandeaux d'interface qui masquent le
 * contenu (§9.1). Valeurs en fraction, affichees par l'editeur.
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
  // Une story 9:16 perd bien plus de place qu'un post carre : les boutons de
  // l'application mangent le haut et le bas.
  const tall = height / width > 1.5;
  return tall ? { top: 0.14, bottom: 0.18, x: 0.06 } : { top: 0.06, bottom: 0.06, x: 0.06 };
}

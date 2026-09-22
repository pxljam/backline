import type { Block, BlockType } from "@backline/layout";

/** Fabrique de blocs : tout nouveau bloc nait centre et visible. */
export function nouveauBloc(type: BlockType, index: number): Block {
  const id = `b${Date.now().toString(36)}${index}`;
  const commun = { id, type, x: 0.1, y: 0.4, w: 0.8, h: 0.15, z: index };

  switch (type) {
    case "text":
      return {
        ...commun,
        props: {
          content: "Texte",
          fontToken: "title",
          colorToken: "text",
          size: 0.08,
          align: "left",
          valign: "top",
        },
      };
    case "image":
      return { ...commun, y: 0.2, h: 0.4, props: { fit: "cover" } };
    case "video":
      return { ...commun, y: 0.2, h: 0.4, props: { fit: "cover" } };
    case "logo":
      return { ...commun, x: 0.08, y: 0.06, w: 0.2, h: 0.1, props: { logoToken: "primary" } };
    case "shape":
      return { ...commun, props: { shape: "rect", fillToken: "accent" } };
    case "gradient":
      return {
        ...commun,
        x: 0,
        y: 0,
        w: 1,
        h: 1,
        props: { fromToken: "primary", toToken: "accent", angle: 180 },
      };
    case "group":
      return { ...commun, props: {}, children: [] };
    default:
      return { ...commun, props: {} };
  }
}

export const NOMS_BLOCS: Record<BlockType, string> = {
  text: "Texte",
  image: "Image",
  video: "Video",
  logo: "Logo",
  shape: "Forme",
  gradient: "Degrade",
  group: "Groupe",
};

export function dupliquer(block: Block): Block {
  return {
    ...structuredClone(block),
    id: `b${Date.now().toString(36)}${Math.floor(Math.random() * 1000)}`,
    x: Math.min(0.9, block.x + 0.02),
    y: Math.min(0.9, block.y + 0.02),
  };
}

export function deplacerCalque(blocks: Block[], id: string, sens: -1 | 1): Block[] {
  const ordonnes = [...blocks].sort((a, b) => (a.z ?? 0) - (b.z ?? 0));
  const i = ordonnes.findIndex((b) => b.id === id);
  const j = i + sens;
  if (i < 0 || j < 0 || j >= ordonnes.length) return blocks;
  [ordonnes[i], ordonnes[j]] = [ordonnes[j], ordonnes[i]];
  return ordonnes.map((b, index) => ({ ...b, z: index }));
}

/** Aligne un bloc sur le canevas — centrage et bords, sans calcul mental. */
export function aligner(block: Block, quoi: string): Block {
  switch (quoi) {
    case "gauche":
      return { ...block, x: 0 };
    case "centre-h":
      return { ...block, x: (1 - block.w) / 2 };
    case "droite":
      return { ...block, x: 1 - block.w };
    case "haut":
      return { ...block, y: 0 };
    case "centre-v":
      return { ...block, y: (1 - block.h) / 2 };
    case "bas":
      return { ...block, y: 1 - block.h };
    default:
      return block;
  }
}

/** Repartit verticalement des blocs entre le premier et le dernier (§9.1). */
export function repartirVertical(blocks: Block[], ids: string[]): Block[] {
  const cibles = blocks.filter((b) => ids.includes(b.id)).sort((a, b) => a.y - b.y);
  if (cibles.length < 3) return blocks;
  const debut = cibles[0].y;
  const fin = cibles[cibles.length - 1].y;
  const pas = (fin - debut) / (cibles.length - 1);
  const positions = new Map(cibles.map((b, i) => [b.id, debut + i * pas]));
  return blocks.map((b) => (positions.has(b.id) ? { ...b, y: positions.get(b.id)! } : b));
}

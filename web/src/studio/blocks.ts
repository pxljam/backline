import type { Block, BlockType } from "@backline/layout";

/** Block factory: every new block is born centred and visible. */
export function newBlock(type: BlockType, index: number): Block {
  const id = `b${Date.now().toString(36)}${index}`;
  const common = { id, type, x: 0.1, y: 0.4, w: 0.8, h: 0.15, z: index };

  switch (type) {
    case "text":
      return {
        ...common,
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
      return { ...common, y: 0.2, h: 0.4, props: { fit: "cover" } };
    case "video":
      return { ...common, y: 0.2, h: 0.4, props: { fit: "cover" } };
    case "logo":
      return { ...common, x: 0.08, y: 0.06, w: 0.2, h: 0.1, props: { logoToken: "primary" } };
    case "shape":
      return { ...common, props: { shape: "rect", fillToken: "accent" } };
    case "gradient":
      return {
        ...common,
        x: 0,
        y: 0,
        w: 1,
        h: 1,
        props: { fromToken: "primary", toToken: "accent", angle: 180 },
      };
    case "group":
      return { ...common, props: {}, children: [] };
    default:
      return { ...common, props: {} };
  }
}

export const BLOCK_NAMES: Record<BlockType, string> = {
  text: "Texte",
  image: "Image",
  video: "Video",
  logo: "Logo",
  shape: "Forme",
  gradient: "Degrade",
  group: "Groupe",
};

export function duplicateBlock(block: Block): Block {
  return {
    ...structuredClone(block),
    id: `b${Date.now().toString(36)}${Math.floor(Math.random() * 1000)}`,
    x: Math.min(0.9, block.x + 0.02),
    y: Math.min(0.9, block.y + 0.02),
  };
}

export function moveLayer(blocks: Block[], id: string, direction: -1 | 1): Block[] {
  const ordered = [...blocks].sort((a, b) => (a.z ?? 0) - (b.z ?? 0));
  const i = ordered.findIndex((b) => b.id === id);
  const j = i + direction;
  if (i < 0 || j < 0 || j >= ordered.length) return blocks;
  [ordered[i], ordered[j]] = [ordered[j], ordered[i]];
  return ordered.map((b, index) => ({ ...b, z: index }));
}

/** Aligns a block on the canvas — centring and edges, no mental arithmetic. */
export function align(block: Block, to: string): Block {
  switch (to) {
    case "left":
      return { ...block, x: 0 };
    case "center-h":
      return { ...block, x: (1 - block.w) / 2 };
    case "right":
      return { ...block, x: 1 - block.w };
    case "top":
      return { ...block, y: 0 };
    case "center-v":
      return { ...block, y: (1 - block.h) / 2 };
    case "bottom":
      return { ...block, y: 1 - block.h };
    default:
      return block;
  }
}

/** Spreads blocks vertically between the first and the last one (§9.1). */
export function distributeVertically(blocks: Block[], ids: string[]): Block[] {
  const targets = blocks.filter((b) => ids.includes(b.id)).sort((a, b) => a.y - b.y);
  if (targets.length < 3) return blocks;
  const start = targets[0].y;
  const end = targets[targets.length - 1].y;
  const step = (end - start) / (targets.length - 1);
  const positions = new Map(targets.map((b, i) => [b.id, start + i * step]));
  return blocks.map((b) => (positions.has(b.id) ? { ...b, y: positions.get(b.id)! } : b));
}

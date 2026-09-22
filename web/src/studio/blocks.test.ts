import { describe, expect, it } from "vitest";
import type { Block } from "@backline/layout";
import { align, moveLayer, duplicateBlock, newBlock, distributeVertically } from "./blocks";
import { toBrand } from "./brand";
import type { BrandToken } from "../lib/types";

const block = (id: string, y: number): Block => ({
  id,
  type: "text",
  x: 0.1,
  y,
  w: 0.4,
  h: 0.1,
  props: { content: id },
});

describe("block manipulation", () => {
  it("centres a block without mental arithmetic", () => {
    expect(align(block("a", 0), "center-h").x).toBeCloseTo(0.3);
    expect(align(block("a", 0), "right").x).toBeCloseTo(0.6);
    expect(align(block("a", 0), "bottom").y).toBeCloseTo(0.9);
  });

  it("spreads vertically between the first and the last", () => {
    const blocks = [block("a", 0), block("b", 0.1), block("c", 0.8)];
    const spread = distributeVertically(blocks, ["a", "b", "c"]);
    expect(spread.map((b) => b.y)).toEqual([0, 0.4, 0.8]);
  });

  it("swaps two neighbouring layers", () => {
    const blocks = [
      { ...block("a", 0), z: 0 },
      { ...block("b", 0.2), z: 1 },
    ];
    const raised = moveLayer(blocks, "a", 1);
    expect(raised.find((b) => b.id === "a")?.z).toBe(1);
    expect(raised.find((b) => b.id === "b")?.z).toBe(0);
  });

  it("duplicates with an offset and a new identifier", () => {
    const copy = duplicateBlock(block("a", 0.2));
    expect(copy.id).not.toBe("a");
    expect(copy.y).toBeCloseTo(0.22);
  });

  it("creates a text block already bound to the brand", () => {
    const b = newBlock("text", 0);
    expect(b.type).toBe("text");
    expect((b.props as { colorToken?: string }).colorToken).toBe("text");
  });
});

describe("brand", () => {
  const token = (key: string, hex: string): BrandToken => ({
    kind: "color",
    key,
    label: key,
    value: { hex },
    position: 0,
  });

  it("the group only overrides what it redefines", () => {
    const brand = toBrand([token("accent", "#ff0000")], [
      token("accent", "#000000"),
      token("background", "#ffffff"),
    ]);
    expect(brand.color?.accent).toEqual({ hex: "#ff0000" });
    expect(brand.color?.background).toEqual({ hex: "#ffffff" });
  });
});

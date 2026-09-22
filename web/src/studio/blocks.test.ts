import { describe, expect, it } from "vitest";
import type { Block } from "@backline/layout";
import { aligner, deplacerCalque, dupliquer, nouveauBloc, repartirVertical } from "./blocks";
import { toBrand } from "./brand";
import type { BrandToken } from "../lib/types";

const bloc = (id: string, y: number): Block => ({
  id,
  type: "text",
  x: 0.1,
  y,
  w: 0.4,
  h: 0.1,
  props: { content: id },
});

describe("manipulation des blocs", () => {
  it("centre un bloc sans calcul mental", () => {
    expect(aligner(bloc("a", 0), "centre-h").x).toBeCloseTo(0.3);
    expect(aligner(bloc("a", 0), "droite").x).toBeCloseTo(0.6);
    expect(aligner(bloc("a", 0), "bas").y).toBeCloseTo(0.9);
  });

  it("repartit verticalement entre le premier et le dernier", () => {
    const blocs = [bloc("a", 0), bloc("b", 0.1), bloc("c", 0.8)];
    const repartis = repartirVertical(blocs, ["a", "b", "c"]);
    expect(repartis.map((b) => b.y)).toEqual([0, 0.4, 0.8]);
  });

  it("echange deux calques voisins", () => {
    const blocs = [
      { ...bloc("a", 0), z: 0 },
      { ...bloc("b", 0.2), z: 1 },
    ];
    const monte = deplacerCalque(blocs, "a", 1);
    expect(monte.find((b) => b.id === "a")?.z).toBe(1);
    expect(monte.find((b) => b.id === "b")?.z).toBe(0);
  });

  it("duplique en decalant, avec un nouvel identifiant", () => {
    const copie = dupliquer(bloc("a", 0.2));
    expect(copie.id).not.toBe("a");
    expect(copie.y).toBeCloseTo(0.22);
  });

  it("cree un texte deja rattache a la charte", () => {
    const b = nouveauBloc("text", 0);
    expect(b.type).toBe("text");
    expect((b.props as { colorToken?: string }).colorToken).toBe("text");
  });
});

describe("charte", () => {
  const token = (key: string, hex: string): BrandToken => ({
    kind: "color",
    key,
    label: key,
    value: { hex },
    position: 0,
  });

  it("le groupe ne surcharge que ce qu'il redefinit", () => {
    const brand = toBrand([token("accent", "#ff0000")], [
      token("accent", "#000000"),
      token("background", "#ffffff"),
    ]);
    expect(brand.color?.accent).toEqual({ hex: "#ff0000" });
    expect(brand.color?.background).toEqual({ hex: "#ffffff" });
  });
});

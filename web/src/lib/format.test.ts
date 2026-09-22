import { describe, expect, it } from "vitest";
import { dateCourte, heure, joursAvant, poids, relatif } from "./format";

describe("mise en forme francaise", () => {
  it("ecrit les dates a la francaise", () => {
    expect(dateCourte("2026-06-12T20:00:00")).toBe("12/06/2026");
    expect(heure("2026-06-12T20:05:00")).toBe("20h05");
  });

  it("dit ce qui presse en clair", () => {
    const dans3jours = new Date(Date.now() + 3 * 86_400_000).toISOString();
    expect(relatif(dans3jours)).toBe("dans 3 jours");
    expect(joursAvant(dans3jours)).toBe(3);

    const hier = new Date(Date.now() - 26 * 3_600_000).toISOString();
    expect(relatif(hier)).toBe("il y a 1 jour");
  });

  it("donne un poids lisible", () => {
    expect(poids(512)).toBe("512 o");
    expect(poids(2048)).toBe("2 ko");
    expect(poids(5 * 1024 * 1024)).toBe("5.0 Mo");
  });
});

import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, api } from "./api";

function reponse(status: number, body: unknown) {
  return {
    status,
    ok: status < 400,
    text: async () => (body === undefined ? "" : JSON.stringify(body)),
  } as Response;
}

afterEach(() => vi.unstubAllGlobals());

describe("client de l'API", () => {
  it("remonte l'erreur telle que l'API la formule", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => reponse(403, { error: "reserve aux admins du collectif" })),
    );

    await expect(api.get("/api/collectives/1/members")).rejects.toThrow(
      "reserve aux admins du collectif",
    );
  });

  it("se rabat sur le code HTTP quand le corps ne dit rien", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => reponse(500, { detail: "boum" })));

    await expect(api.get("/api/me")).rejects.toMatchObject({ message: "Erreur 500" });
  });

  it("accepte une reponse vide", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => reponse(204, undefined)));
    await expect(api.del("/api/collectives/1/groups/2/members/3")).resolves.toBeUndefined();
  });

  it("expose le statut pour distinguer un 401", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => reponse(401, { error: "non connecte" })));
    const erreur = await api.get("/api/me").catch((e) => e);
    expect(erreur).toBeInstanceOf(ApiError);
    expect((erreur as ApiError).status).toBe(401);
  });
});

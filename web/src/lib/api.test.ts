import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, api } from "./api";

function response(status: number, body: unknown) {
  return {
    status,
    ok: status < 400,
    text: async () => (body === undefined ? "" : JSON.stringify(body)),
  } as Response;
}

afterEach(() => vi.unstubAllGlobals());

describe("api client", () => {
  it("surfaces the error exactly as the API words it", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => response(403, { error: "reserve aux admins du collectif" })),
    );

    await expect(api.get("/api/collectives/1/members")).rejects.toThrow(
      "reserve aux admins du collectif",
    );
  });

  it("falls back on the HTTP code when the body says nothing", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response(500, { detail: "boum" })));

    await expect(api.get("/api/me")).rejects.toMatchObject({ message: "Erreur 500" });
  });

  it("accepts an empty response", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response(204, undefined)));
    await expect(api.del("/api/collectives/1/groups/2/members/3")).resolves.toBeUndefined();
  });

  it("exposes the status so a 401 can be told apart", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => response(401, { error: "non connecte" })));
    const error = await api.get("/api/me").catch((e) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).status).toBe(401);
  });
});

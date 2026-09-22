/**
 * Client de l'API. Une seule facon de parler au serveur : les erreurs
 * remontent en francais, telles que l'API les formule.
 */

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: body === undefined ? {} : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (res.status === 204) return undefined as T;

  const text = await res.text();
  const data = text ? safeJson(text) : null;

  if (!res.ok) throw new ApiError(res.status, errorMessage(data, res.status));

  return data as T;
}

/** L'API renvoie `{ "error": "..." }` ; sinon on se rabat sur le code HTTP. */
function errorMessage(data: unknown, status: number): string {
  if (data && typeof data === "object" && "error" in data) {
    const value = (data as { error: unknown }).error;
    if (typeof value === "string" && value) return value;
  }
  return `Erreur ${status}`;
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

export const api = {
  get: <T>(path: string) => request<T>("GET", path),
  post: <T>(path: string, body?: unknown) => request<T>("POST", path, body ?? {}),
  put: <T>(path: string, body?: unknown) => request<T>("PUT", path, body ?? {}),
  patch: <T>(path: string, body?: unknown) => request<T>("PATCH", path, body ?? {}),
  del: <T>(path: string) => request<T>("DELETE", path),

  async upload<T>(path: string, form: FormData): Promise<T> {
    const res = await fetch(path, { method: "POST", credentials: "same-origin", body: form });
    const text = await res.text();
    const data = text ? safeJson(text) : null;
    if (!res.ok) throw new ApiError(res.status, errorMessage(data, res.status));
    return data as T;
  },
};

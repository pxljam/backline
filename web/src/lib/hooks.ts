import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "./api";

/**
 * Data loading, minimal and honest: we expose the loading state, the error
 * exactly as the API words it, and a reload.
 */
export function useResource<T>(path: string | null, deps: unknown[] = []) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(path !== null);
  const version = useRef(0);

  const reload = useCallback(async () => {
    if (!path) {
      setData(null);
      setLoading(false);
      return;
    }
    const mine = ++version.current;
    setLoading(true);
    try {
      const result = await api.get<T>(path);
      // A response arriving after a route change must not overwrite the one
      // that followed it.
      if (mine === version.current) {
        setData(result);
        setError(null);
      }
    } catch (e) {
      if (mine === version.current) {
        setError(e instanceof ApiError ? e.message : "Erreur reseau");
      }
    } finally {
      if (mine === version.current) setLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, ...deps]);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { data, error, loading, reload, setData };
}

/** A writing action: keeps the "in progress" state and a displayable error. */
export function useAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async <T>(fn: () => Promise<T>): Promise<T | null> => {
    setBusy(true);
    setError(null);
    try {
      return await fn();
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Erreur reseau");
      return null;
    } finally {
      setBusy(false);
    }
  }, []);

  return { run, busy, error, setError };
}

export function useLocalState<T>(key: string, initial: T): [T, (v: T) => void] {
  const [value, setValue] = useState<T>(() => {
    try {
      const raw = localStorage.getItem(key);
      return raw ? (JSON.parse(raw) as T) : initial;
    } catch {
      return initial;
    }
  });
  const set = useCallback(
    (v: T) => {
      setValue(v);
      try {
        localStorage.setItem(key, JSON.stringify(v));
      } catch {
        // Private browsing: carry on without remembering anything.
      }
    },
    [key],
  );
  return [value, set];
}

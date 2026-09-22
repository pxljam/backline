import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "./api";

/**
 * Chargement de donnees, version minimale et honnete : on expose l'etat de
 * chargement, l'erreur telle que l'API la formule, et un `recharger`.
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
      // Une reponse arrivee apres un changement de route ne doit pas ecraser
      // la suivante.
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

/** Action qui ecrit : garde l'etat « en cours » et l'erreur affichable. */
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
        // Mode navigation privee : on continue sans memoriser.
      }
    },
    [key],
  );
  return [value, set];
}

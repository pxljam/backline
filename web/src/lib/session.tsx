import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { api, ApiError } from "./api";
import type { Me } from "./types";

/**
 * Session et **collectif courant**. Un compte appartient a plusieurs
 * collectifs (§3) : basculer de l'un a l'autre est une operation de premier
 * plan, pas un reglage cache.
 */
interface SessionValue {
  me: Me | null;
  loading: boolean;
  collectiveId: string | null;
  setCollectiveId: (id: string) => void;
  role: "admin" | "member" | null;
  isAdmin: boolean;
  reload: () => Promise<void>;
  logout: () => Promise<void>;
}

const Ctx = createContext<SessionValue | null>(null);
const STORAGE_KEY = "backline.collective";

export const SessionProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [me, setMe] = useState<Me | null>(null);
  const [loading, setLoading] = useState(true);
  const [collectiveId, setCollective] = useState<string | null>(
    () => localStorage.getItem(STORAGE_KEY),
  );

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      const result = await api.get<Me>("/api/me");
      setMe(result);
      setCollective((current) => {
        const valid = current && result.collectives.some((c) => c.id === current);
        return valid ? current : (result.collectives[0]?.id ?? null);
      });
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) setMe(null);
      else setMe(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const setCollectiveId = useCallback((id: string) => {
    setCollective(id);
    localStorage.setItem(STORAGE_KEY, id);
  }, []);

  const logout = useCallback(async () => {
    await api.post("/api/auth/logout").catch(() => undefined);
    setMe(null);
  }, []);

  const value = useMemo<SessionValue>(() => {
    const role = me?.collectives.find((c) => c.id === collectiveId)?.role ?? null;
    return {
      me,
      loading,
      collectiveId,
      setCollectiveId,
      role,
      isAdmin: role === "admin",
      reload,
      logout,
    };
  }, [me, loading, collectiveId, setCollectiveId, reload, logout]);

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
};

export function useSession(): SessionValue {
  const value = useContext(Ctx);
  if (!value) throw new Error("useSession hors SessionProvider");
  return value;
}

/** Racine des routes du collectif courant. */
export function useCollectiveBase(): string {
  const { collectiveId } = useSession();
  return `/api/collectives/${collectiveId}`;
}

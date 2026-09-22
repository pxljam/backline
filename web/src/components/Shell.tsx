import React, { useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { useSession } from "../lib/session";
import { useResource } from "../lib/hooks";
import type { Dashboard } from "../lib/types";

/**
 * Coquille de l'application. Responsive : les admins travaillent aussi depuis
 * un telephone (§14).
 */

const LIENS: { to: string; label: string; adminOnly?: boolean }[] = [
  { to: "/", label: "Tableau de bord" },
  { to: "/opportunites", label: "Opportunites" },
  { to: "/evenements", label: "Evenements" },
  { to: "/calendrier", label: "Calendrier" },
  { to: "/groupes", label: "Groupes" },
  { to: "/membres", label: "Membres" },
  { to: "/lieux", label: "Lieux" },
  { to: "/studio", label: "Studio" },
  { to: "/rendus", label: "Rendus" },
  { to: "/charte", label: "Charte" },
  { to: "/reglages", label: "Reglages" },
];

export const Shell: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { me, collectiveId, setCollectiveId, logout, isAdmin } = useSession();
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const { data: dash } = useResource<Dashboard>(
    collectiveId ? `/api/collectives/${collectiveId}/dashboard` : null,
  );

  const nonLues = dash?.unread_notifications ?? 0;

  return (
    <div className="flex min-h-full flex-col lg:flex-row">
      <header className="flex items-center justify-between gap-3 border-b border-line bg-panel px-4 py-3 lg:hidden">
        <button
          onClick={() => setOpen((v) => !v)}
          className="rounded-lg border border-line px-3 py-1.5 text-sm"
          aria-label="Menu"
        >
          ☰
        </button>
        <span className="font-semibold tracking-[0.2em]">BCKLN</span>
        <NavLink to="/notifications" className="relative text-sm">
          🔔
          {nonLues > 0 && (
            <span className="absolute -right-2 -top-1 rounded-full bg-accent px-1.5 text-[10px] text-white">
              {nonLues}
            </span>
          )}
        </NavLink>
      </header>

      <nav
        className={`${open ? "block" : "hidden"} border-b border-line bg-panel lg:block lg:w-60 lg:shrink-0 lg:border-b-0 lg:border-r`}
      >
        <div className="hidden px-5 py-5 lg:block">
          <span className="text-lg font-semibold tracking-[0.25em]">BCKLN</span>
          <p className="mt-0.5 text-xs text-ink-soft">Backline</p>
        </div>

        <div className="px-3 pb-2 pt-3 lg:pt-0">
          <select
            value={collectiveId ?? ""}
            onChange={(e) => {
              setCollectiveId(e.target.value);
              navigate("/");
            }}
            className="w-full rounded-lg border border-line bg-panel px-2.5 py-2 text-sm"
          >
            {me?.collectives.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </div>

        <ul className="space-y-0.5 px-3 pb-4">
          {LIENS.filter((l) => !l.adminOnly || isAdmin).map((l) => (
            <li key={l.to}>
              <NavLink
                to={l.to}
                end={l.to === "/"}
                onClick={() => setOpen(false)}
                className={({ isActive }) =>
                  `block rounded-lg px-3 py-2 text-sm ${
                    isActive ? "bg-ink text-paper" : "text-ink-soft hover:bg-paper"
                  }`
                }
              >
                {l.label}
              </NavLink>
            </li>
          ))}
          <li>
            <NavLink
              to="/notifications"
              onClick={() => setOpen(false)}
              className={({ isActive }) =>
                `flex items-center justify-between rounded-lg px-3 py-2 text-sm ${
                  isActive ? "bg-ink text-paper" : "text-ink-soft hover:bg-paper"
                }`
              }
            >
              Notifications
              {nonLues > 0 && (
                <span className="rounded-full bg-accent px-1.5 text-[10px] text-white">
                  {nonLues}
                </span>
              )}
            </NavLink>
          </li>
          {me?.is_instance_admin && (
            <li>
              <NavLink
                to="/instance"
                onClick={() => setOpen(false)}
                className={({ isActive }) =>
                  `block rounded-lg px-3 py-2 text-sm ${
                    isActive ? "bg-ink text-paper" : "text-ink-soft hover:bg-paper"
                  }`
                }
              >
                Instance
              </NavLink>
            </li>
          )}
        </ul>

        <div className="border-t border-line px-4 py-3 text-xs text-ink-soft">
          <p className="font-medium text-ink">{me?.display_name}</p>
          <p>{isAdmin ? "admin du collectif" : "membre"}</p>
          <button
            onClick={async () => {
              await logout();
              navigate("/connexion");
            }}
            className="mt-2 underline"
          >
            se deconnecter
          </button>
        </div>
      </nav>

      <main className="min-w-0 flex-1 px-4 py-6 lg:px-8 lg:py-8">{children}</main>
    </div>
  );
};

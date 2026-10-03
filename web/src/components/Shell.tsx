import React, { useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { useSession } from "../lib/session";
import { useResource } from "../lib/hooks";
import type { Dashboard } from "../lib/types";
import { Icon, type IconName } from "./icons";
import { Badge, Select, cx } from "./ui";

/**
 * Application shell. Responsive: admins work from a phone too (§14).
 *
 * The active item is a raised slab with an accent bar rather than a filled
 * block: on a dark chrome a solid fill shouts, and the only thing allowed to
 * shout is the thing you should act on.
 *
 * `tourId` anchors the walkthrough to this list instead of letting it hunt the
 * DOM for a label — copy changes, anchors should not.
 */

const LIENS: {
  to: string;
  label: string;
  icon: IconName;
  tourId?: string;
  adminOnly?: boolean;
}[] = [
  { to: "/", label: "Tableau de bord", icon: "dashboard", tourId: "dashboard" },
  { to: "/opportunites", label: "Opportunités", icon: "opportunity", tourId: "opportunites" },
  { to: "/evenements", label: "Événements", icon: "event", tourId: "evenements" },
  { to: "/calendrier", label: "Calendrier", icon: "calendar" },
  { to: "/groupes", label: "Groupes", icon: "group", tourId: "groupes" },
  { to: "/membres", label: "Membres", icon: "member", tourId: "membres" },
  { to: "/lieux", label: "Lieux", icon: "venue" },
  { to: "/studio", label: "Studio", icon: "studio", tourId: "studio" },
  { to: "/rendus", label: "Rendus", icon: "render" },
  { to: "/charte", label: "Charte", icon: "brand", tourId: "charte" },
  { to: "/reglages", label: "Réglages", icon: "settings" },
];

const link = ({ isActive }: { isActive: boolean }) =>
  cx(
    "relative flex min-h-11 items-center gap-2.5 rounded-control px-3 py-2 text-sm transition-colors duration-200",
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
    isActive
      ? "bg-raised font-medium text-ink before:absolute before:inset-y-1.5 before:-left-0.5 before:w-0.5 before:rounded-full before:bg-accent"
      : "text-ink-soft hover:bg-raised hover:text-ink",
  );

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
      {/* Eleven links stand between a keyboard and the page on every screen. */}
      <a
        href="#contenu"
        className="sr-only focus:not-sr-only focus:fixed focus:left-3 focus:top-3 focus:z-50 focus:rounded-control focus:bg-accent focus:px-3 focus:py-2 focus:text-sm focus:font-medium focus:text-on-accent"
      >
        Aller au contenu
      </a>
      <header className="sticky top-0 z-10 flex items-center justify-between gap-3 border-b border-line bg-stage/80 px-4 py-3 backdrop-blur lg:hidden">
        <button
          onClick={() => setOpen((v) => !v)}
          className="flex min-h-11 items-center rounded-control border border-line-strong px-3 text-sm"
          aria-label="Menu"
          aria-expanded={open}
        >
          <Icon name="menu" className="h-5 w-5" />
        </button>
        <span className="font-display font-semibold tracking-[0.2em]">
          BCKLN<span className="text-accent">_</span>
        </span>
        <NavLink
          to="/notifications"
          className="relative flex min-h-11 items-center px-2 text-ink-soft"
          aria-label="Notifications"
        >
          <Icon name="bell" className="h-5 w-5" />
          {nonLues > 0 && (
            <span className="tabular absolute right-0 top-1.5 rounded-[0.25rem] bg-accent px-1 text-[10px] font-medium text-on-accent">
              {nonLues}
            </span>
          )}
        </NavLink>
      </header>

      <nav
        className={cx(
          open ? "block" : "hidden",
          "border-b border-line bg-panel lg:block lg:w-60 lg:shrink-0 lg:border-b-0 lg:border-r",
        )}
      >
        <div className="hidden px-5 py-5 lg:block">
          <span className="font-display text-lg font-semibold tracking-[0.25em]">
            BCKLN<span className="text-accent">_</span>
          </span>
          <p className="mt-0.5 text-xs text-ink-soft">Backline</p>
        </div>

        <div className="px-3 pb-2 pt-3 lg:pt-0">
          <Select
            aria-label="Collectif courant"
            value={collectiveId ?? ""}
            onChange={(e) => {
              setCollectiveId(e.target.value);
              navigate("/");
            }}
          >
            {me?.collectives.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </Select>
        </div>

        <ul className="space-y-0.5 px-3 pb-4">
          {LIENS.filter((l) => !l.adminOnly || isAdmin).map((l) => (
            <li key={l.to}>
              <NavLink
                to={l.to}
                end={l.to === "/"}
                data-tour={l.tourId}
                onClick={() => setOpen(false)}
                className={link}
              >
                <Icon name={l.icon} />
                {l.label}
              </NavLink>
            </li>
          ))}
          <li>
            <NavLink
              to="/notifications"
              onClick={() => setOpen(false)}
              className={(state) => cx(link(state), "justify-between")}
            >
              <span className="flex items-center gap-2.5">
                <Icon name="bell" />
                Notifications
              </span>
              {nonLues > 0 && (
                <span className="tabular rounded-[0.25rem] bg-accent px-1 text-[10px] font-medium text-on-accent">
                  {nonLues}
                </span>
              )}
            </NavLink>
          </li>
          {me?.is_instance_admin && (
            <li>
              <NavLink to="/instance" onClick={() => setOpen(false)} className={link}>
                <Icon name="instance" />
                Instance
              </NavLink>
            </li>
          )}
        </ul>

        <div className="border-t border-line px-4 py-3 text-xs text-ink-soft">
          <p className="font-medium text-ink">{me?.display_name}</p>
          <p className="flex flex-wrap items-center gap-1.5">
            {me?.is_instance_admin && <Badge tone="accent">super admin</Badge>}
            <span>{isAdmin ? "admin du collectif" : "membre"}</span>
          </p>
          <button
            onClick={async () => {
              await logout();
              navigate("/connexion");
            }}
            className="mt-2 rounded px-0.5 underline hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            se déconnecter
          </button>
        </div>
      </nav>

      {/* Capped at 1440px: past that, a list row is a line the eye cannot
          follow back. The editors fit comfortably inside it. */}
      <main id="contenu" tabIndex={-1} className="min-w-0 flex-1 px-4 pb-10 pt-6 outline-none lg:px-8 lg:pb-12 lg:pt-8">
        <div className="mx-auto w-full max-w-[90rem]">{children}</div>
      </main>
    </div>
  );
};

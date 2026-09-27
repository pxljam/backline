import React from "react";
import { Link } from "react-router-dom";
import type { Dashboard } from "../lib/types";
import { useLocalState } from "../lib/hooks";
import { Button, Card, cx } from "./ui";
import { Icon } from "./icons";

/**
 * First steps, on the dashboard of a collective that has barely started.
 *
 * Completion is **derived from the data**, never stored: the list is therefore
 * always true, survives a change of device, and comes back on its own if the
 * last group is deleted. The only thing worth remembering is that someone
 * dismissed it, and that is a per-device convenience.
 *
 * Each line says what the step unblocks, never where to click — §9.1: the tool
 * makes the right thing easy rather than explaining it.
 */

export interface Step {
  key: string;
  label: string;
  why: string;
  to: string;
  done: boolean;
}

/** Pure, so the branching is testable without rendering anything. */
export function computeSteps(data: Dashboard, isAdmin: boolean): Step[] {
  const setup = data.setup;
  if (!setup) return [];

  if (!isAdmin) {
    // A member inherits none of an admin's setup chores. They only ever see
    // what is actually waiting for them, and nothing when nothing is.
    return [
      {
        key: "poll",
        label: "Réponds aux sondages de dispos",
        why: "Sans ta réponse, la date ne peut pas être tranchée.",
        to: "/opportunites",
        done: !data.pending_polls.some((p) => p.mine_missing),
      },
      {
        key: "slot",
        label: "Prends un poste",
        why: "Transport, régie, photo : un concert tient à qui s'en occupe.",
        to: "/evenements",
        done: data.vacant_slots.length === 0,
      },
      {
        key: "calendar",
        label: "Branche ton agenda",
        why: "Le flux iCal se met à jour tout seul, dans ton téléphone.",
        to: "/reglages",
        done: false,
      },
    ].filter((s) => !s.done || s.key === "calendar");
  }

  return [
    {
      key: "members",
      label: "Invite tes membres",
      why: "Chaque personne reçoit un lien à usage unique. Il n'y a pas d'inscription libre.",
      to: "/membres",
      done: setup.members > 1,
    },
    {
      key: "groups",
      label: "Crée un groupe",
      why: "Un groupe, c'est un projet qui joue : sa charte, ses comptes, sa fiche technique.",
      to: "/groupes",
      done: setup.groups > 0,
    },
    {
      key: "venues",
      label: "Ajoute un lieu",
      why: "Le carnet d'adresses : contacts, conditions, historique.",
      to: "/lieux",
      done: setup.venues > 0,
    },
    {
      key: "opportunities",
      label: "Ouvre une opportunité",
      why: "Une date possible. Le sondage de dispos tranche à ta place.",
      to: "/opportunites",
      done: setup.opportunities > 0,
    },
    {
      key: "brand",
      label: "Règle ta charte graphique",
      why: "Couleurs, polices, logos. Le Studio ne pourra plus en sortir par accident.",
      to: "/charte",
      done: setup.brand_has_logo,
    },
    {
      key: "templates",
      label: "Crée un gabarit",
      why: "Une affiche, une story, un post : une seule mise en page, déclinée par format.",
      to: "/studio",
      done: setup.templates > 0,
    },
    {
      key: "render",
      label: "Connecte une machine de rendu",
      why: "Sans machine, les vidéos attendent. Les visuels fixes sortent quand même.",
      to: "/rendus",
      done: data.render_machines_online > 0,
    },
  ];
}

export const FirstSteps: React.FC<{ data: Dashboard; isAdmin: boolean; collectiveId: string }> = ({
  data,
  isAdmin,
  collectiveId,
}) => {
  const [dismissed, setDismissed] = useLocalState<boolean>(
    `backline.onboarding.dismissed.${collectiveId}`,
    false,
  );

  const steps = computeSteps(data, isAdmin);
  const done = steps.filter((s) => s.done).length;

  // Once everything is done the card has nothing left to say, and it goes
  // without anyone having to close it.
  if (dismissed || steps.length === 0 || done === steps.length) return null;

  return (
    <Card
      tone="accent"
      className="mb-4"
      title={
        <span className="flex items-center gap-2">
          <Icon name="spark" className="h-3.5 w-3.5 text-accent" />
          Premiers pas
        </span>
      }
      action={
        <Button size="sm" onClick={() => setDismissed(true)}>
          Masquer
        </Button>
      }
    >
      <p className="mb-3 text-sm text-ink-soft">
        <span className="tabular font-medium text-ink">
          {done}/{steps.length}
        </span>{" "}
        — de quoi rendre ce collectif utilisable au quotidien.
      </p>
      <ul className="space-y-1">
        {steps.map((s) => (
          <li key={s.key}>
            <Link
              to={s.to}
              className={cx(
                "-mx-2 flex items-start gap-3 rounded-control px-2 py-2 transition",
                "hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
                s.done && "opacity-55",
              )}
            >
              <span
                className={cx(
                  "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border",
                  s.done ? "border-accent bg-accent text-on-accent" : "border-line-strong",
                )}
              >
                {s.done && <Icon name="check" className="h-3 w-3" />}
              </span>
              <span className="min-w-0">
                <span className={cx("block text-sm", s.done ? "line-through" : "font-medium")}>
                  {s.label}
                </span>
                <span className="block text-xs text-ink-faint">{s.why}</span>
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </Card>
  );
};

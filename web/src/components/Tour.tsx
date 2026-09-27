import React from "react";
import { Button, cx } from "./ui";
import { Icon } from "./icons";

/**
 * The guided walkthrough — on demand, never imposed.
 *
 * §9.1 says the tool should make the right thing structurally easy rather than
 * explain it, which is why the empty states and the first-steps card come
 * first and this comes last: a tour is what you reach for when you already
 * know the map and want it read aloud once.
 *
 * Anchored on the `data-tour` attributes carried by the navigation (see
 * `Shell.tsx`), never on label text — copy changes, anchors should not. On a
 * phone the navigation is folded behind ☰, so the spotlight is skipped and the
 * steps simply read as a list.
 */

interface Step {
  tourId: string;
  title: string;
  body: string;
}

const STEPS: Step[] = [
  {
    tourId: "dashboard",
    title: "Le tableau de bord ne raconte pas tout",
    body: "Il liste ce qui bloque : un sondage sans réponse, un poste vacant, une publication en retard. Quand il est vide, c'est une bonne nouvelle.",
  },
  {
    tourId: "opportunites",
    title: "Une opportunité, c'est une date possible",
    body: "Tu proposes plusieurs dates, les membres disent leurs dispos, et la matrice tranche à ta place. Confirmer crée l'événement d'un coup : postes, plan de com, fiches techniques.",
  },
  {
    tourId: "evenements",
    title: "L'événement tient tout ensemble",
    body: "Line-up, logistique, plan de com, feuille de route du jour J. Le téléphone d'une personne n'apparaît qu'à ceux qui jouent avec elle.",
  },
  {
    tourId: "groupes",
    title: "Un groupe est un projet qui joue",
    body: "Sa charte locale surcharge celle du collectif, ses comptes sociaux disent qui a l'accès, sa fiche technique part en PDF au lieu.",
  },
  {
    tourId: "membres",
    title: "On n'ouvre pas de compte tout seul",
    body: "Un admin crée la personne et lui envoie un lien à usage unique. Le lien est valable trente jours, et en générer un nouveau tue le précédent.",
  },
  {
    tourId: "charte",
    title: "La charte est une donnée, pas du code",
    body: "Couleurs, polices, logos, règles de placement. Une fois réglée, le Studio ne peut plus en sortir par accident.",
  },
  {
    tourId: "studio",
    title: "Le Studio remplit, il ne conçoit pas",
    body: "Un gabarit porte ses déclinaisons par format : une affiche, une story, un post. Les vidéos partent sur une machine connectée — le serveur n'encode jamais.",
  },
];

/** The rectangle of the anchored element, or null when it is not on screen. */
function useSpotlight(tourId: string | null): DOMRect | null {
  const [rect, setRect] = React.useState<DOMRect | null>(null);

  React.useEffect(() => {
    if (!tourId) return setRect(null);
    const read = () => {
      const el = document.querySelector(`[data-tour="${tourId}"]`);
      setRect(el ? el.getBoundingClientRect() : null);
    };
    read();
    window.addEventListener("resize", read);
    window.addEventListener("scroll", read, true);
    return () => {
      window.removeEventListener("resize", read);
      window.removeEventListener("scroll", read, true);
    };
  }, [tourId]);

  return rect;
}

export const Tour: React.FC<{ open: boolean; onClose: () => void }> = ({ open, onClose }) => {
  const [i, setI] = React.useState(0);
  const step = STEPS[i];
  const rect = useSpotlight(open ? step.tourId : null);
  const card = React.useRef<HTMLDivElement>(null);

  // The card takes focus so Escape reaches it and a keyboard lands inside.
  React.useEffect(() => {
    if (open) card.current?.focus();
  }, [open, i]);

  React.useEffect(() => {
    if (open) setI(0);
  }, [open]);

  if (!open) return null;

  const close = () => {
    setI(0);
    onClose();
  };

  // Deliberately not the `<dialog>` used elsewhere: `showModal()` puts the card
  // in the top layer, above everything — including the very element the
  // spotlight is trying to point at. So the card is an ordinary fixed panel,
  // the dim comes from the spotlight's own shadow, and Escape is handled here.
  return (
    <div
      role="dialog"
      aria-modal="false"
      aria-label={`Visite guidée, étape ${i + 1} sur ${STEPS.length}`}
      onKeyDown={(e) => e.key === "Escape" && close()}
    >
      {rect ? (
        <div
          aria-hidden
          className="pointer-events-none fixed z-40 rounded-control ring-2 ring-accent transition-all duration-200"
          style={{
            left: rect.left - 4,
            top: rect.top - 4,
            width: rect.width + 8,
            height: rect.height + 8,
            boxShadow: "0 0 0 9999px rgb(0 0 0 / 0.55)",
          }}
        />
      ) : (
        // Nothing to point at — on a phone the navigation is folded away — so
        // the dim is drawn on its own and the steps simply read as text.
        <div aria-hidden className="pointer-events-none fixed inset-0 z-40 bg-black/55" />
      )}

      <div
        ref={card}
        tabIndex={-1}
        className={cx(
          "fixed z-50 w-[min(26rem,calc(100vw-2rem))] rounded-panel border border-line bg-panel p-4",
          "shadow-[var(--shadow-panel)] focus:outline-none",
          // Kept clear of the navigation, which is what the spotlight points at.
          "left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 lg:left-auto lg:right-10 lg:translate-x-0",
        )}
      >
        <div className="mb-3 flex items-center justify-between gap-3">
          <span className="tabular font-display text-xs font-semibold uppercase tracking-[0.12em] text-ink-soft">
            Visite {i + 1}/{STEPS.length}
          </span>
          <Button size="sm" onClick={close} aria-label="Fermer la visite">
            <Icon name="close" />
          </Button>
        </div>

        <h3 className="font-display text-base font-semibold">{step.title}</h3>
        <p className="mt-2 text-sm text-ink-soft">{step.body}</p>

        <div className="mt-5 flex items-center justify-between gap-3">
          <div className="flex gap-1" aria-hidden>
            {STEPS.map((s, n) => (
              <span
                key={s.tourId}
                className={cx("h-1 w-5 rounded-full transition", n === i ? "bg-accent" : "bg-line")}
              />
            ))}
          </div>
          <div className="flex gap-2">
            {i > 0 && (
              <Button size="sm" onClick={() => setI(i - 1)}>
                Précédent
              </Button>
            )}
            {i < STEPS.length - 1 ? (
              <Button size="sm" variant="primary" onClick={() => setI(i + 1)}>
                Suivant
                <Icon name="arrow" />
              </Button>
            ) : (
              <Button size="sm" variant="primary" onClick={close}>
                J'ai compris
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

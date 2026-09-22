import React, { useCallback, useEffect, useRef, useState } from "react";
import { Player } from "@remotion/player";
import type { Block, Brand, FieldData, Layout, MediaMap } from "@backline/layout";
import { Still, safeArea } from "@backline/layout";

/**
 * Le canevas de l'editeur (§9.1) : **une souris, pas de code**.
 *
 * Le rendu affiche est exactement celui du moteur — le meme composant que le
 * service de visuels fixes et la CLI video. L'editeur ne dessine que ses
 * poignees par-dessus : ce que voit l'admin est ce qui sort.
 */

const PAS_GRILLE = 1 / 24;
const TOLERANCE = 0.008;
const MIN = 0.02;

type Poignee = "nw" | "ne" | "sw" | "se";
type Mode = "move" | Poignee | "rotate";

interface Geste {
  mode: Mode;
  id: string;
  depart: Block;
  x0: number;
  y0: number;
  gauche: number;
  haut: number;
  largeur: number;
  hauteur: number;
}

interface Guide {
  axe: "x" | "y";
  valeur: number;
}

export interface CanvasProps {
  layout: Layout;
  brand: Brand;
  data: FieldData;
  media: MediaMap;
  selection: string | null;
  onSelect: (id: string | null) => void;
  onBlocks: (blocks: Block[]) => void;
  magnetisme: boolean;
  zonesSures: boolean;
  lectureSeule?: boolean;
}

export const Canvas: React.FC<CanvasProps> = ({
  layout,
  brand,
  data,
  media,
  selection,
  onSelect,
  onBlocks,
  magnetisme,
  zonesSures,
  lectureSeule = false,
}) => {
  const cadre = useRef<HTMLDivElement>(null);
  const geste = useRef<Geste | null>(null);
  const [guides, setGuides] = useState<Guide[]>([]);

  const blocs = [...layout.blocks].sort((a, b) => (a.z ?? 0) - (b.z ?? 0));

  const remplacer = useCallback(
    (id: string, patch: Partial<Block>) =>
      onBlocks(layout.blocks.map((b) => (b.id === id ? { ...b, ...patch } : b))),
    [layout.blocks, onBlocks],
  );

  // --- Magnetisme et guides ------------------------------------------------

  const accrocher = useCallback(
    (bloc: Block, id: string): { bloc: Block; guides: Guide[] } => {
      if (!magnetisme) return { bloc, guides: [] };

      const autres = layout.blocks.filter((b) => b.id !== id);
      const cibles = (axe: "x" | "y") => {
        const valeurs = [0, 0.5, 1];
        for (const b of autres) {
          const debut = axe === "x" ? b.x : b.y;
          const taille = axe === "x" ? b.w : b.h;
          valeurs.push(debut, debut + taille / 2, debut + taille);
        }
        return valeurs;
      };

      const trouves: Guide[] = [];
      const ajuste = { ...bloc };

      for (const axe of ["x", "y"] as const) {
        const debut = axe === "x" ? ajuste.x : ajuste.y;
        const taille = axe === "x" ? ajuste.w : ajuste.h;
        const points = [debut, debut + taille / 2, debut + taille];
        let meilleur: { delta: number; valeur: number } | null = null;
        for (const position of points) {
          for (const cible of cibles(axe)) {
            const delta = cible - position;
            if (
              Math.abs(delta) <= TOLERANCE &&
              (!meilleur || Math.abs(delta) < Math.abs(meilleur.delta))
            ) {
              meilleur = { delta, valeur: cible };
            }
          }
        }
        if (meilleur) {
          if (axe === "x") ajuste.x += meilleur.delta;
          else ajuste.y += meilleur.delta;
          trouves.push({ axe, valeur: meilleur.valeur });
        } else {
          // A defaut d'un autre bloc, la grille.
          const arrondi = Math.round(debut / PAS_GRILLE) * PAS_GRILLE;
          if (Math.abs(arrondi - debut) <= TOLERANCE) {
            if (axe === "x") ajuste.x = arrondi;
            else ajuste.y = arrondi;
          }
        }
      }

      return { bloc: ajuste, guides: trouves };
    },
    [layout.blocks, magnetisme],
  );

  // --- Gestes ---------------------------------------------------------------

  const demarrer = (e: React.PointerEvent, bloc: Block, mode: Mode) => {
    if (lectureSeule || bloc.locked) return;
    e.stopPropagation();
    e.preventDefault();
    const rect = cadre.current?.getBoundingClientRect();
    if (!rect) return;
    geste.current = {
      mode,
      id: bloc.id,
      depart: bloc,
      x0: e.clientX,
      y0: e.clientY,
      gauche: rect.left,
      haut: rect.top,
      largeur: rect.width,
      hauteur: rect.height,
    };
    onSelect(bloc.id);
    (e.target as Element).setPointerCapture?.(e.pointerId);
  };

  useEffect(() => {
    const bouger = (e: PointerEvent) => {
      const g = geste.current;
      if (!g) return;
      const dx = (e.clientX - g.x0) / g.largeur;
      const dy = (e.clientY - g.y0) / g.hauteur;
      const d = g.depart;

      if (g.mode === "rotate") {
        // Angle entre le centre du bloc et le pointeur. La poignee est au-dessus
        // du centre : 0 degre doit donc correspondre a « vers le haut ».
        const centreX = g.gauche + (d.x + d.w / 2) * g.largeur;
        const centreY = g.haut + (d.y + d.h / 2) * g.hauteur;
        const angle = (Math.atan2(e.clientY - centreY, e.clientX - centreX) * 180) / Math.PI + 90;
        const arrondi = e.shiftKey ? Math.round(angle / 15) * 15 : Math.round(angle);
        remplacer(g.id, { rotation: arrondi });
        return;
      }

      let suivant: Block;
      switch (g.mode) {
        case "move":
          suivant = { ...d, x: d.x + dx, y: d.y + dy };
          break;
        case "se":
          suivant = { ...d, w: Math.max(MIN, d.w + dx), h: Math.max(MIN, d.h + dy) };
          break;
        case "sw":
          suivant = {
            ...d,
            x: Math.min(d.x + d.w - MIN, d.x + dx),
            w: Math.max(MIN, d.w - dx),
            h: Math.max(MIN, d.h + dy),
          };
          break;
        case "ne":
          suivant = {
            ...d,
            y: Math.min(d.y + d.h - MIN, d.y + dy),
            w: Math.max(MIN, d.w + dx),
            h: Math.max(MIN, d.h - dy),
          };
          break;
        case "nw":
          suivant = {
            ...d,
            x: Math.min(d.x + d.w - MIN, d.x + dx),
            y: Math.min(d.y + d.h - MIN, d.y + dy),
            w: Math.max(MIN, d.w - dx),
            h: Math.max(MIN, d.h - dy),
          };
          break;
        default:
          return;
      }

      const { bloc, guides: trouves } = accrocher(suivant, g.id);
      setGuides(trouves);
      remplacer(g.id, bloc);
    };

    const finir = () => {
      geste.current = null;
      setGuides([]);
    };

    window.addEventListener("pointermove", bouger);
    window.addEventListener("pointerup", finir);
    return () => {
      window.removeEventListener("pointermove", bouger);
      window.removeEventListener("pointerup", finir);
    };
  }, [accrocher, remplacer]);

  // --- Clavier : deplacement fin et suppression -----------------------------

  useEffect(() => {
    const touche = (e: KeyboardEvent) => {
      if (lectureSeule || !selection) return;
      const actif = document.activeElement?.tagName;
      if (actif === "INPUT" || actif === "TEXTAREA" || actif === "SELECT") return;

      const bloc = layout.blocks.find((b) => b.id === selection);
      if (!bloc || bloc.locked) return;
      const pas = e.shiftKey ? 0.05 : 0.005;

      switch (e.key) {
        case "ArrowLeft":
          remplacer(bloc.id, { x: bloc.x - pas });
          break;
        case "ArrowRight":
          remplacer(bloc.id, { x: bloc.x + pas });
          break;
        case "ArrowUp":
          remplacer(bloc.id, { y: bloc.y - pas });
          break;
        case "ArrowDown":
          remplacer(bloc.id, { y: bloc.y + pas });
          break;
        case "Backspace":
        case "Delete":
          onBlocks(layout.blocks.filter((b) => b.id !== selection));
          onSelect(null);
          break;
        case "Escape":
          onSelect(null);
          return;
        default:
          return;
      }
      e.preventDefault();
    };

    window.addEventListener("keydown", touche);
    return () => window.removeEventListener("keydown", touche);
  }, [layout.blocks, lectureSeule, onBlocks, onSelect, remplacer, selection]);

  const zone = safeArea(brand, layout.width, layout.height);

  return (
    <div
      ref={cadre}
      className="relative w-full select-none overflow-hidden rounded-lg border border-line bg-[repeating-conic-gradient(#e9e6e1_0%_25%,#f7f5f2_0%_50%)] bg-[length:16px_16px]"
      style={{ aspectRatio: `${layout.width} / ${layout.height}` }}
      onPointerDown={() => onSelect(null)}
    >
      <Player
        component={Still}
        inputProps={{ layout, brand, data, media }}
        durationInFrames={1}
        fps={layout.fps ?? 30}
        compositionWidth={layout.width}
        compositionHeight={layout.height}
        style={{ width: "100%", height: "100%", pointerEvents: "none" }}
        controls={false}
        clickToPlay={false}
        doubleClickToFullscreen={false}
        spaceKeyToPlayOrPause={false}
      />

      {zonesSures && (
        <div className="pointer-events-none absolute inset-0">
          <div
            className="absolute border-2 border-dashed border-sky-500/40"
            style={{
              left: `${zone.x * 100}%`,
              right: `${zone.x * 100}%`,
              top: `${zone.top * 100}%`,
              bottom: `${zone.bottom * 100}%`,
            }}
          />
        </div>
      )}

      {guides.map((g, i) => (
        <div
          key={i}
          className="pointer-events-none absolute bg-fuchsia-500"
          style={
            g.axe === "x"
              ? { left: `${g.valeur * 100}%`, top: 0, bottom: 0, width: 1 }
              : { top: `${g.valeur * 100}%`, left: 0, right: 0, height: 1 }
          }
        />
      ))}

      {blocs.map((bloc) => {
        const actif = bloc.id === selection;
        return (
          <div
            key={bloc.id}
            onPointerDown={(e) => demarrer(e, bloc, "move")}
            className={`absolute ${bloc.locked ? "cursor-not-allowed" : "cursor-move"} ${
              actif ? "outline outline-2 outline-ink" : "hover:outline hover:outline-1 hover:outline-ink/40"
            }`}
            style={{
              left: `${bloc.x * 100}%`,
              top: `${bloc.y * 100}%`,
              width: `${bloc.w * 100}%`,
              height: `${bloc.h * 100}%`,
              transform: `rotate(${bloc.rotation ?? 0}deg)`,
            }}
          >
            {actif && !lectureSeule && !bloc.locked && (
              <>
                {(["nw", "ne", "sw", "se"] as Poignee[]).map((p) => (
                  <span
                    key={p}
                    onPointerDown={(e) => demarrer(e, bloc, p)}
                    className="absolute h-3 w-3 rounded-sm border border-ink bg-paper"
                    style={{
                      left: p.endsWith("w") ? -6 : undefined,
                      right: p.endsWith("e") ? -6 : undefined,
                      top: p.startsWith("n") ? -6 : undefined,
                      bottom: p.startsWith("s") ? -6 : undefined,
                      cursor: p === "nw" || p === "se" ? "nwse-resize" : "nesw-resize",
                    }}
                  />
                ))}
                <span
                  onPointerDown={(e) => demarrer(e, bloc, "rotate")}
                  className="absolute left-1/2 h-3 w-3 -translate-x-1/2 cursor-grab rounded-full border border-ink bg-paper"
                  style={{ top: -22 }}
                  title="Rotation"
                />
              </>
            )}
          </div>
        );
      })}
    </div>
  );
};

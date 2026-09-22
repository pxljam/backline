import React, { useCallback, useEffect, useRef, useState } from "react";
import { Player } from "@remotion/player";
import type { Block, Brand, FieldData, Layout, MediaMap } from "@backline/layout";
import { Still, safeArea } from "@backline/layout";

/**
 * The editor canvas (§9.1): **a mouse, not code**.
 *
 * What it shows is exactly what the engine renders — the same component as the
 * still visuals service and the video CLI. The editor only draws its handles on
 * top: what the admin sees is what comes out.
 */

const GRID_STEP = 1 / 24;
const TOLERANCE = 0.008;
const MIN = 0.02;

type Handle = "nw" | "ne" | "sw" | "se";
type Mode = "move" | Handle | "rotate";

interface Gesture {
  mode: Mode;
  id: string;
  start: Block;
  x0: number;
  y0: number;
  left: number;
  top: number;
  width: number;
  height: number;
}

interface Guide {
  axis: "x" | "y";
  value: number;
}

export interface CanvasProps {
  layout: Layout;
  brand: Brand;
  data: FieldData;
  media: MediaMap;
  selection: string | null;
  onSelect: (id: string | null) => void;
  onBlocks: (blocks: Block[]) => void;
  snap: boolean;
  safeAreas: boolean;
  readOnly?: boolean;
}

export const Canvas: React.FC<CanvasProps> = ({
  layout,
  brand,
  data,
  media,
  selection,
  onSelect,
  onBlocks,
  snap,
  safeAreas,
  readOnly = false,
}) => {
  const frame = useRef<HTMLDivElement>(null);
  const gesture = useRef<Gesture | null>(null);
  const [guides, setGuides] = useState<Guide[]>([]);

  const blocks = [...layout.blocks].sort((a, b) => (a.z ?? 0) - (b.z ?? 0));

  const replace = useCallback(
    (id: string, patch: Partial<Block>) =>
      onBlocks(layout.blocks.map((b) => (b.id === id ? { ...b, ...patch } : b))),
    [layout.blocks, onBlocks],
  );

  // --- Snapping and guides -------------------------------------------------

  const snapTo = useCallback(
    (block: Block, id: string): { block: Block; guides: Guide[] } => {
      if (!snap) return { block, guides: [] };

      const others = layout.blocks.filter((b) => b.id !== id);
      const targets = (axis: "x" | "y") => {
        const values = [0, 0.5, 1];
        for (const b of others) {
          const start = axis === "x" ? b.x : b.y;
          const size = axis === "x" ? b.w : b.h;
          values.push(start, start + size / 2, start + size);
        }
        return values;
      };

      const found: Guide[] = [];
      const adjusted = { ...block };

      for (const axis of ["x", "y"] as const) {
        const start = axis === "x" ? adjusted.x : adjusted.y;
        const size = axis === "x" ? adjusted.w : adjusted.h;
        const points = [start, start + size / 2, start + size];
        let best: { delta: number; value: number } | null = null;
        for (const position of points) {
          for (const target of targets(axis)) {
            const delta = target - position;
            if (
              Math.abs(delta) <= TOLERANCE &&
              (!best || Math.abs(delta) < Math.abs(best.delta))
            ) {
              best = { delta, value: target };
            }
          }
        }
        if (best) {
          if (axis === "x") adjusted.x += best.delta;
          else adjusted.y += best.delta;
          found.push({ axis, value: best.value });
        } else {
          // Failing another block, the grid.
          const rounded = Math.round(start / GRID_STEP) * GRID_STEP;
          if (Math.abs(rounded - start) <= TOLERANCE) {
            if (axis === "x") adjusted.x = rounded;
            else adjusted.y = rounded;
          }
        }
      }

      return { block: adjusted, guides: found };
    },
    [layout.blocks, snap],
  );

  // --- Gestures -------------------------------------------------------------

  const startGesture = (e: React.PointerEvent, block: Block, mode: Mode) => {
    if (readOnly || block.locked) return;
    e.stopPropagation();
    e.preventDefault();
    const rect = frame.current?.getBoundingClientRect();
    if (!rect) return;
    gesture.current = {
      mode,
      id: block.id,
      start: block,
      x0: e.clientX,
      y0: e.clientY,
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
    };
    onSelect(block.id);
    (e.target as Element).setPointerCapture?.(e.pointerId);
  };

  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      const g = gesture.current;
      if (!g) return;
      const dx = (e.clientX - g.x0) / g.width;
      const dy = (e.clientY - g.y0) / g.height;
      const d = g.start;

      if (g.mode === "rotate") {
        // Angle between the block's centre and the pointer. The handle sits
        // above the centre, so 0 degrees must mean "upwards".
        const centerX = g.left + (d.x + d.w / 2) * g.width;
        const centerY = g.top + (d.y + d.h / 2) * g.height;
        const angle = (Math.atan2(e.clientY - centerY, e.clientX - centerX) * 180) / Math.PI + 90;
        const rounded = e.shiftKey ? Math.round(angle / 15) * 15 : Math.round(angle);
        replace(g.id, { rotation: rounded });
        return;
      }

      let next: Block;
      switch (g.mode) {
        case "move":
          next = { ...d, x: d.x + dx, y: d.y + dy };
          break;
        case "se":
          next = { ...d, w: Math.max(MIN, d.w + dx), h: Math.max(MIN, d.h + dy) };
          break;
        case "sw":
          next = {
            ...d,
            x: Math.min(d.x + d.w - MIN, d.x + dx),
            w: Math.max(MIN, d.w - dx),
            h: Math.max(MIN, d.h + dy),
          };
          break;
        case "ne":
          next = {
            ...d,
            y: Math.min(d.y + d.h - MIN, d.y + dy),
            w: Math.max(MIN, d.w + dx),
            h: Math.max(MIN, d.h - dy),
          };
          break;
        case "nw":
          next = {
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

      const { block, guides: found } = snapTo(next, g.id);
      setGuides(found);
      replace(g.id, block);
    };

    const onUp = () => {
      gesture.current = null;
      setGuides([]);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [snapTo, replace]);

  // --- Keyboard: fine movement and deletion ---------------------------------

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (readOnly || !selection) return;
      const focused = document.activeElement?.tagName;
      if (focused === "INPUT" || focused === "TEXTAREA" || focused === "SELECT") return;

      const block = layout.blocks.find((b) => b.id === selection);
      if (!block || block.locked) return;
      const step = e.shiftKey ? 0.05 : 0.005;

      switch (e.key) {
        case "ArrowLeft":
          replace(block.id, { x: block.x - step });
          break;
        case "ArrowRight":
          replace(block.id, { x: block.x + step });
          break;
        case "ArrowUp":
          replace(block.id, { y: block.y - step });
          break;
        case "ArrowDown":
          replace(block.id, { y: block.y + step });
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

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [layout.blocks, readOnly, onBlocks, onSelect, replace, selection]);

  const safe = safeArea(brand, layout.width, layout.height);

  return (
    <div
      ref={frame}
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

      {safeAreas && (
        <div className="pointer-events-none absolute inset-0">
          <div
            className="absolute border-2 border-dashed border-sky-500/40"
            style={{
              left: `${safe.x * 100}%`,
              right: `${safe.x * 100}%`,
              top: `${safe.top * 100}%`,
              bottom: `${safe.bottom * 100}%`,
            }}
          />
        </div>
      )}

      {guides.map((g, i) => (
        <div
          key={i}
          className="pointer-events-none absolute bg-fuchsia-500"
          style={
            g.axis === "x"
              ? { left: `${g.value * 100}%`, top: 0, bottom: 0, width: 1 }
              : { top: `${g.value * 100}%`, left: 0, right: 0, height: 1 }
          }
        />
      ))}

      {blocks.map((block) => {
        const active = block.id === selection;
        return (
          <div
            key={block.id}
            onPointerDown={(e) => startGesture(e, block, "move")}
            className={`absolute ${block.locked ? "cursor-not-allowed" : "cursor-move"} ${
              active ? "outline outline-2 outline-ink" : "hover:outline hover:outline-1 hover:outline-ink/40"
            }`}
            style={{
              left: `${block.x * 100}%`,
              top: `${block.y * 100}%`,
              width: `${block.w * 100}%`,
              height: `${block.h * 100}%`,
              transform: `rotate(${block.rotation ?? 0}deg)`,
            }}
          >
            {active && !readOnly && !block.locked && (
              <>
                {(["nw", "ne", "sw", "se"] as Handle[]).map((p) => (
                  <span
                    key={p}
                    onPointerDown={(e) => startGesture(e, block, p)}
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
                  onPointerDown={(e) => startGesture(e, block, "rotate")}
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

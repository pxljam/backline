import { interpolate as remotionInterpolate, Easing } from "remotion";
import type { Animation } from "./types";

/**
 * A **small, safe** set of animations (§10.2): fade, slide, slow zoom, reveal.
 * No Bezier curves and no hand-placed keyframes — the goal is for a member to
 * produce a decent teaser in ten minutes.
 */
export interface AnimatedStyle {
  opacity: number;
  translateX: number;
  translateY: number;
  scale: number;
  clipInset: number;
}

export const NEUTRAL: AnimatedStyle = {
  opacity: 1,
  translateX: 0,
  translateY: 0,
  scale: 1,
  clipInset: 0,
};

export function animatedStyle(
  animations: Animation[] | undefined,
  frame: number,
  width: number,
): AnimatedStyle {
  if (!animations || animations.length === 0) return NEUTRAL;

  return animations.reduce<AnimatedStyle>((acc, anim) => {
    const from = anim.from ?? 0;
    const to = anim.to ?? from + 15;
    // A zero-length animation would divide by zero when interpolating: treat
    // it as already finished.
    if (to <= from) return acc;

    const t = remotionInterpolate(frame, [from, to], [0, 1], {
      extrapolateLeft: "clamp",
      extrapolateRight: "clamp",
      easing: Easing.out(Easing.cubic),
    });

    switch (anim.type) {
      case "fade":
        return { ...acc, opacity: acc.opacity * t };
      case "slide": {
        const amount = (anim.amount ?? 0.06) * width;
        const offset = (1 - t) * amount;
        switch (anim.direction ?? "up") {
          case "up":
            return { ...acc, translateY: acc.translateY + offset };
          case "down":
            return { ...acc, translateY: acc.translateY - offset };
          case "left":
            return { ...acc, translateX: acc.translateX + offset };
          case "right":
            return { ...acc, translateX: acc.translateX - offset };
        }
        return acc;
      }
      case "zoom": {
        const start = anim.amount ?? 1.08;
        return { ...acc, scale: acc.scale * (start + (1 - start) * t) };
      }
      case "reveal":
        return { ...acc, clipInset: Math.max(acc.clipInset, 1 - t) };
      default:
        return acc;
    }
  }, NEUTRAL);
}

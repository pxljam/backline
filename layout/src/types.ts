/**
 * JSON description of a layout — **the single source of truth** (PRD §15).
 *
 * Coordinate conventions, mirrored on the Rust side (`api/src/services/templates.rs`):
 * - `x`, `w` are fractions of the canvas **width**;
 * - `y`, `h` are fractions of its **height**;
 * - every **size** (font, stroke, radius) is a fraction of the **width**, so
 *   that two formats of equal width render text identically.
 *
 * Nothing here is expressed in pixels: that is what makes deriving one format
 * from another possible without rewriting anything.
 */

export type BlockType =
  | "text"
  | "image"
  | "video"
  | "logo"
  | "shape"
  | "gradient"
  | "group";

export type Align = "left" | "center" | "right";
export type VAlign = "top" | "middle" | "bottom";

export interface Timing {
  /** Frame the block appears on. */
  from?: number;
  /** Frame the block disappears on. */
  to?: number;
}

export type AnimationType = "fade" | "slide" | "zoom" | "reveal";

export interface Animation {
  type: AnimationType;
  /** Animation start, in frames, relative to the scene. */
  from?: number;
  /** Animation end. */
  to?: number;
  /** `slide`: direction the block enters from. */
  direction?: "up" | "down" | "left" | "right";
  /** `slide`: travel as a fraction of the width. `zoom`: starting factor. */
  amount?: number;
}

export interface TextProps {
  content: string;
  fontToken?: string;
  colorToken?: string;
  /** Fraction of the canvas width. */
  size?: number;
  align?: Align;
  valign?: VAlign;
  lineHeight?: number;
  tracking?: number;
  transform?: "none" | "uppercase" | "lowercase";
  /** Shrink the size until the text fits inside the block. */
  autoFit?: boolean;
}

export interface ImageProps {
  assetId?: string;
  /** Direct URL — used by the editor before upload. */
  src?: string;
  fit?: "cover" | "contain";
  radius?: number;
  focusX?: number;
  focusY?: number;
}

export interface ShapeProps {
  shape: "rect" | "circle" | "line";
  fillToken?: string;
  strokeToken?: string;
  strokeWidth?: number;
  radius?: number;
}

export interface GradientProps {
  fromToken?: string;
  toToken?: string;
  angle?: number;
}

export interface LogoProps {
  /** Key of the logo token in the brand. */
  logoToken?: string;
  variant?: "color" | "mono-light" | "mono-dark";
  fit?: "contain" | "cover";
}

export type BlockProps =
  | TextProps
  | ImageProps
  | ShapeProps
  | GradientProps
  | LogoProps
  | Record<string, unknown>;

export interface Block {
  id: string;
  type: BlockType;
  x: number;
  y: number;
  w: number;
  h: number;
  rotation?: number;
  opacity?: number;
  locked?: boolean;
  z?: number;
  props: BlockProps;
  timing?: Timing;
  animations?: Animation[];
  children?: Block[];
}

export interface Background {
  type: "color" | "gradient" | "image";
  token?: string;
  assetId?: string;
  src?: string;
  fromToken?: string;
  toToken?: string;
  angle?: number;
}

export interface Layout {
  version: number;
  width: number;
  height: number;
  background?: Background | null;
  blocks: Block[];
  /** Present only for an animated composition. */
  durationInFrames?: number;
  fps?: number;
}

/** Brand tokens, exactly as the API returns them. */
export interface Brand {
  color?: Record<string, { hex: string }>;
  font?: Record<string, { family: string; weight?: string; stack?: string; assetId?: string }>;
  logo?: Record<string, { assetId?: string; src?: string; variant?: string }>;
  grid?: Record<string, unknown>;
  rule?: Record<string, Record<string, unknown>>;
}

/** Media resolved to URLs, supplied by the API or by the render bundle. */
export type MediaMap = Record<string, { url: string; mime?: string }>;

/** Automatic event fields (§9.1). */
export type FieldData = Record<string, unknown>;

// --- Video: scenes and timeline (§10.2) -----------------------------------

export interface Scene {
  id: string;
  /** Scene duration, in frames. */
  durationInFrames: number;
  /** Entry transition. */
  transition?: { type: "cut" | "fade" | "slide"; durationInFrames?: number };
  background?: Background | null;
  blocks: Block[];
}

export interface AudioTrack {
  assetId?: string;
  src?: string;
  /** Entry point into the source file, in frames. */
  startFrom?: number;
  endAt?: number;
  volume?: number;
  fadeInFrames?: number;
  fadeOutFrames?: number;
}

export interface VideoSpec {
  version: number;
  width: number;
  height: number;
  fps: number;
  scenes: Scene[];
  audio?: AudioTrack | null;
}

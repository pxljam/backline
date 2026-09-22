/**
 * Description JSON d'une mise en page — **seule source de verite** (PRD §15).
 *
 * Conventions de coordonnees, identiques cote Rust (`api/src/services/templates.rs`) :
 * - `x`, `w` sont des fractions de la **largeur** du canevas ;
 * - `y`, `h` des fractions de la **hauteur** ;
 * - toutes les **tailles** (police, trait, rayon) sont des fractions de la
 *   **largeur**, pour que deux formats de meme largeur rendent un texte
 *   strictement identique.
 *
 * Rien ici n'est exprime en pixels : c'est ce qui rend une declinaison de
 * format possible sans reecrire quoi que ce soit.
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
  /** Image d'apparition, en frames. */
  from?: number;
  /** Image de disparition. */
  to?: number;
}

export type AnimationType = "fade" | "slide" | "zoom" | "reveal";

export interface Animation {
  type: AnimationType;
  /** Debut de l'animation, en frames, relatif au plan. */
  from?: number;
  /** Fin de l'animation. */
  to?: number;
  /** `slide` : direction d'entree. */
  direction?: "up" | "down" | "left" | "right";
  /** `slide` : amplitude en fraction de la largeur. `zoom` : facteur de depart. */
  amount?: number;
}

export interface TextProps {
  content: string;
  fontToken?: string;
  colorToken?: string;
  /** Fraction de la largeur du canevas. */
  size?: number;
  align?: Align;
  valign?: VAlign;
  lineHeight?: number;
  tracking?: number;
  transform?: "none" | "uppercase" | "lowercase";
  /** Reduit la taille jusqu'a ce que le texte tienne dans le bloc. */
  autoFit?: boolean;
}

export interface ImageProps {
  assetId?: string;
  /** URL directe — utilisee par l'editeur avant televersement. */
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
  /** Cle du token de logo dans la charte. */
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
  /** Presente uniquement pour une composition animee. */
  durationInFrames?: number;
  fps?: number;
}

/** Tokens de charte, tels que l'API les renvoie. */
export interface Brand {
  color?: Record<string, { hex: string }>;
  font?: Record<string, { family: string; weight?: string; stack?: string; assetId?: string }>;
  logo?: Record<string, { assetId?: string; src?: string; variant?: string }>;
  grid?: Record<string, unknown>;
  rule?: Record<string, Record<string, unknown>>;
}

/** Medias resolus en URL, fournis par l'API ou par le bundle de rendu. */
export type MediaMap = Record<string, { url: string; mime?: string }>;

/** Champs automatiques de l'evenement (§9.1). */
export type FieldData = Record<string, unknown>;

// --- Video : plans et timeline (§10.2) ------------------------------------

export interface Scene {
  id: string;
  /** Duree du plan, en frames. */
  durationInFrames: number;
  /** Transition d'entree. */
  transition?: { type: "cut" | "fade" | "slide"; durationInFrames?: number };
  background?: Background | null;
  blocks: Block[];
}

export interface AudioTrack {
  assetId?: string;
  src?: string;
  /** Point d'entree dans le fichier source, en frames. */
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

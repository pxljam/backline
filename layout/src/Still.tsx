import React from "react";
import type { Brand, FieldData, Layout, MediaMap } from "./types";
import { LayoutRenderer } from "./Renderer";

/**
 * Composition Remotion d'un **visuel fixe**.
 *
 * « Les visuels fixes passent aussi par Remotion (rendu d'image fixe). La seule
 * difference entre une image et une video devient la duree. » (§15)
 */
export const Still: React.FC<{
  layout: Layout;
  brand: Brand;
  data?: FieldData;
  media?: MediaMap;
}> = ({ layout, brand, data, media }) => (
  // Image 1 : les animations sont a leur etat final, ce qui est l'etat
  // attendu d'un visuel fixe.
  <LayoutRenderer layout={layout} brand={brand} data={data} media={media} frameOverride={99999} />
);

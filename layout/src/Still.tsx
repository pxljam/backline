import React from "react";
import type { Brand, FieldData, Layout, MediaMap } from "./types";
import { LayoutRenderer } from "./Renderer";

/**
 * Remotion composition for a **still visual**.
 *
 * "Still visuals also go through Remotion (single-frame render). The only
 * difference between an image and a video becomes duration." (§15)
 */
export const Still: React.FC<{
  layout: Layout;
  brand: Brand;
  data?: FieldData;
  media?: MediaMap;
}> = ({ layout, brand, data, media }) => (
  // Last frame: animations sit at their final state, which is what a still
  // visual is expected to show.
  <LayoutRenderer layout={layout} brand={brand} data={data} media={media} frameOverride={99999} />
);

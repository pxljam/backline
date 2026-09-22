import React from "react";
import { Composition } from "remotion";
import { Still } from "@backline/layout";
import type { Brand, FieldData, Layout, MediaMap } from "@backline/layout";

/**
 * Le service `stills` ne rend **que des visuels fixes** — aucun encodage video
 * ne tourne sur le VPS (§15).
 *
 * Il n'y a pas de composant de mise en page ici : on monte ceux de `layout/`,
 * exactement ceux qu'affiche l'editeur et qu'utilisera la CLI video.
 */

/**
 * `Record<string, unknown>` : Remotion exige des props serialisables, la
 * description JSON en est une par construction.
 */
export interface StillProps extends Record<string, unknown> {
  layout: Layout;
  brand: Brand;
  data: FieldData;
  media: MediaMap;
}

const PLACEHOLDER: Layout = { version: 1, width: 1080, height: 1350, blocks: [] };

const StillEntry: React.FC<StillProps> = ({ layout, brand, data, media }) => (
  <Still layout={layout} brand={brand} data={data} media={media} />
);

export const RemotionRoot: React.FC = () => (
  <Composition
    id="still"
    component={StillEntry}
    durationInFrames={1}
    fps={1}
    width={PLACEHOLDER.width}
    height={PLACEHOLDER.height}
    defaultProps={{
      layout: PLACEHOLDER,
      brand: {} as Brand,
      data: {} as FieldData,
      media: {} as MediaMap,
    }}
    // Le format **impose** ses dimensions : le cadre ne peut pas quitter le
    // ratio du format choisi (§9.2).
    calculateMetadata={({ props }: { props: StillProps }) => ({
      width: props.layout?.width ?? PLACEHOLDER.width,
      height: props.layout?.height ?? PLACEHOLDER.height,
      durationInFrames: 1,
      fps: 1,
    })}
  />
);

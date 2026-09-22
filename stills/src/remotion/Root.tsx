import React from "react";
import { Composition } from "remotion";
import { Still } from "@backline/layout";
import type { Brand, FieldData, Layout, MediaMap } from "@backline/layout";

/**
 * The `stills` service renders **still visuals only** — no video encoding runs
 * on the VPS (§15).
 *
 * There is no layout component here: we mount the ones from `layout/`, exactly
 * those the editor displays and the video CLI will use.
 */

/**
 * `Record<string, unknown>`: Remotion requires serialisable props, and the
 * JSON description is one by construction.
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
    // The format **dictates** its dimensions: the frame cannot leave the
    // ratio of the chosen format (§9.2).
    calculateMetadata={({ props }: { props: StillProps }) => ({
      width: props.layout?.width ?? PLACEHOLDER.width,
      height: props.layout?.height ?? PLACEHOLDER.height,
      durationInFrames: 1,
      fps: 1,
    })}
  />
);

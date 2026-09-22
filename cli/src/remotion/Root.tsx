import React from "react";
import { Composition } from "remotion";
import { VideoComposition, totalDuration } from "@backline/layout";
import type { Brand, FieldData, MediaMap, VideoSpec } from "@backline/layout";

/**
 * The CLI ships **the same layout components** as the server (§15). That is
 * what guarantees a teaser rendered on Anas's laptop is identical to the
 * preview Antoine approved in their browser.
 */

export interface VideoProps extends Record<string, unknown> {
  spec: VideoSpec;
  brand: Brand;
  data: FieldData;
  media: MediaMap;
}

const EMPTY: VideoSpec = {
  version: 1,
  width: 1080,
  height: 1920,
  fps: 30,
  scenes: [],
  audio: null,
};

const VideoEntry: React.FC<VideoProps> = ({ spec, brand, data, media }) => (
  <VideoComposition spec={spec} brand={brand} data={data} media={media} />
);

export const RemotionRoot: React.FC = () => (
  <Composition
    id="video"
    component={VideoEntry}
    durationInFrames={30}
    fps={EMPTY.fps}
    width={EMPTY.width}
    height={EMPTY.height}
    defaultProps={{
      spec: EMPTY,
      brand: {} as Brand,
      data: {} as FieldData,
      media: {} as MediaMap,
    }}
    calculateMetadata={({ props }: { props: VideoProps }) => {
      const spec = props.spec ?? EMPTY;
      return {
        width: spec.width,
        height: spec.height,
        fps: spec.fps,
        // An empty composition would render a zero-frame video: Remotion
        // refuses, with an unhelpful error. One frame minimum.
        durationInFrames: Math.max(1, totalDuration(spec)),
      };
    }}
  />
);

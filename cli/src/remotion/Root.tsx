import React from "react";
import { Composition } from "remotion";
import { VideoComposition, totalDuration } from "@backline/layout";
import type { Brand, FieldData, MediaMap, VideoSpec } from "@backline/layout";

/**
 * La CLI embarque **les memes composants de mise en page** que le serveur
 * (§15). C'est ce qui garantit qu'un teaser rendu sur le portable d'Anas est
 * identique a l'apercu qu'Antoine a valide dans son navigateur.
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
        // Une composition vide rendrait une video de zero image : Remotion
        // refuse, et l'erreur serait incomprehensible. Une image minimum.
        durationInFrames: Math.max(1, totalDuration(spec)),
      };
    }}
  />
);

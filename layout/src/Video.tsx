import React from "react";
import { AbsoluteFill, Audio, interpolate, Sequence, useCurrentFrame } from "remotion";
import type { Brand, FieldData, Layout, MediaMap, Scene, VideoSpec } from "./types";
import { LayoutRenderer } from "./Renderer";
import { mediaUrl } from "./resolve";

/**
 * Remotion composition for a **video**: a sequence of scenes, each with its own
 * duration and entry transition, plus a single audio track (§10.2).
 *
 * The blocks are exactly those of still visuals: one layout implementation, and
 * therefore "what the admin sees is what comes out".
 */

export function totalDuration(spec: VideoSpec): number {
  return spec.scenes.reduce((sum, scene) => sum + Math.max(1, scene.durationInFrames), 0);
}

const SceneLayer: React.FC<{
  scene: Scene;
  spec: VideoSpec;
  brand: Brand;
  data: FieldData;
  media: MediaMap;
}> = ({ scene, spec, brand, data, media }) => {
  const frame = useCurrentFrame();

  const layout: Layout = {
    version: spec.version,
    width: spec.width,
    height: spec.height,
    background: scene.background ?? null,
    blocks: scene.blocks,
  };

  // The entry transition belongs to the incoming scene: a crossfade between
  // two scenes is not a third object to manage.
  const transition = scene.transition ?? { type: "cut" as const };
  const fadeFrames = transition.type === "fade" ? (transition.durationInFrames ?? 12) : 0;
  const slideFrames = transition.type === "slide" ? (transition.durationInFrames ?? 12) : 0;

  const opacity =
    fadeFrames > 0
      ? interpolate(frame, [0, fadeFrames], [0, 1], {
          extrapolateLeft: "clamp",
          extrapolateRight: "clamp",
        })
      : 1;

  const translateX =
    slideFrames > 0
      ? interpolate(frame, [0, slideFrames], [spec.width, 0], {
          extrapolateLeft: "clamp",
          extrapolateRight: "clamp",
        })
      : 0;

  return (
    <AbsoluteFill style={{ opacity, transform: `translateX(${translateX}px)` }}>
      <LayoutRenderer layout={layout} brand={brand} data={data} media={media} />
    </AbsoluteFill>
  );
};

export const VideoComposition: React.FC<{
  spec: VideoSpec;
  brand: Brand;
  data?: FieldData;
  media?: MediaMap;
}> = ({ spec, brand, data = {}, media = {} }) => {
  const audioUrl = spec.audio ? mediaUrl(media, spec.audio.assetId, spec.audio.src) : undefined;

  let offset = 0;
  const sequences = spec.scenes.map((scene) => {
    const from = offset;
    const durationInFrames = Math.max(1, scene.durationInFrames);
    offset += durationInFrames;
    return { scene, from, durationInFrames };
  });

  return (
    <AbsoluteFill style={{ background: "black" }}>
      {sequences.map(({ scene, from, durationInFrames }) => (
        <Sequence key={scene.id} from={from} durationInFrames={durationInFrames}>
          <SceneLayer scene={scene} spec={spec} brand={brand} data={data} media={media} />
        </Sequence>
      ))}
      {audioUrl ? (
        <Audio
          src={audioUrl}
          volume={spec.audio?.volume ?? 1}
          startFrom={spec.audio?.startFrom}
          endAt={spec.audio?.endAt}
        />
      ) : null}
    </AbsoluteFill>
  );
};

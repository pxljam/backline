import React from "react";
import { AbsoluteFill, Audio, interpolate, Sequence, useCurrentFrame } from "remotion";
import type { Brand, FieldData, Layout, MediaMap, Scene, VideoSpec } from "./types";
import { LayoutRenderer } from "./Renderer";
import { mediaUrl } from "./resolve";

/**
 * Composition Remotion d'une **video** : une suite de plans, chacun avec sa
 * duree et sa transition d'entree, plus une piste audio unique (§10.2).
 *
 * Les blocs sont exactement ceux des visuels fixes : une seule implementation
 * de la mise en page, et donc « ce que voit l'admin est ce qui sort ».
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

  // La transition d'entree appartient au plan entrant : un fondu entre deux
  // plans n'est pas un troisieme objet a gerer.
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

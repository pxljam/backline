import React from "react";
import { AbsoluteFill, useCurrentFrame } from "remotion";
import type { Background, Block, Brand, FieldData, Layout, MediaMap } from "./types";
import { color, mediaUrl } from "./resolve";
import { animatedStyle } from "./animate";
import { TextBlock } from "./blocks/Text";
import { ImageBlock, LogoBlock, VideoBlock } from "./blocks/Media";
import { GradientBlock, ShapeBlock } from "./blocks/Shape";

/**
 * Renders a JSON description. **This is the only place in the project where
 * layout is written** (PRD §15): the editor displays it as-is and puts its
 * handles on top, the `stills` service draws still visuals from it, the CLI
 * draws videos from it. Any temptation to reimplement rendering elsewhere is a
 * regression.
 */

export const BackgroundLayer: React.FC<{
  background: Background | null | undefined;
  brand: Brand;
  media: MediaMap;
}> = ({ background, brand, media }) => {
  if (!background) return null;

  if (background.type === "gradient") {
    return (
      <AbsoluteFill
        style={{
          background: `linear-gradient(${background.angle ?? 180}deg, ${color(
            brand,
            background.fromToken,
            "#ffffff",
          )}, ${color(brand, background.toToken, "#000000")})`,
        }}
      />
    );
  }

  if (background.type === "image") {
    const url = mediaUrl(media, background.assetId, background.src);
    if (!url) return null;
    return (
      <AbsoluteFill
        style={{
          backgroundImage: `url(${url})`,
          backgroundSize: "cover",
          backgroundPosition: "center",
        }}
      />
    );
  }

  return <AbsoluteFill style={{ background: color(brand, background.token, "#ffffff") }} />;
};

interface BlockLayerProps {
  block: Block;
  brand: Brand;
  data: FieldData;
  media: MediaMap;
  canvasWidth: number;
  frame: number;
}

const BlockLayer: React.FC<BlockLayerProps> = ({
  block,
  brand,
  data,
  media,
  canvasWidth,
  frame,
}) => {
  // The block's time window: outside its range, it does not exist.
  const from = block.timing?.from ?? 0;
  const to = block.timing?.to;
  if (frame < from) return null;
  if (to !== undefined && frame >= to) return null;

  const anim = animatedStyle(block.animations, frame - from, canvasWidth);
  const opacity = (block.opacity ?? 1) * anim.opacity;

  const inner = (() => {
    switch (block.type) {
      case "text":
        return (
          <TextBlock
            props={block.props as never}
            brand={brand}
            data={data}
            canvasWidth={canvasWidth}
          />
        );
      case "image":
        return <ImageBlock props={block.props as never} media={media} />;
      case "video":
        return <VideoBlock props={block.props as never} media={media} />;
      case "logo":
        return <LogoBlock props={block.props as never} brand={brand} media={media} />;
      case "shape":
        return <ShapeBlock props={block.props as never} brand={brand} canvasWidth={canvasWidth} />;
      case "gradient":
        return <GradientBlock props={block.props as never} brand={brand} />;
      case "group":
        return (
          <div style={{ position: "relative", width: "100%", height: "100%" }}>
            {(block.children ?? []).map((child) => (
              <BlockLayer
                key={child.id}
                block={child}
                brand={brand}
                data={data}
                media={media}
                canvasWidth={canvasWidth}
                frame={frame}
              />
            ))}
          </div>
        );
      default:
        return null;
    }
  })();

  return (
    <div
      data-block-id={block.id}
      style={{
        position: "absolute",
        left: `${block.x * 100}%`,
        top: `${block.y * 100}%`,
        width: `${block.w * 100}%`,
        height: `${block.h * 100}%`,
        opacity,
        transform: `translate(${anim.translateX}px, ${anim.translateY}px) rotate(${
          block.rotation ?? 0
        }deg) scale(${anim.scale})`,
        transformOrigin: "center center",
        clipPath:
          anim.clipInset > 0 ? `inset(0 ${anim.clipInset * 100}% 0 0)` : undefined,
        zIndex: block.z ?? 0,
      }}
    >
      {inner}
    </div>
  );
};

export interface LayoutRendererProps {
  layout: Layout;
  brand: Brand;
  data?: FieldData;
  media?: MediaMap;
  /** Current frame, for rendering outside a Remotion context (static preview). */
  frameOverride?: number;
}

export const LayoutRenderer: React.FC<LayoutRendererProps> = ({
  layout,
  brand,
  data = {},
  media = {},
  frameOverride,
}) => {
  const remotionFrame = useCurrentFrameSafe();
  const frame = frameOverride ?? remotionFrame;

  const blocks = [...layout.blocks].sort((a, b) => (a.z ?? 0) - (b.z ?? 0));

  return (
    <AbsoluteFill style={{ overflow: "hidden" }}>
      <BackgroundLayer background={layout.background} brand={brand} media={media} />
      {blocks.map((block) => (
        <BlockLayer
          key={block.id}
          block={block}
          brand={brand}
          data={data}
          media={media}
          canvasWidth={layout.width}
          frame={frame}
        />
      ))}
    </AbsoluteFill>
  );
};

/**
 * `useCurrentFrame` throws outside a Remotion context. The editor nonetheless
 * renders the same components into a plain div, so we fall back to frame 0 —
 * exactly the "animation finished" state of a still visual.
 */
function useCurrentFrameSafe(): number {
  try {
    // eslint-disable-next-line react-hooks/rules-of-hooks
    return useCurrentFrame();
  } catch {
    return 0;
  }
}

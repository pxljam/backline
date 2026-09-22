import React from "react";
import { Img, OffthreadVideo } from "remotion";
import type { Brand, ImageProps, LogoProps, MediaMap } from "../types";
import { mediaUrl } from "../resolve";

/** An empty, visible frame: missing media shows up, it does not vanish. */
const Missing: React.FC<{ label: string }> = ({ label }) => (
  <div
    style={{
      width: "100%",
      height: "100%",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      background: "rgba(0,0,0,0.06)",
      border: "2px dashed rgba(0,0,0,0.25)",
      color: "rgba(0,0,0,0.45)",
      fontFamily: "Inter, Helvetica, Arial, sans-serif",
      fontSize: 18,
      textAlign: "center",
      padding: 12,
      boxSizing: "border-box",
    }}
  >
    {label}
  </div>
);

export const ImageBlock: React.FC<{ props: ImageProps; media: MediaMap }> = ({ props, media }) => {
  const url = mediaUrl(media, props.assetId, props.src);
  if (!url) return <Missing label="image a choisir" />;
  return (
    <Img
      src={url}
      style={{
        width: "100%",
        height: "100%",
        objectFit: props.fit ?? "cover",
        objectPosition: `${(props.focusX ?? 0.5) * 100}% ${(props.focusY ?? 0.5) * 100}%`,
        borderRadius: props.radius ? `${props.radius * 100}%` : undefined,
      }}
    />
  );
};

export const VideoBlock: React.FC<{ props: ImageProps; media: MediaMap }> = ({ props, media }) => {
  const url = mediaUrl(media, props.assetId, props.src);
  if (!url) return <Missing label="video a choisir" />;
  return (
    <OffthreadVideo
      src={url}
      style={{ width: "100%", height: "100%", objectFit: props.fit ?? "cover" }}
    />
  );
};

export const LogoBlock: React.FC<{ props: LogoProps; brand: Brand; media: MediaMap }> = ({
  props,
  brand,
  media,
}) => {
  const token = props.logoToken ?? "primary";
  const entry = brand.logo?.[token];
  const url = mediaUrl(media, entry?.assetId, entry?.src);
  if (!url) return <Missing label="logo a televerser" />;
  return (
    <Img
      src={url}
      style={{ width: "100%", height: "100%", objectFit: props.fit ?? "contain" }}
    />
  );
};

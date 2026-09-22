import React from "react";
import type { Brand, GradientProps, ShapeProps } from "../types";
import { color } from "../resolve";

export const ShapeBlock: React.FC<{
  props: ShapeProps;
  brand: Brand;
  canvasWidth: number;
}> = ({ props, brand, canvasWidth }) => {
  const fill = color(brand, props.fillToken, "#111111");
  const stroke = props.strokeToken ? color(brand, props.strokeToken) : undefined;
  const strokeWidth = (props.strokeWidth ?? 0) * canvasWidth;

  if (props.shape === "circle") {
    return (
      <div
        style={{
          width: "100%",
          height: "100%",
          borderRadius: "50%",
          background: props.fillToken ? fill : "transparent",
          border: stroke ? `${strokeWidth}px solid ${stroke}` : undefined,
          boxSizing: "border-box",
        }}
      />
    );
  }

  if (props.shape === "line") {
    return (
      <div
        style={{
          width: "100%",
          height: Math.max(strokeWidth, 1),
          background: stroke ?? fill,
          alignSelf: "center",
        }}
      />
    );
  }

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        background: props.fillToken ? fill : "transparent",
        border: stroke ? `${strokeWidth}px solid ${stroke}` : undefined,
        borderRadius: props.radius ? props.radius * canvasWidth : undefined,
        boxSizing: "border-box",
      }}
    />
  );
};

export const GradientBlock: React.FC<{ props: GradientProps; brand: Brand }> = ({
  props,
  brand,
}) => (
  <div
    style={{
      width: "100%",
      height: "100%",
      background: `linear-gradient(${props.angle ?? 180}deg, ${color(
        brand,
        props.fromToken,
        "#00000000",
      )}, ${color(brand, props.toToken, "#000000")})`,
    }}
  />
);

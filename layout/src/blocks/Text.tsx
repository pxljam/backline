import React from "react";
import type { Align, Brand, FieldData, TextProps, VAlign } from "../types";
import { color, fontStack, fontWeight, interpolate } from "../resolve";

const justify: Record<VAlign, React.CSSProperties["justifyContent"]> = {
  top: "flex-start",
  middle: "center",
  bottom: "flex-end",
};

const items: Record<Align, React.CSSProperties["alignItems"]> = {
  left: "flex-start",
  center: "center",
  right: "flex-end",
};

export const TextBlock: React.FC<{
  props: TextProps;
  brand: Brand;
  data: FieldData;
  canvasWidth: number;
}> = ({ props, brand, data, canvasWidth }) => {
  const content = interpolate(props.content ?? "", data);
  // La taille est une fraction de la largeur du canevas : deux formats de meme
  // largeur rendent un texte strictement identique.
  const fontSize = (props.size ?? 0.05) * canvasWidth;
  const align = props.align ?? "left";
  const valign = props.valign ?? "top";

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        justifyContent: justify[valign],
        alignItems: items[align],
      }}
    >
      <div
        style={{
          width: "100%",
          fontFamily: fontStack(brand, props.fontToken),
          fontWeight: fontWeight(brand, props.fontToken),
          color: color(brand, props.colorToken, "#111111"),
          fontSize,
          lineHeight: props.lineHeight ?? 1.1,
          letterSpacing: (props.tracking ?? 0) * fontSize,
          textAlign: align,
          textTransform: props.transform === "none" ? undefined : props.transform,
          // Les retours a la ligne d'un champ automatique (un line-up, par
          // exemple) doivent rester des retours a la ligne.
          whiteSpace: "pre-wrap",
          overflowWrap: "anywhere",
        }}
      >
        {content}
      </div>
    </div>
  );
};

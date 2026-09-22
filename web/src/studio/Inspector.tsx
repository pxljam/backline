import React from "react";
import type {
  Animation, Block, ImageProps, LogoProps, ShapeProps, TextProps, GradientProps,
} from "@backline/layout";
import type { Asset, BrandToken } from "../lib/types";
import { Button, Field, Input, Select, Textarea } from "../components/ui";
import { EVENT_FIELDS, fieldToken } from "./fields";
import { align } from "./blocks";

/**
 * Properties panel. **Only brand tokens are offered** in the colour and font
 * selectors: the brand cannot be left by accident (§9.1).
 */

export interface InspectorProps {
  block: Block;
  tokens: BrandToken[];
  assets: Asset[];
  onChange: (patch: Partial<Block>) => void;
  onProps: (patch: Record<string, unknown>) => void;
  onDelete: () => void;
  onDuplicate: () => void;
  onLayer: (direction: -1 | 1) => void;
  /** The video editor additionally exposes the time window and animations. */
  timeline?: boolean;
}

const TokenSelect: React.FC<{
  label: string;
  kind: BrandToken["kind"];
  tokens: BrandToken[];
  value: string | undefined;
  onChange: (key: string) => void;
}> = ({ label, kind, tokens, value, onChange }) => (
  <Field label={label}>
    <Select value={value ?? ""} onChange={(e) => onChange(e.target.value)}>
      <option value="">— aucun —</option>
      {tokens
        .filter((t) => t.kind === kind)
        .map((t) => (
          <option key={t.key} value={t.key}>
            {t.label}
          </option>
        ))}
    </Select>
  </Field>
);

export const Inspector: React.FC<InspectorProps> = ({
  block,
  tokens,
  assets,
  onChange,
  onProps,
  onDelete,
  onDuplicate,
  onLayer,
  timeline = false,
}) => {
  const p = block.props as Record<string, unknown>;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap gap-1.5">
        <Button size="sm" onClick={() => onLayer(1)}>
          monter
        </Button>
        <Button size="sm" onClick={() => onLayer(-1)}>
          descendre
        </Button>
        <Button size="sm" onClick={onDuplicate}>
          dupliquer
        </Button>
        <Button size="sm" onClick={() => onChange({ locked: !block.locked })}>
          {block.locked ? "deverrouiller" : "verrouiller"}
        </Button>
        <Button size="sm" variant="danger" onClick={onDelete}>
          supprimer
        </Button>
      </div>

      <div>
        <p className="mb-1 text-xs font-medium uppercase tracking-wide text-ink-soft">Aligner</p>
        <div className="flex flex-wrap gap-1.5">
          {[
            ["left", "gauche"],
            ["center-h", "centre H"],
            ["right", "droite"],
            ["top", "haut"],
            ["center-v", "centre V"],
            ["bottom", "bas"],
          ].map(([key, label]) => (
            <Button key={key} size="sm" onClick={() => onChange(align(block, key))}>
              {label}
            </Button>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-2">
        <Field label="X">
          <Input
            type="number"
            step={0.01}
            value={round(block.x)}
            onChange={(e) => onChange({ x: Number(e.target.value) })}
          />
        </Field>
        <Field label="Y">
          <Input
            type="number"
            step={0.01}
            value={round(block.y)}
            onChange={(e) => onChange({ y: Number(e.target.value) })}
          />
        </Field>
        <Field label="Largeur">
          <Input
            type="number"
            step={0.01}
            value={round(block.w)}
            onChange={(e) => onChange({ w: Number(e.target.value) })}
          />
        </Field>
        <Field label="Hauteur">
          <Input
            type="number"
            step={0.01}
            value={round(block.h)}
            onChange={(e) => onChange({ h: Number(e.target.value) })}
          />
        </Field>
        <Field label="Rotation">
          <Input
            type="number"
            value={block.rotation ?? 0}
            onChange={(e) => onChange({ rotation: Number(e.target.value) })}
          />
        </Field>
        <Field label="Opacite">
          <Input
            type="number"
            min={0}
            max={1}
            step={0.05}
            value={block.opacity ?? 1}
            onChange={(e) => onChange({ opacity: Number(e.target.value) })}
          />
        </Field>
      </div>

      {block.type === "text" && (
        <TextBlockProps props={p as unknown as TextProps} tokens={tokens} onProps={onProps} />
      )}

      {(block.type === "image" || block.type === "video") && (
        <MediaProps props={p as unknown as ImageProps} assets={assets} kind={block.type} onProps={onProps} />
      )}

      {block.type === "logo" && (
        <>
          <TokenSelect
            label="Logo"
            kind="logo"
            tokens={tokens}
            value={(p as unknown as LogoProps).logoToken}
            onChange={(key) => onProps({ logoToken: key })}
          />
          <Field label="Cadrage">
            <Select
              value={(p as unknown as LogoProps).fit ?? "contain"}
              onChange={(e) => onProps({ fit: e.target.value })}
            >
              <option value="contain">entier</option>
              <option value="cover">rempli</option>
            </Select>
          </Field>
        </>
      )}

      {block.type === "shape" && (
        <>
          <Field label="Forme">
            <Select
              value={(p as unknown as ShapeProps).shape ?? "rect"}
              onChange={(e) => onProps({ shape: e.target.value })}
            >
              <option value="rect">rectangle</option>
              <option value="circle">cercle</option>
              <option value="line">trait</option>
            </Select>
          </Field>
          <TokenSelect
            label="Remplissage"
            kind="color"
            tokens={tokens}
            value={(p as unknown as ShapeProps).fillToken}
            onChange={(key) => onProps({ fillToken: key })}
          />
          <TokenSelect
            label="Contour"
            kind="color"
            tokens={tokens}
            value={(p as unknown as ShapeProps).strokeToken}
            onChange={(key) => onProps({ strokeToken: key })}
          />
          <Field label="Epaisseur du contour" hint="Fraction de la largeur du canevas.">
            <Input
              type="number"
              step={0.002}
              value={(p as unknown as ShapeProps).strokeWidth ?? 0}
              onChange={(e) => onProps({ strokeWidth: Number(e.target.value) })}
            />
          </Field>
        </>
      )}

      {block.type === "gradient" && (
        <>
          <TokenSelect
            label="De"
            kind="color"
            tokens={tokens}
            value={(p as unknown as GradientProps).fromToken}
            onChange={(key) => onProps({ fromToken: key })}
          />
          <TokenSelect
            label="Vers"
            kind="color"
            tokens={tokens}
            value={(p as unknown as GradientProps).toToken}
            onChange={(key) => onProps({ toToken: key })}
          />
          <Field label="Angle">
            <Input
              type="number"
              value={(p as unknown as GradientProps).angle ?? 180}
              onChange={(e) => onProps({ angle: Number(e.target.value) })}
            />
          </Field>
        </>
      )}

      {timeline && <TimingProps block={block} onChange={onChange} />}
    </div>
  );
};

const TextBlockProps: React.FC<{
  props: TextProps;
  tokens: BrandToken[];
  onProps: (patch: Record<string, unknown>) => void;
}> = ({ props, tokens, onProps }) => (
  <>
    <Field label="Contenu" hint="Les champs automatiques se remplissent a la generation.">
      <Textarea
        rows={3}
        value={props.content ?? ""}
        onChange={(e) => onProps({ content: e.target.value })}
      />
    </Field>

    <div>
      <p className="mb-1 text-xs font-medium uppercase tracking-wide text-ink-soft">
        Champs automatiques
      </p>
      <div className="flex flex-wrap gap-1">
        {EVENT_FIELDS.map((c) => (
          <button
            key={c.path}
            type="button"
            title={c.label}
            onClick={() => onProps({ content: `${props.content ?? ""}${fieldToken(c.path)}` })}
            className="rounded border border-line px-1.5 py-0.5 text-[11px] hover:border-ink"
          >
            {c.label}
          </button>
        ))}
      </div>
    </div>

    <TokenSelect
      label="Police"
      kind="font"
      tokens={tokens}
      value={props.fontToken}
      onChange={(key) => onProps({ fontToken: key })}
    />
    <TokenSelect
      label="Couleur"
      kind="color"
      tokens={tokens}
      value={props.colorToken}
      onChange={(key) => onProps({ colorToken: key })}
    />

    <div className="grid grid-cols-2 gap-2">
      <Field label="Taille" hint="Fraction de la largeur.">
        <Input
          type="number"
          step={0.005}
          value={props.size ?? 0.05}
          onChange={(e) => onProps({ size: Number(e.target.value) })}
        />
      </Field>
      <Field label="Interligne">
        <Input
          type="number"
          step={0.05}
          value={props.lineHeight ?? 1.1}
          onChange={(e) => onProps({ lineHeight: Number(e.target.value) })}
        />
      </Field>
      <Field label="Alignement">
        <Select value={props.align ?? "left"} onChange={(e) => onProps({ align: e.target.value })}>
          <option value="left">gauche</option>
          <option value="center">centre</option>
          <option value="right">droite</option>
        </Select>
      </Field>
      <Field label="Vertical">
        <Select value={props.valign ?? "top"} onChange={(e) => onProps({ valign: e.target.value })}>
          <option value="top">haut</option>
          <option value="middle">milieu</option>
          <option value="bottom">bas</option>
        </Select>
      </Field>
      <Field label="Casse">
        <Select
          value={props.transform ?? "none"}
          onChange={(e) => onProps({ transform: e.target.value })}
        >
          <option value="none">telle quelle</option>
          <option value="uppercase">MAJUSCULES</option>
          <option value="lowercase">minuscules</option>
        </Select>
      </Field>
      <Field label="Interlettrage">
        <Input
          type="number"
          step={0.01}
          value={props.tracking ?? 0}
          onChange={(e) => onProps({ tracking: Number(e.target.value) })}
        />
      </Field>
    </div>
  </>
);

const MediaProps: React.FC<{
  props: ImageProps;
  assets: Asset[];
  kind: "image" | "video";
  onProps: (patch: Record<string, unknown>) => void;
}> = ({ props, assets, kind, onProps }) => (
  <>
    <Field label={kind === "image" ? "Image" : "Video"} hint="Bibliotheque du collectif.">
      <Select value={props.assetId ?? ""} onChange={(e) => onProps({ assetId: e.target.value })}>
        <option value="">— a choisir —</option>
        {assets
          .filter((a) => a.kind === kind)
          .map((a) => (
            <option key={a.id} value={a.id}>
              {a.filename}
            </option>
          ))}
      </Select>
    </Field>
    <div className="grid grid-cols-2 gap-2">
      <Field label="Cadrage">
        <Select value={props.fit ?? "cover"} onChange={(e) => onProps({ fit: e.target.value })}>
          <option value="cover">rempli</option>
          <option value="contain">entier</option>
        </Select>
      </Field>
      <Field label="Arrondi">
        <Input
          type="number"
          step={0.01}
          min={0}
          max={0.5}
          value={props.radius ?? 0}
          onChange={(e) => onProps({ radius: Number(e.target.value) })}
        />
      </Field>
    </div>
  </>
);

const ANIMATIONS: Animation["type"][] = ["fade", "slide", "zoom", "reveal"];

const TimingProps: React.FC<{ block: Block; onChange: (patch: Partial<Block>) => void }> = ({
  block,
  onChange,
}) => {
  const anim = block.animations?.[0];
  return (
    <div className="border-t border-line pt-3">
      <p className="mb-2 text-xs font-medium uppercase tracking-wide text-ink-soft">
        Apparition dans le plan
      </p>
      <div className="grid grid-cols-2 gap-2">
        <Field label="De (images)">
          <Input
            type="number"
            value={block.timing?.from ?? 0}
            onChange={(e) => onChange({ timing: { ...block.timing, from: Number(e.target.value) } })}
          />
        </Field>
        <Field label="A (images)">
          <Input
            type="number"
            value={block.timing?.to ?? ""}
            onChange={(e) =>
              onChange({
                timing: {
                  ...block.timing,
                  to: e.target.value === "" ? undefined : Number(e.target.value),
                },
              })
            }
          />
        </Field>
        <Field label="Animation">
          <Select
            value={anim?.type ?? ""}
            onChange={(e) =>
              onChange({
                animations: e.target.value
                  ? [{ type: e.target.value as Animation["type"], from: 0, to: 18 }]
                  : [],
              })
            }
          >
            <option value="">aucune</option>
            {ANIMATIONS.map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </Select>
        </Field>
        {anim?.type === "slide" && (
          <Field label="Direction">
            <Select
              value={anim.direction ?? "up"}
              onChange={(e) =>
                onChange({
                  animations: [{ ...anim, direction: e.target.value as Animation["direction"] }],
                })
              }
            >
              <option value="up">vers le haut</option>
              <option value="down">vers le bas</option>
              <option value="left">vers la gauche</option>
              <option value="right">vers la droite</option>
            </Select>
          </Field>
        )}
        {anim && (
          <Field label="Duree (images)">
            <Input
              type="number"
              value={anim.to ?? 18}
              onChange={(e) => onChange({ animations: [{ ...anim, to: Number(e.target.value) }] })}
            />
          </Field>
        )}
      </div>
    </div>
  );
};

function round(v: number): number {
  return Math.round(v * 1000) / 1000;
}

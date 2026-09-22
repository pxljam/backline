import React, { useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import type { Background, Block, BlockType, FieldData, Layout } from "@backline/layout";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { Asset, BacklineEvent, BrandView, Format, Template } from "../lib/types";
import { Badge, Button, Card, ErrorNote, Field, Loading, Select } from "../components/ui";
import { Canvas } from "./Canvas";
import { Inspector } from "./Inspector";
import { toBrand, useMediaMap } from "./brand";
import { DONNEES_EXEMPLE } from "./fields";
import { NOMS_BLOCS, deplacerCalque, dupliquer, nouveauBloc } from "./blocks";

/**
 * Editeur de gabarit (§9.1 a §9.3). Un gabarit est concu sur un **format
 * maitre** puis decline : chaque declinaison memorise ses propres ajustements
 * sans toucher au maitre, et le cadre reste verrouille au ratio du format.
 */
export const TemplateEditor: React.FC = () => {
  const { id = "" } = useParams();
  const base = useCollectiveBase();
  const { isAdmin } = useSession();

  const template = useResource<Template>(`${base}/studio/templates/${id}`, [id]);
  const formats = useResource<Format[]>(`${base}/studio/formats`);
  const assets = useResource<Asset[]>(`${base}/studio/assets`);
  const events = useResource<BacklineEvent[]>(`${base}/events`);
  const { run, busy, error } = useAction();

  const [formatId, setFormatId] = useState<string>("");
  const [layout, setLayout] = useState<Layout | null>(null);
  const [selection, setSelection] = useState<string | null>(null);
  const [modifie, setModifie] = useState(false);
  const [magnetisme, setMagnetisme] = useState(true);
  const [zonesSures, setZonesSures] = useState(true);
  const [eventId, setEventId] = useState("");

  const brand = useResource<BrandView>(
    template.data
      ? `${base}/studio/brand${template.data.group_id ? `?group_id=${template.data.group_id}` : ""}`
      : null,
    [template.data?.group_id],
  );

  // La declinaison affichee par defaut est le format maitre.
  useEffect(() => {
    if (!template.data) return;
    const maitre = template.data.variants.find((v) => v.is_master) ?? template.data.variants[0];
    setFormatId((actuel) =>
      actuel && template.data?.variants.some((v) => v.format_id === actuel)
        ? actuel
        : (maitre?.format_id ?? ""),
    );
  }, [template.data]);

  const variante = template.data?.variants.find((v) => v.format_id === formatId);

  useEffect(() => {
    if (variante) {
      setLayout(structuredClone(variante.layout));
      setSelection(null);
      setModifie(false);
    }
  }, [variante?.format_id, template.data?.version]); // eslint-disable-line react-hooks/exhaustive-deps

  const fields = useResource<FieldData>(
    eventId ? `${base}/studio/preview-fields/${eventId}` : null,
    [eventId],
  );

  const assetIds = useMemo(() => {
    const ids: string[] = [];
    const visiter = (blocks: Block[]) => {
      for (const b of blocks) {
        const a = (b.props as { assetId?: string }).assetId;
        if (a) ids.push(a);
        if (b.children) visiter(b.children);
      }
    };
    if (layout) visiter(layout.blocks);
    if (layout?.background?.assetId) ids.push(layout.background.assetId);
    for (const token of brand.data?.tokens ?? []) {
      const value = token.value as { assetId?: string };
      if (token.kind === "logo" && value.assetId) ids.push(value.assetId);
    }
    return ids;
  }, [layout, brand.data]);

  const media = useMediaMap(base, assetIds);
  const tokens = brand.data ? [...(brand.data.inherited ?? []), ...brand.data.tokens] : [];
  const charte = brand.data ? toBrand(brand.data.tokens, brand.data.inherited) : {};

  if (template.loading || !layout) return <Loading />;

  const bloc = layout.blocks.find((b) => b.id === selection) ?? null;

  const majBlocks = (blocks: Block[]) => {
    setLayout({ ...layout, blocks });
    setModifie(true);
  };

  const patchBloc = (patch: Partial<Block>) =>
    bloc && majBlocks(layout.blocks.map((b) => (b.id === bloc.id ? { ...b, ...patch } : b)));

  const patchProps = (patch: Record<string, unknown>) =>
    bloc &&
    majBlocks(
      layout.blocks.map((b) =>
        b.id === bloc.id ? { ...b, props: { ...(b.props as object), ...patch } } : b,
      ),
    );

  const ajouter = (type: BlockType) => {
    const b = nouveauBloc(type, layout.blocks.length);
    majBlocks([...layout.blocks, b]);
    setSelection(b.id);
  };

  const enregistrer = () =>
    void run(async () => {
      await api.put(`${base}/studio/templates/${id}/variants/${formatId}`, { layout });
      setModifie(false);
      await template.reload();
    });

  const ajouterFormat = (nouveauFormatId: string) =>
    void run(async () => {
      await api.post(`${base}/studio/templates/${id}/variants`, { format_id: nouveauFormatId });
      await template.reload();
      setFormatId(nouveauFormatId);
    });

  const formatsRestants =
    formats.data?.filter((f) => !template.data?.variants.some((v) => v.format_id === f.id)) ?? [];

  const donnees: FieldData = fields.data ?? DONNEES_EXEMPLE;

  return (
    <>
      <header className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs text-ink-soft">
            <Link to="/studio" className="underline">
              Studio
            </Link>{" "}
            / gabarit
          </p>
          <h1 className="text-2xl font-semibold tracking-tight">{template.data?.name}</h1>
          <p className="mt-1 text-sm text-ink-soft">
            v{template.data?.version} · {variante?.width} × {variante?.height} ({variante?.ratio})
            {variante?.is_master && " · format maitre"}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {modifie && <Badge tone="warn">non enregistre</Badge>}
          {isAdmin && (
            <Button variant="primary" disabled={busy || !modifie} onClick={enregistrer}>
              Enregistrer
            </Button>
          )}
        </div>
      </header>

      <ErrorNote>{template.error ?? brand.error ?? error}</ErrorNote>

      <nav className="mb-4 flex flex-wrap items-center gap-1.5">
        {template.data?.variants.map((v) => (
          <button
            key={v.format_id}
            onClick={() => setFormatId(v.format_id)}
            className={`rounded-lg px-3 py-1.5 text-sm ${
              v.format_id === formatId ? "bg-ink text-paper" : "border border-line"
            }`}
          >
            {v.format_key} <span className="opacity-70">{v.ratio}</span>
            {v.is_master && " ★"}
          </button>
        ))}
        {isAdmin && formatsRestants.length > 0 && (
          <Select
            className="w-auto"
            value=""
            onChange={(e) => e.target.value && ajouterFormat(e.target.value)}
          >
            <option value="">+ decliner…</option>
            {formatsRestants.map((f) => (
              <option key={f.id} value={f.id}>
                {f.platform} — {f.label} ({f.ratio})
              </option>
            ))}
          </Select>
        )}
      </nav>

      <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
        <div>
          <div className="mb-3 flex flex-wrap items-center gap-1.5">
            {isAdmin &&
              (Object.keys(NOMS_BLOCS) as BlockType[])
                .filter((t) => t !== "group")
                .map((t) => (
                  <Button key={t} size="sm" onClick={() => ajouter(t)}>
                    + {NOMS_BLOCS[t]}
                  </Button>
                ))}
            <label className="ml-auto flex items-center gap-1.5 text-xs">
              <input
                type="checkbox"
                checked={magnetisme}
                onChange={(e) => setMagnetisme(e.target.checked)}
              />
              magnetisme
            </label>
            <label className="flex items-center gap-1.5 text-xs">
              <input
                type="checkbox"
                checked={zonesSures}
                onChange={(e) => setZonesSures(e.target.checked)}
              />
              zones sures
            </label>
          </div>

          <Canvas
            layout={layout}
            brand={charte}
            data={donnees}
            media={media}
            selection={selection}
            onSelect={setSelection}
            onBlocks={majBlocks}
            magnetisme={magnetisme}
            zonesSures={zonesSures}
            lectureSeule={!isAdmin}
          />

          <div className="mt-3">
            <Field label="Apercu avec un evenement" hint="Les champs automatiques se remplissent.">
              <Select value={eventId} onChange={(e) => setEventId(e.target.value)}>
                <option value="">donnees d'exemple</option>
                {events.data?.map((ev) => (
                  <option key={ev.id} value={ev.id}>
                    {ev.title}
                  </option>
                ))}
              </Select>
            </Field>
          </div>
        </div>

        <div className="space-y-4">
          <Card title="Fond">
            <FondEditeur
              background={layout.background ?? null}
              tokens={tokens}
              assets={assets.data ?? []}
              onChange={(background) => {
                setLayout({ ...layout, background });
                setModifie(true);
              }}
            />
          </Card>

          <Card title="Calques">
            <ul className="space-y-1">
              {[...layout.blocks]
                .sort((a, b) => (b.z ?? 0) - (a.z ?? 0))
                .map((b) => (
                  <li key={b.id}>
                    <button
                      onClick={() => setSelection(b.id)}
                      className={`flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm ${
                        b.id === selection ? "bg-ink text-paper" : "hover:bg-paper"
                      }`}
                    >
                      <span className="truncate">
                        {NOMS_BLOCS[b.type]}
                        {b.type === "text" &&
                          ` — ${String((b.props as { content?: string }).content ?? "").slice(0, 18)}`}
                      </span>
                      {b.locked && <span className="text-xs">verrouille</span>}
                    </button>
                  </li>
                ))}
            </ul>
          </Card>

          {bloc && (
            <Card title={NOMS_BLOCS[bloc.type]}>
              <Inspector
                block={bloc}
                tokens={tokens}
                assets={assets.data ?? []}
                onChange={patchBloc}
                onProps={patchProps}
                onDelete={() => {
                  majBlocks(layout.blocks.filter((b) => b.id !== bloc.id));
                  setSelection(null);
                }}
                onDuplicate={() => {
                  const copie = dupliquer(bloc);
                  majBlocks([...layout.blocks, copie]);
                  setSelection(copie.id);
                }}
                onLayer={(sens) => majBlocks(deplacerCalque(layout.blocks, bloc.id, sens))}
              />
            </Card>
          )}
        </div>
      </div>
    </>
  );
};

export const FondEditeur: React.FC<{
  background: Background | null;
  tokens: { kind: string; key: string; label: string }[];
  assets: Asset[];
  onChange: (background: Background | null) => void;
}> = ({ background, tokens, assets, onChange }) => {
  const couleurs = tokens.filter((t) => t.kind === "color");

  return (
    <div className="space-y-2">
      <Field label="Type">
        <Select
          value={background?.type ?? ""}
          onChange={(e) => {
            const type = e.target.value as Background["type"] | "";
            if (!type) return onChange(null);
            if (type === "color") return onChange({ type, token: couleurs[0]?.key });
            if (type === "gradient")
              return onChange({
                type,
                fromToken: couleurs[0]?.key,
                toToken: couleurs[1]?.key,
                angle: 180,
              });
            return onChange({ type });
          }}
        >
          <option value="">transparent</option>
          <option value="color">couleur</option>
          <option value="gradient">degrade</option>
          <option value="image">image</option>
        </Select>
      </Field>

      {background?.type === "color" && (
        <Field label="Couleur">
          <Select
            value={background.token ?? ""}
            onChange={(e) => onChange({ ...background, token: e.target.value })}
          >
            {couleurs.map((t) => (
              <option key={t.key} value={t.key}>
                {t.label}
              </option>
            ))}
          </Select>
        </Field>
      )}

      {background?.type === "gradient" && (
        <div className="grid grid-cols-2 gap-2">
          <Field label="De">
            <Select
              value={background.fromToken ?? ""}
              onChange={(e) => onChange({ ...background, fromToken: e.target.value })}
            >
              {couleurs.map((t) => (
                <option key={t.key} value={t.key}>
                  {t.label}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Vers">
            <Select
              value={background.toToken ?? ""}
              onChange={(e) => onChange({ ...background, toToken: e.target.value })}
            >
              {couleurs.map((t) => (
                <option key={t.key} value={t.key}>
                  {t.label}
                </option>
              ))}
            </Select>
          </Field>
        </div>
      )}

      {background?.type === "image" && (
        <Field label="Image">
          <Select
            value={background.assetId ?? ""}
            onChange={(e) => onChange({ ...background, assetId: e.target.value })}
          >
            <option value="">— a choisir —</option>
            {assets
              .filter((a) => a.kind === "image")
              .map((a) => (
                <option key={a.id} value={a.id}>
                  {a.filename}
                </option>
              ))}
          </Select>
        </Field>
      )}
    </div>
  );
};

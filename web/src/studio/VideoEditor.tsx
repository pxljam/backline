import React, { useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Player } from "@remotion/player";
import type { Block, BlockType, FieldData, Layout, Scene, VideoSpec } from "@backline/layout";
import { VideoComposition, totalDuration } from "@backline/layout";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase } from "../lib/session";
import type { Asset, BrandView, VideoCompositionRow } from "../lib/types";
import { Badge, Button, Card, ErrorNote, Field, Input, Loading, Select } from "../components/ui";
import { Canvas } from "./Canvas";
import { Inspector } from "./Inspector";
import { BackgroundEditor } from "./TemplateEditor";
import { toBrand, useMediaMap } from "./brand";
import { SAMPLE_DATA } from "./fields";
import { BLOCK_NAMES, moveLayer, duplicateBlock, newBlock } from "./blocks";

/**
 * Video editor (§10.2): **a video is described entirely in the app and
 * previewed in the browser without starting any encoding**. The render itself
 * goes into a queue that machines claim from (§10.3).
 */
export const VideoEditor: React.FC = () => {
  const { id = "" } = useParams();
  const base = useCollectiveBase();

  const comp = useResource<VideoCompositionRow>(`${base}/video-compositions/${id}`, [id]);
  const assets = useResource<Asset[]>(`${base}/studio/assets`);
  const { run, busy, error } = useAction();

  const [spec, setSpec] = useState<VideoSpec | null>(null);
  const [scene, setScene] = useState(0);
  const [selection, setSelection] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [snap, setSnap] = useState(true);
  const [queueNotice, setQueueNotice] = useState<string | null>(null);

  const brand = useResource<BrandView>(
    comp.data
      ? `${base}/studio/brand${comp.data.group_id ? `?group_id=${comp.data.group_id}` : ""}`
      : null,
    [comp.data?.group_id],
  );
  const fields = useResource<FieldData>(
    comp.data?.event_id ? `${base}/studio/preview-fields/${comp.data.event_id}` : null,
    [comp.data?.event_id],
  );

  useEffect(() => {
    if (comp.data) {
      setSpec(structuredClone(comp.data.spec));
      setDirty(false);
    }
  }, [comp.data]);

  const assetIds = useMemo(() => {
    const ids: string[] = [];
    for (const s of spec?.scenes ?? []) {
      if (s.background?.assetId) ids.push(s.background.assetId);
      for (const b of s.blocks) {
        const a = (b.props as { assetId?: string }).assetId;
        if (a) ids.push(a);
      }
    }
    if (spec?.audio?.assetId) ids.push(spec.audio.assetId);
    for (const token of brand.data?.tokens ?? []) {
      const value = token.value as { assetId?: string };
      if (token.kind === "logo" && value.assetId) ids.push(value.assetId);
    }
    return ids;
  }, [spec, brand.data]);

  const media = useMediaMap(base, assetIds);
  const tokens = brand.data ? [...(brand.data.inherited ?? []), ...brand.data.tokens] : [];
  const brandTokens = brand.data ? toBrand(brand.data.tokens, brand.data.inherited) : {};
  const data: FieldData = fields.data ?? SAMPLE_DATA;

  if (comp.loading || !spec) return <Loading />;

  const current: Scene | undefined = spec.scenes[scene];
  const block = current?.blocks.find((b) => b.id === selection) ?? null;
  const duration = totalDuration(spec);

  const updateSpec = (next: VideoSpec) => {
    setSpec(next);
    setDirty(true);
  };

  const updateScene = (patch: Partial<Scene>) =>
    updateSpec({
      ...spec,
      scenes: spec.scenes.map((s, i) => (i === scene ? { ...s, ...patch } : s)),
    });

  const updateBlocks = (blocks: Block[]) => updateScene({ blocks });

  const addScene = () =>
    updateSpec({
      ...spec,
      scenes: [
        ...spec.scenes,
        {
          id: `s${Date.now().toString(36)}`,
          durationInFrames: spec.fps * 3,
          transition: { type: "cut" },
          background: null,
          blocks: [],
        },
      ],
    });

  const addBlock = (type: BlockType) => {
    if (!current) return;
    const b = newBlock(type, current.blocks.length);
    updateBlocks([...current.blocks, b]);
    setSelection(b.id);
  };

  const save = () =>
    void run(async () => {
      await api.put(`${base}/video-compositions/${id}`, { spec });
      setDirty(false);
      await comp.reload();
    });

  const startRender = () =>
    void run(async () => {
      if (dirty) await api.put(`${base}/video-compositions/${id}`, { spec });
      const r = await api.post<{ id: string; machines_online: number }>(
        `${base}/video-compositions/${id}/render`,
      );
      setDirty(false);
      setQueueNotice(
        r.machines_online > 0
          ? "Rendu en file — une machine va le reclamer."
          : "Rendu en file, mais aucune machine connectee : un admin sera prevenu.",
      );
    });

  const layout: Layout | null = current
    ? {
        version: spec.version,
        width: spec.width,
        height: spec.height,
        background: current.background ?? null,
        blocks: current.blocks,
        fps: spec.fps,
      }
    : null;

  return (
    <>
      <header className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs text-ink-soft">
            <Link to="/studio" className="underline">
              Studio
            </Link>{" "}
            / video
          </p>
          <h1 className="text-2xl font-semibold tracking-tight">{comp.data?.name}</h1>
          <p className="mt-1 text-sm text-ink-soft">
            {spec.width} × {spec.height} · {spec.fps} img/s · {spec.scenes.length} plan(s) ·{" "}
            {(duration / spec.fps).toFixed(1)} s · v{comp.data?.version}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {dirty && <Badge tone="warn">non enregistre</Badge>}
          <Button disabled={busy || !dirty} onClick={save}>
            Enregistrer
          </Button>
          <Button variant="primary" disabled={busy || spec.scenes.length === 0} onClick={startRender}>
            Lancer le rendu
          </Button>
        </div>
      </header>

      <ErrorNote>{comp.error ?? brand.error ?? error}</ErrorNote>
      {queueNotice && (
        <p className="mb-4 rounded-lg border border-line bg-panel px-3 py-2 text-sm">
          {queueNotice}{" "}
          <Link to="/rendus" className="underline">
            suivre la file
          </Link>
        </p>
      )}

      <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
        <div>
          {/* Timeline: the scenes in order, each with its duration. */}
          <div className="mb-3 flex flex-wrap items-center gap-1.5">
            {spec.scenes.map((s, i) => (
              <button
                key={s.id}
                onClick={() => {
                  setScene(i);
                  setSelection(null);
                }}
                className={`rounded-lg px-3 py-1.5 text-sm ${
                  i === scene ? "bg-ink text-paper" : "border border-line"
                }`}
              >
                plan {i + 1}
                <span className="ml-1.5 opacity-70">{(s.durationInFrames / spec.fps).toFixed(1)}s</span>
              </button>
            ))}
            <Button size="sm" onClick={addScene}>
              + plan
            </Button>
          </div>

          {layout && current ? (
            <>
              <div className="mb-3 flex flex-wrap items-center gap-1.5">
                {(Object.keys(BLOCK_NAMES) as BlockType[])
                  .filter((t) => t !== "group")
                  .map((t) => (
                    <Button key={t} size="sm" onClick={() => addBlock(t)}>
                      + {BLOCK_NAMES[t]}
                    </Button>
                  ))}
                <label className="ml-auto flex items-center gap-1.5 text-xs">
                  <input
                    type="checkbox"
                    checked={snap}
                    onChange={(e) => setSnap(e.target.checked)}
                  />
                  magnetisme
                </label>
              </div>

              <Canvas
                layout={layout}
                brand={brandTokens}
                data={data}
                media={media}
                selection={selection}
                onSelect={setSelection}
                onBlocks={updateBlocks}
                snap={snap}
                safeAreas={false}
              />
            </>
          ) : (
            <Card>
              <p className="py-6 text-center text-sm text-ink-soft">
                Aucun plan : ajoutez-en un pour commencer.
              </p>
            </Card>
          )}

          <div className="mt-4">
            <Card title="Apercu">
              {duration > 0 ? (
                <Player
                  component={VideoComposition}
                  inputProps={{ spec, brand: brandTokens, data: data, media }}
                  durationInFrames={duration}
                  fps={spec.fps}
                  compositionWidth={spec.width}
                  compositionHeight={spec.height}
                  style={{ width: "100%" }}
                  controls
                />
              ) : (
                <p className="py-6 text-center text-sm text-ink-soft">
                  L'apercu se lit ici, dans le navigateur : aucun encodage n'est lance.
                </p>
              )}
            </Card>
          </div>
        </div>

        <div className="space-y-4">
          {current && (
            <Card title={`Plan ${scene + 1}`}>
              <div className="space-y-2">
                <Field label="Duree (secondes)">
                  <Input
                    type="number"
                    step={0.1}
                    min={0.1}
                    value={(current.durationInFrames / spec.fps).toFixed(1)}
                    onChange={(e) =>
                      updateScene({
                        durationInFrames: Math.max(1, Math.round(Number(e.target.value) * spec.fps)),
                      })
                    }
                  />
                </Field>
                <Field label="Transition d'entree">
                  <Select
                    value={current.transition?.type ?? "cut"}
                    onChange={(e) =>
                      updateScene({
                        transition: {
                          type: e.target.value as "cut" | "fade" | "slide",
                          durationInFrames: current.transition?.durationInFrames ?? 12,
                        },
                      })
                    }
                  >
                    <option value="cut">franche</option>
                    <option value="fade">fondu</option>
                    <option value="slide">glissement</option>
                  </Select>
                </Field>

                <BackgroundEditor
                  background={current.background ?? null}
                  tokens={tokens}
                  assets={assets.data ?? []}
                  onChange={(background) => updateScene({ background })}
                />

                <div className="flex flex-wrap gap-1.5 pt-1">
                  <Button
                    size="sm"
                    onClick={() =>
                      updateSpec({
                        ...spec,
                        scenes: [
                          ...spec.scenes.slice(0, scene + 1),
                          { ...structuredClone(current), id: `s${Date.now().toString(36)}` },
                          ...spec.scenes.slice(scene + 1),
                        ],
                      })
                    }
                  >
                    dupliquer
                  </Button>
                  <Button
                    size="sm"
                    disabled={scene === 0}
                    onClick={() => {
                      const scenes = [...spec.scenes];
                      [scenes[scene - 1], scenes[scene]] = [scenes[scene], scenes[scene - 1]];
                      updateSpec({ ...spec, scenes });
                      setScene(scene - 1);
                    }}
                  >
                    avancer
                  </Button>
                  <Button
                    size="sm"
                    disabled={scene >= spec.scenes.length - 1}
                    onClick={() => {
                      const scenes = [...spec.scenes];
                      [scenes[scene + 1], scenes[scene]] = [scenes[scene], scenes[scene + 1]];
                      updateSpec({ ...spec, scenes });
                      setScene(scene + 1);
                    }}
                  >
                    reculer
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => {
                      updateSpec({ ...spec, scenes: spec.scenes.filter((_, i) => i !== scene) });
                      setScene(Math.max(0, scene - 1));
                      setSelection(null);
                    }}
                  >
                    supprimer
                  </Button>
                </div>
              </div>
            </Card>
          )}

          <Card title="Bande son">
            <div className="space-y-2">
              <Field label="Piste" hint="Un seul fichier pour toute la video.">
                <Select
                  value={spec.audio?.assetId ?? ""}
                  onChange={(e) =>
                    updateSpec({
                      ...spec,
                      audio: e.target.value ? { ...spec.audio, assetId: e.target.value } : null,
                    })
                  }
                >
                  <option value="">aucune</option>
                  {(assets.data ?? [])
                    .filter((a) => a.kind === "audio" || a.mime.startsWith("audio/"))
                    .map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.filename}
                      </option>
                    ))}
                </Select>
              </Field>
              {spec.audio && (
                <div className="grid grid-cols-2 gap-2">
                  <Field label="Debut (images)">
                    <Input
                      type="number"
                      value={spec.audio.startFrom ?? 0}
                      onChange={(e) =>
                        updateSpec({
                          ...spec,
                          audio: { ...spec.audio, startFrom: Number(e.target.value) },
                        })
                      }
                    />
                  </Field>
                  <Field label="Volume">
                    <Input
                      type="number"
                      step={0.1}
                      min={0}
                      max={1}
                      value={spec.audio.volume ?? 1}
                      onChange={(e) =>
                        updateSpec({
                          ...spec,
                          audio: { ...spec.audio, volume: Number(e.target.value) },
                        })
                      }
                    />
                  </Field>
                </div>
              )}
            </div>
          </Card>

          {current && (
            <Card title="Calques">
              <ul className="space-y-1">
                {[...current.blocks]
                  .sort((a, b) => (b.z ?? 0) - (a.z ?? 0))
                  .map((b) => (
                    <li key={b.id}>
                      <button
                        onClick={() => setSelection(b.id)}
                        className={`flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm ${
                          b.id === selection ? "bg-ink text-paper" : "hover:bg-paper"
                        }`}
                      >
                        <span className="truncate">{BLOCK_NAMES[b.type]}</span>
                      </button>
                    </li>
                  ))}
              </ul>
            </Card>
          )}

          {block && current && (
            <Card title={BLOCK_NAMES[block.type]}>
              <Inspector
                block={block}
                tokens={tokens}
                assets={assets.data ?? []}
                timeline
                onChange={(patch) =>
                  updateBlocks(current.blocks.map((b) => (b.id === block.id ? { ...b, ...patch } : b)))
                }
                onProps={(patch) =>
                  updateBlocks(
                    current.blocks.map((b) =>
                      b.id === block.id ? { ...b, props: { ...(b.props as object), ...patch } } : b,
                    ),
                  )
                }
                onDelete={() => {
                  updateBlocks(current.blocks.filter((b) => b.id !== block.id));
                  setSelection(null);
                }}
                onDuplicate={() => {
                  const copy = duplicateBlock(block);
                  updateBlocks([...current.blocks, copy]);
                  setSelection(copy.id);
                }}
                onLayer={(direction) => updateBlocks(moveLayer(current.blocks, block.id, direction))}
              />
            </Card>
          )}
        </div>
      </div>
    </>
  );
};

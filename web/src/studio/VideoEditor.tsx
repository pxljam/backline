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
import { FondEditeur } from "./TemplateEditor";
import { toBrand, useMediaMap } from "./brand";
import { DONNEES_EXEMPLE } from "./fields";
import { NOMS_BLOCS, deplacerCalque, dupliquer, nouveauBloc } from "./blocks";

/**
 * Editeur video (§10.2) : **une video se decrit entierement dans l'app et
 * s'apercoit dans le navigateur sans lancer d'encodage**. Le rendu, lui, part
 * dans une file que des machines reclament (§10.3).
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
  const [modifie, setModifie] = useState(false);
  const [magnetisme, setMagnetisme] = useState(true);
  const [file, setFile] = useState<string | null>(null);

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
      setModifie(false);
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
  const charte = brand.data ? toBrand(brand.data.tokens, brand.data.inherited) : {};
  const donnees: FieldData = fields.data ?? DONNEES_EXEMPLE;

  if (comp.loading || !spec) return <Loading />;

  const courant: Scene | undefined = spec.scenes[scene];
  const bloc = courant?.blocks.find((b) => b.id === selection) ?? null;
  const duree = totalDuration(spec);

  const majSpec = (suivant: VideoSpec) => {
    setSpec(suivant);
    setModifie(true);
  };

  const majScene = (patch: Partial<Scene>) =>
    majSpec({
      ...spec,
      scenes: spec.scenes.map((s, i) => (i === scene ? { ...s, ...patch } : s)),
    });

  const majBlocks = (blocks: Block[]) => majScene({ blocks });

  const ajouterPlan = () =>
    majSpec({
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

  const ajouterBloc = (type: BlockType) => {
    if (!courant) return;
    const b = nouveauBloc(type, courant.blocks.length);
    majBlocks([...courant.blocks, b]);
    setSelection(b.id);
  };

  const enregistrer = () =>
    void run(async () => {
      await api.put(`${base}/video-compositions/${id}`, { spec });
      setModifie(false);
      await comp.reload();
    });

  const lancerRendu = () =>
    void run(async () => {
      if (modifie) await api.put(`${base}/video-compositions/${id}`, { spec });
      const r = await api.post<{ id: string; machines_online: number }>(
        `${base}/video-compositions/${id}/render`,
      );
      setModifie(false);
      setFile(
        r.machines_online > 0
          ? "Rendu en file — une machine va le reclamer."
          : "Rendu en file, mais aucune machine connectee : un admin sera prevenu.",
      );
    });

  const layout: Layout | null = courant
    ? {
        version: spec.version,
        width: spec.width,
        height: spec.height,
        background: courant.background ?? null,
        blocks: courant.blocks,
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
            {(duree / spec.fps).toFixed(1)} s · v{comp.data?.version}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {modifie && <Badge tone="warn">non enregistre</Badge>}
          <Button disabled={busy || !modifie} onClick={enregistrer}>
            Enregistrer
          </Button>
          <Button variant="primary" disabled={busy || spec.scenes.length === 0} onClick={lancerRendu}>
            Lancer le rendu
          </Button>
        </div>
      </header>

      <ErrorNote>{comp.error ?? brand.error ?? error}</ErrorNote>
      {file && (
        <p className="mb-4 rounded-lg border border-line bg-panel px-3 py-2 text-sm">
          {file}{" "}
          <Link to="/rendus" className="underline">
            suivre la file
          </Link>
        </p>
      )}

      <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
        <div>
          {/* Timeline : les plans dans l'ordre, chacun avec sa duree. */}
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
            <Button size="sm" onClick={ajouterPlan}>
              + plan
            </Button>
          </div>

          {layout && courant ? (
            <>
              <div className="mb-3 flex flex-wrap items-center gap-1.5">
                {(Object.keys(NOMS_BLOCS) as BlockType[])
                  .filter((t) => t !== "group")
                  .map((t) => (
                    <Button key={t} size="sm" onClick={() => ajouterBloc(t)}>
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
                zonesSures={false}
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
              {duree > 0 ? (
                <Player
                  component={VideoComposition}
                  inputProps={{ spec, brand: charte, data: donnees, media }}
                  durationInFrames={duree}
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
          {courant && (
            <Card title={`Plan ${scene + 1}`}>
              <div className="space-y-2">
                <Field label="Duree (secondes)">
                  <Input
                    type="number"
                    step={0.1}
                    min={0.1}
                    value={(courant.durationInFrames / spec.fps).toFixed(1)}
                    onChange={(e) =>
                      majScene({
                        durationInFrames: Math.max(1, Math.round(Number(e.target.value) * spec.fps)),
                      })
                    }
                  />
                </Field>
                <Field label="Transition d'entree">
                  <Select
                    value={courant.transition?.type ?? "cut"}
                    onChange={(e) =>
                      majScene({
                        transition: {
                          type: e.target.value as "cut" | "fade" | "slide",
                          durationInFrames: courant.transition?.durationInFrames ?? 12,
                        },
                      })
                    }
                  >
                    <option value="cut">franche</option>
                    <option value="fade">fondu</option>
                    <option value="slide">glissement</option>
                  </Select>
                </Field>

                <FondEditeur
                  background={courant.background ?? null}
                  tokens={tokens}
                  assets={assets.data ?? []}
                  onChange={(background) => majScene({ background })}
                />

                <div className="flex flex-wrap gap-1.5 pt-1">
                  <Button
                    size="sm"
                    onClick={() =>
                      majSpec({
                        ...spec,
                        scenes: [
                          ...spec.scenes.slice(0, scene + 1),
                          { ...structuredClone(courant), id: `s${Date.now().toString(36)}` },
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
                      majSpec({ ...spec, scenes });
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
                      majSpec({ ...spec, scenes });
                      setScene(scene + 1);
                    }}
                  >
                    reculer
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => {
                      majSpec({ ...spec, scenes: spec.scenes.filter((_, i) => i !== scene) });
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
                    majSpec({
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
                        majSpec({
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
                        majSpec({
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

          {courant && (
            <Card title="Calques">
              <ul className="space-y-1">
                {[...courant.blocks]
                  .sort((a, b) => (b.z ?? 0) - (a.z ?? 0))
                  .map((b) => (
                    <li key={b.id}>
                      <button
                        onClick={() => setSelection(b.id)}
                        className={`flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm ${
                          b.id === selection ? "bg-ink text-paper" : "hover:bg-paper"
                        }`}
                      >
                        <span className="truncate">{NOMS_BLOCS[b.type]}</span>
                      </button>
                    </li>
                  ))}
              </ul>
            </Card>
          )}

          {bloc && courant && (
            <Card title={NOMS_BLOCS[bloc.type]}>
              <Inspector
                block={bloc}
                tokens={tokens}
                assets={assets.data ?? []}
                temporel
                onChange={(patch) =>
                  majBlocks(courant.blocks.map((b) => (b.id === bloc.id ? { ...b, ...patch } : b)))
                }
                onProps={(patch) =>
                  majBlocks(
                    courant.blocks.map((b) =>
                      b.id === bloc.id ? { ...b, props: { ...(b.props as object), ...patch } } : b,
                    ),
                  )
                }
                onDelete={() => {
                  majBlocks(courant.blocks.filter((b) => b.id !== bloc.id));
                  setSelection(null);
                }}
                onDuplicate={() => {
                  const copie = dupliquer(bloc);
                  majBlocks([...courant.blocks, copie]);
                  setSelection(copie.id);
                }}
                onLayer={(sens) => majBlocks(deplacerCalque(courant.blocks, bloc.id, sens))}
              />
            </Card>
          )}
        </div>
      </div>
    </>
  );
};

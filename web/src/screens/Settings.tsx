import React, { useEffect, useState } from "react";
import { api } from "../lib/api";
import { useAction, useResource } from "../lib/hooks";
import { useCollectiveBase, useSession } from "../lib/session";
import type { CollectiveDetail, EventType, Milestone } from "../lib/types";
import {
  Badge, Button, Card, Empty, ErrorNote, Field, Input, Loading, PageTitle, Select, Textarea,
} from "../components/ui";

/**
 * Reglages du collectif (§14) : types d'evenements et **jalons de com**, bot,
 * flux iCal, sauvegardes. Les timelines sont de la donnee : les modifier se
 * fait ici, jamais dans le code (§11.1).
 */
export const Settings: React.FC = () => {
  const [onglet, setOnglet] = useState<"types" | "bot" | "ical" | "sauvegardes">("types");

  return (
    <>
      <PageTitle
        title="Reglages"
        subtitle="Types d'evenements, jalons de com, bot, flux iCal, sauvegardes."
      />
      <nav className="mb-5 flex flex-wrap gap-1.5">
        {(
          [
            ["types", "Types et jalons"],
            ["bot", "Bot et notifications"],
            ["ical", "Flux iCal"],
            ["sauvegardes", "Sauvegardes"],
          ] as ["types" | "bot" | "ical" | "sauvegardes", string][]
        ).map(([key, label]) => (
          <button
            key={key}
            onClick={() => setOnglet(key)}
            className={`rounded-lg px-3 py-1.5 text-sm ${
              onglet === key ? "bg-ink text-paper" : "border border-line"
            }`}
          >
            {label}
          </button>
        ))}
      </nav>

      {onglet === "types" && <TypesEvenements />}
      {onglet === "bot" && <BotEtNotifications />}
      {onglet === "ical" && <FluxIcal />}
      {onglet === "sauvegardes" && <Sauvegardes />}
    </>
  );
};

// --- Types d'evenements et timelines de com --------------------------------

const TypesEvenements: React.FC = () => {
  const base = useCollectiveBase();
  const collective = useResource<CollectiveDetail>(base);
  const [ouvert, setOuvert] = useState<string | null>(null);

  if (collective.loading) return <Loading />;

  return (
    <>
      <ErrorNote>{collective.error}</ErrorNote>
      <div className="space-y-4">
        {collective.data?.event_types.map((t) => (
          <TypeCard
            key={t.id}
            type={t}
            ouvert={ouvert === t.id}
            onToggle={() => setOuvert(ouvert === t.id ? null : t.id)}
            onSaved={() => void collective.reload()}
          />
        ))}
      </div>
    </>
  );
};

const TypeCard: React.FC<{
  type: EventType;
  ouvert: boolean;
  onToggle: () => void;
  onSaved: () => void;
}> = ({ type, ouvert, onToggle, onSaved }) => {
  const base = useCollectiveBase();
  const { isAdmin } = useSession();
  const { run, busy, error } = useAction();
  const [label, setLabel] = useState(type.label);
  const [jalons, setJalons] = useState<Milestone[]>(type.comms_milestones ?? []);

  useEffect(() => {
    setLabel(type.label);
    setJalons(type.comms_milestones ?? []);
  }, [type]);

  const modifier = (index: number, patch: Partial<Milestone>) =>
    setJalons((liste) => liste.map((j, i) => (i === index ? { ...j, ...patch } : j)));

  const enregistrer = () =>
    void run(async () => {
      await api.patch(`${base}/event-types/${type.id}`, {
        label,
        comms_milestones: jalons,
      });
      onSaved();
    });

  return (
    <Card
      title={
        <span className="flex items-center gap-2">
          {type.label}
          <span className="font-normal normal-case text-ink-soft">{type.key}</span>
        </span>
      }
      action={
        <div className="flex items-center gap-2">
          {type.requires_venue && <Badge>lieu requis</Badge>}
          {type.is_range && <Badge>sur une plage</Badge>}
          <Badge tone="neutral">{jalons.length} jalon(s)</Badge>
          <Button size="sm" onClick={onToggle}>
            {ouvert ? "replier" : "jalons de com"}
          </Button>
        </div>
      }
    >
      {!ouvert ? (
        <p className="text-sm text-ink-soft">
          {jalons.length === 0
            ? "Aucun jalon : un evenement de ce type ne genere pas de plan de com."
            : jalons.map((j) => j.label).join(" · ")}
        </p>
      ) : (
        <>
          <ErrorNote>{error}</ErrorNote>

          <div className="mb-4 max-w-sm">
            <Field label="Nom affiche">
              <Input
                value={label}
                disabled={!isAdmin}
                onChange={(e) => setLabel(e.target.value)}
              />
            </Field>
          </div>

          <p className="mb-3 text-xs text-ink-soft">
            Le decalage se compte en jours par rapport au debut de l'evenement (negatif = avant).
            Une residence communique depuis sa fin : choisir l'ancre « fin ».
          </p>

          <ul className="space-y-3">
            {jalons.map((j, i) => (
              <li key={i} className="rounded-lg border border-line p-3">
                <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                  <Field label="Cle">
                    <Input
                      value={j.key}
                      disabled={!isAdmin}
                      onChange={(e) => modifier(i, { key: e.target.value })}
                    />
                  </Field>
                  <Field label="Intitule">
                    <Input
                      value={j.label}
                      disabled={!isAdmin}
                      onChange={(e) => modifier(i, { label: e.target.value })}
                    />
                  </Field>
                  <Field label="Jours">
                    <Input
                      type="number"
                      value={j.offset_days}
                      disabled={!isAdmin}
                      onChange={(e) => modifier(i, { offset_days: Number(e.target.value) })}
                    />
                  </Field>
                  <Field label="Minutes" hint="« J0 moins 15 min »">
                    <Input
                      type="number"
                      value={j.offset_minutes}
                      disabled={!isAdmin}
                      onChange={(e) => modifier(i, { offset_minutes: Number(e.target.value) })}
                    />
                  </Field>
                  <Field label="Heure locale">
                    <Input
                      placeholder="18:00"
                      value={j.at ?? ""}
                      disabled={!isAdmin}
                      onChange={(e) => modifier(i, { at: e.target.value || null })}
                    />
                  </Field>
                  <Field label="Ancre">
                    <Select
                      value={j.anchor}
                      disabled={!isAdmin}
                      onChange={(e) =>
                        modifier(i, { anchor: e.target.value as Milestone["anchor"] })
                      }
                    >
                      <option value="start">debut</option>
                      <option value="end">fin</option>
                    </Select>
                  </Field>
                  <div className="lg:col-span-2">
                    <Field label="Formats" hint="Cles du catalogue, separees par des virgules.">
                      <Input
                        value={j.formats.join(", ")}
                        disabled={!isAdmin}
                        onChange={(e) =>
                          modifier(i, {
                            formats: e.target.value
                              .split(",")
                              .map((s) => s.trim())
                              .filter(Boolean),
                          })
                        }
                      />
                    </Field>
                  </div>
                  <div className="sm:col-span-2 lg:col-span-4">
                    <Field label="Legende par defaut" hint="Un humain la relit toujours.">
                      <Textarea
                        rows={2}
                        value={j.caption}
                        disabled={!isAdmin}
                        onChange={(e) => modifier(i, { caption: e.target.value })}
                      />
                    </Field>
                  </div>
                </div>
                {isAdmin && (
                  <div className="mt-2 text-right">
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => setJalons(jalons.filter((_, k) => k !== i))}
                    >
                      supprimer ce jalon
                    </Button>
                  </div>
                )}
              </li>
            ))}
          </ul>

          {isAdmin && (
            <div className="mt-4 flex flex-wrap gap-2">
              <Button
                onClick={() =>
                  setJalons([
                    ...jalons,
                    {
                      key: `j-${jalons.length + 1}`,
                      label: "Nouveau jalon",
                      offset_days: -7,
                      offset_minutes: 0,
                      at: "18:00",
                      formats: [],
                      caption: "",
                      anchor: "start",
                    },
                  ])
                }
              >
                ajouter un jalon
              </Button>
              <Button variant="primary" disabled={busy} onClick={enregistrer}>
                Enregistrer
              </Button>
            </div>
          )}

          <p className="mt-3 text-xs text-ink-soft">
            Les evenements deja confirmes gardent le plan instancie a leur confirmation.
          </p>
        </>
      )}
    </Card>
  );
};

// --- Bot Telegram et notifications personnelles ----------------------------

interface PublicConfig {
  telegram_bot_username: string | null;
  telegram_enabled: boolean;
  public_base_url: string;
}

const TYPES_NOTIFICATION: [string, string][] = [
  ["poll_open", "Sondage de dispos ouvert"],
  ["lineup_retained", "Line-up retenu"],
  ["lineup_not_retained", "Line-up non retenu"],
  ["logistics_vacant", "Poste logistique vacant"],
  ["tech_rider_missing", "Fiche technique manquante"],
  ["run_sheet", "Feuille de route de la veille"],
  ["publication_due", "Tache de com a publier"],
  ["publication_assigned", "Tache de com assignee"],
];

const BotEtNotifications: React.FC = () => {
  const { me } = useSession();
  const { data: config } = useResource<PublicConfig>("/api/config");
  const { run, busy, error } = useAction();
  const [from, setFrom] = useState("22");
  const [to, setTo] = useState("8");
  const [optOut, setOptOut] = useState<string[]>([]);
  const [enregistre, setEnregistre] = useState(false);

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card title="Bot Telegram">
        {config?.telegram_enabled ? (
          <>
            <p className="text-sm">
              Bot actif :{" "}
              <a
                className="underline"
                href={`https://t.me/${config.telegram_bot_username ?? ""}`}
                target="_blank"
                rel="noreferrer"
              >
                @{config.telegram_bot_username}
              </a>
            </p>
            <p className="mt-2 text-sm">
              {me?.telegram_linked ? (
                <Badge tone="good">votre compte est lie</Badge>
              ) : (
                <>
                  <Badge tone="warn">compte non lie</Badge>
                  <span className="ml-2 text-ink-soft">
                    Envoyez <code>/start</code> au bot avec le code de votre invitation.
                  </span>
                </>
              )}
            </p>
          </>
        ) : (
          <p className="text-sm text-ink-soft">
            Aucun jeton de bot sur cette instance : les notifications restent dans le centre web,
            qui est un doublon complet du bot.
          </p>
        )}
        <ul className="mt-3 space-y-1 text-xs text-ink-soft">
          <li>
            <code>/dispos</code> — repondre au sondage en cours
          </li>
          <li>
            <code>/agenda</code> — les prochains evenements
          </li>
          <li>
            <code>/postes</code> — prendre un poste vacant
          </li>
          <li>
            <code>/publier</code> — mes taches de com
          </li>
          <li>
            <code>/fiche</code> — envoyer une fiche technique
          </li>
          <li>
            <code>/collectif</code> — basculer de collectif
          </li>
        </ul>
      </Card>

      <Card title="Mes notifications">
        <ErrorNote>{error}</ErrorNote>
        <form
          className="space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            void run(async () => {
              await api.put("/api/notifications/preferences", {
                quiet_from: from === "" ? null : Number(from),
                quiet_to: to === "" ? null : Number(to),
                opt_out: optOut,
              });
              setEnregistre(true);
            });
          }}
        >
          <div className="flex gap-3">
            <Field label="Silence de">
              <Input
                type="number"
                min={0}
                max={23}
                value={from}
                onChange={(e) => setFrom(e.target.value)}
              />
            </Field>
            <Field label="jusqu'a">
              <Input
                type="number"
                min={0}
                max={23}
                value={to}
                onChange={(e) => setTo(e.target.value)}
              />
            </Field>
          </div>
          <p className="text-xs text-ink-soft">
            Les alertes critiques — « on est en ligne », alerte admin, publication ratee —
            traversent le silence nocturne.
          </p>

          <fieldset>
            <legend className="mb-1 text-xs font-medium uppercase tracking-wide text-ink-soft">
              Ne plus recevoir
            </legend>
            <ul className="space-y-1">
              {TYPES_NOTIFICATION.map(([key, label]) => (
                <li key={key}>
                  <label className="flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={optOut.includes(key)}
                      onChange={(e) =>
                        setOptOut((liste) =>
                          e.target.checked ? [...liste, key] : liste.filter((k) => k !== key),
                        )
                      }
                    />
                    {label}
                  </label>
                </li>
              ))}
            </ul>
          </fieldset>

          <div className="flex items-center gap-3">
            <Button type="submit" variant="primary" disabled={busy}>
              Enregistrer
            </Button>
            {enregistre && !busy && <span className="text-xs text-ink-soft">enregistre</span>}
          </div>
        </form>
      </Card>
    </div>
  );
};

// --- Flux iCal --------------------------------------------------------------

const FluxIcal: React.FC = () => {
  const base = useCollectiveBase();
  const feeds = useResource<{ scope: string; label: string; url: string }[]>(
    `${base}/calendar/feeds`,
  );

  if (feeds.loading) return <Loading />;

  const libelle: Record<string, string> = {
    collective: "collectif",
    user: "personnel",
    group: "groupe",
  };

  return (
    <Card title="Flux iCal">
      <ErrorNote>{feeds.error}</ErrorNote>
      <p className="mb-3 text-sm text-ink-soft">
        A coller dans Google Calendar ou Apple Calendrier. Le flux personnel ne doit pas etre
        partage : il porte votre jeton.
      </p>
      {!feeds.data || feeds.data.length === 0 ? (
        <Empty>Aucun flux.</Empty>
      ) : (
        <ul className="space-y-2">
          {feeds.data.map((f) => (
            <li key={f.url} className="flex flex-wrap items-center gap-2">
              <Badge>{libelle[f.scope] ?? f.scope}</Badge>
              <span className="text-sm font-medium">{f.label}</span>
              <code className="min-w-0 flex-1 truncate rounded bg-paper px-2 py-1 text-xs">
                {f.url}
              </code>
              <Button size="sm" onClick={() => void navigator.clipboard?.writeText(f.url)}>
                copier
              </Button>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
};

// --- Sauvegardes ------------------------------------------------------------

const Sauvegardes: React.FC = () => (
  <Card title="Sauvegardes">
    <p className="text-sm">
      La sauvegarde est une tache d'exploitation, pas un bouton dans l'application : un
      <code className="mx-1">pg_dump</code> quotidien et la synchronisation du bucket vers un
      stockage objet distant, avec retention et <strong>restauration testee</strong> (§15).
    </p>
    <ul className="mt-3 space-y-1 text-sm text-ink-soft">
      <li>
        base : <code>mise run db:dump</code> — depose une archive horodatee dans{" "}
        <code>infra/backups/</code>
      </li>
      <li>
        medias : <code>mise run storage:sync</code> — miroir du bucket
      </li>
      <li>
        restauration : <code>mise run db:restore -- infra/backups/&lt;archive&gt;.sql.gz</code>
      </li>
    </ul>
    <p className="mt-3 text-xs text-ink-soft">
      Une sauvegarde dont la restauration n'a jamais ete testee n'est pas une sauvegarde.
    </p>
  </Card>
);

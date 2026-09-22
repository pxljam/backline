-- Etapes 3 et 4 du §17 : evenements, logistique, calendrier, streams.

CREATE TABLE event_types (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    key           TEXT NOT NULL,          -- dj_night | concert | residency | stream
    label         TEXT NOT NULL,
    -- jalons de com instancies a la confirmation (§11.1), entierement editables
    comms_milestones JSONB NOT NULL DEFAULT '[]',
    default_logistics_slots JSONB NOT NULL DEFAULT '[]',
    requires_venue BOOLEAN NOT NULL DEFAULT TRUE,
    is_range       BOOLEAN NOT NULL DEFAULT FALSE,   -- residence : une plage, un seul evenement
    position       INT NOT NULL DEFAULT 0,
    UNIQUE (collective_id, key)
);

CREATE TABLE events (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id  UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    event_type_id  UUID NOT NULL REFERENCES event_types(id),
    opportunity_id UUID REFERENCES opportunities(id) ON DELETE SET NULL,
    venue_id       UUID REFERENCES venues(id) ON DELETE SET NULL,
    -- porteur : le collectif (NULL) ou un groupe. Meme objet, un seul champ.
    host_group_id  UUID REFERENCES groups(id) ON DELETE SET NULL,
    title          TEXT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'confirmed'
                   CHECK (status IN ('draft', 'confirmed', 'past', 'cancelled')),
    starts_at      TIMESTAMPTZ NOT NULL,
    ends_at        TIMESTAMPTZ,
    doors_at       TIMESTAMPTZ,
    soundcheck_at  TIMESTAMPTZ,
    -- creneaux du line-up : facultatifs, et leur publication est un reglage distinct
    set_times_state  TEXT NOT NULL DEFAULT 'undefined'
                     CHECK (set_times_state IN ('undefined', 'to_confirm', 'defined')),
    set_times_public BOOLEAN NOT NULL DEFAULT FALSE,
    capacity       INT,
    notes          TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX events_collective_idx ON events(collective_id, starts_at);

CREATE TABLE participations (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id   UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    group_id   UUID REFERENCES groups(id) ON DELETE CASCADE,
    user_id    UUID REFERENCES users(id) ON DELETE CASCADE,
    status     TEXT NOT NULL DEFAULT 'confirmed'
               CHECK (status IN ('invited', 'confirmed', 'declined', 'standby')),
    stage_role TEXT,
    slot_start TIME,
    slot_end   TIME,
    position   INT NOT NULL DEFAULT 0,
    acknowledged_at TIMESTAMPTZ,
    CHECK (group_id IS NOT NULL OR user_id IS NOT NULL)
);
CREATE INDEX participations_event_idx ON participations(event_id);

-- Residence : « tu viens ? », pas « quand exactement ? » (§6)
CREATE TABLE residency_presences (
    id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id  UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    answer    TEXT NOT NULL CHECK (answer IN ('coming', 'not_coming', 'unsure')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (event_id, user_id)
);

-- Jours precis : facultatifs, jamais reclames.
CREATE TABLE residency_presence_days (
    presence_id UUID NOT NULL REFERENCES residency_presences(id) ON DELETE CASCADE,
    day         DATE NOT NULL,
    PRIMARY KEY (presence_id, day)
);

CREATE TABLE logistics_slots (
    id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id  UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    label     TEXT NOT NULL,          -- texte libre : « transport backline », « lumiere »…
    quantity  INT NOT NULL DEFAULT 1 CHECK (quantity > 0),
    notes     TEXT,
    position  INT NOT NULL DEFAULT 0
);
CREATE INDEX logistics_slots_event_idx ON logistics_slots(event_id);

CREATE TABLE logistics_assignments (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slot_id    UUID NOT NULL REFERENCES logistics_slots(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (slot_id, user_id)
);

-- Relances automatiques J-14 / J-7 / J-2 : trace de ce qui est deja parti.
CREATE TABLE logistics_reminders (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id   UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    milestone  TEXT NOT NULL,          -- j-14 | j-7 | j-2
    sent_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    recipients INT NOT NULL DEFAULT 0,
    UNIQUE (event_id, milestone)
);

CREATE TABLE event_streams (
    event_id         UUID PRIMARY KEY REFERENCES events(id) ON DELETE CASCADE,
    capture_location TEXT,
    planned_duration_min INT,
    replay_url       TEXT,
    live_alert_sent_at TIMESTAMPTZ
);

CREATE TABLE event_stream_platforms (
    id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,     -- twitch | youtube | instagram | tiktok
    url      TEXT
);

-- Flux iCal a jeton secret : un par membre, par groupe, par collectif (§7)
CREATE TABLE ical_tokens (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope      TEXT NOT NULL CHECK (scope IN ('user', 'group', 'collective')),
    scope_id   UUID NOT NULL,
    token      TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (scope, scope_id)
);

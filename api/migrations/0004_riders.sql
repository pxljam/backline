-- Etape 5 du §17 : fiches techniques versionnees, press kit, comptes sociaux.

CREATE TABLE tech_riders (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id   UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    version    INT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published')),
    -- sections structurees : identite, line-up scene, plan de scene, input list,
    -- backline, son, lumiere, loges, arrivee, contacts techniques (§12)
    data       JSONB NOT NULL DEFAULT '{}',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (group_id, version)
);

-- L'evenement pointe vers la version reellement envoyee au lieu.
CREATE TABLE event_tech_riders (
    event_id      UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    group_id      UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    tech_rider_id UUID REFERENCES tech_riders(id) ON DELETE SET NULL,
    sent_at       TIMESTAMPTZ,
    sent_to       TEXT,
    PRIMARY KEY (event_id, group_id)
);

CREATE TABLE press_kits (
    id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id  UUID NOT NULL UNIQUE REFERENCES groups(id) ON DELETE CASCADE,
    bio_short TEXT,
    bio_long  TEXT,
    links     JSONB NOT NULL DEFAULT '[]',
    photo_asset_ids UUID[] NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- L'app recense les acces, jamais les mots de passe (§11.3).
CREATE TABLE social_accounts (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    group_id      UUID REFERENCES groups(id) ON DELETE CASCADE,
    platform      TEXT NOT NULL,   -- instagram | tiktok | youtube | facebook | twitch
    handle        TEXT NOT NULL,
    url           TEXT,
    mode          TEXT NOT NULL DEFAULT 'shared' CHECK (mode IN ('shared', 'personal')),
    vault_url     TEXT,            -- lien vers le gestionnaire externe, jamais le secret
    notes         TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX social_accounts_collective_idx ON social_accounts(collective_id);

-- « Qui peut publier sur ce compte ? » — la seule question qui bloque le jour J.
CREATE TABLE social_account_access (
    social_account_id UUID NOT NULL REFERENCES social_accounts(id) ON DELETE CASCADE,
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (social_account_id, user_id)
);

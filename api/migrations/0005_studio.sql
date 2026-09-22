-- Step 6 of §17: brand, format catalogue, templates, assets.

-- One brand per collective, partially overridden per group.
CREATE TABLE brands (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    group_id      UUID REFERENCES groups(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX brands_collective_unique ON brands(collective_id) WHERE group_id IS NULL;
CREATE UNIQUE INDEX brands_group_unique ON brands(group_id) WHERE group_id IS NOT NULL;

-- Tokens: editable data, never code (§8).
CREATE TABLE brand_tokens (
    id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    brand_id UUID NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    kind     TEXT NOT NULL CHECK (kind IN ('color', 'font', 'logo', 'grid', 'rule')),
    key      TEXT NOT NULL,     -- primary, secondary, accent, background, text, title…
    label    TEXT NOT NULL,
    value    JSONB NOT NULL,
    position INT NOT NULL DEFAULT 0,
    UNIQUE (brand_id, kind, key)
);

-- Editable catalogue: platforms change their formats without touching the code.
CREATE TABLE formats (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID REFERENCES collectives(id) ON DELETE CASCADE,  -- NULL = catalogue d'instance
    key           TEXT NOT NULL,
    platform      TEXT NOT NULL,
    label         TEXT NOT NULL,
    width         INT NOT NULL CHECK (width > 0),
    height        INT NOT NULL CHECK (height > 0),
    kind          TEXT NOT NULL DEFAULT 'image' CHECK (kind IN ('image', 'video', 'print')),
    position      INT NOT NULL DEFAULT 0,
    archived      BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE UNIQUE INDEX formats_instance_key ON formats(key) WHERE collective_id IS NULL;
CREATE UNIQUE INDEX formats_collective_key ON formats(collective_id, key) WHERE collective_id IS NOT NULL;

CREATE TABLE assets (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    group_id      UUID REFERENCES groups(id) ON DELETE SET NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('image', 'video', 'audio', 'font', 'pdf', 'render')),
    filename      TEXT NOT NULL,
    storage_key   TEXT NOT NULL UNIQUE,
    mime          TEXT NOT NULL,
    bytes         BIGINT NOT NULL DEFAULT 0,
    width         INT,
    height        INT,
    duration_ms   INT,
    tags          TEXT[] NOT NULL DEFAULT '{}',
    uploaded_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- renders are purged automatically at 6 months: they regenerate identically (§15).
    purge_after   TIMESTAMPTZ
);
CREATE INDEX assets_collective_idx ON assets(collective_id, created_at DESC);
CREATE INDEX assets_purge_idx ON assets(purge_after) WHERE purge_after IS NOT NULL;

-- A template is designed on a master format then derived (§9.3).
CREATE TABLE templates (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id    UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    group_id         UUID REFERENCES groups(id) ON DELETE CASCADE,
    name             TEXT NOT NULL,
    master_format_id UUID NOT NULL REFERENCES formats(id),
    version          INT NOT NULL DEFAULT 1,
    milestone_key    TEXT,       -- rattache le gabarit a un jalon de com
    archived         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX templates_collective_idx ON templates(collective_id);

-- One variant per format, ratio locked, with its own adjustments.
CREATE TABLE template_variants (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
    format_id   UUID NOT NULL REFERENCES formats(id),
    is_master   BOOLEAN NOT NULL DEFAULT FALSE,
    layout      JSONB NOT NULL,     -- description JSON : seule source de verite (§15)
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (template_id, format_id)
);

-- Editing a template breaks no visual already produced.
CREATE TABLE template_versions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
    version     INT NOT NULL,
    snapshot    JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (template_id, version)
);

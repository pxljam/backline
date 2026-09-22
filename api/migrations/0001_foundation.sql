-- Step 1 of §17: the foundation. Users, collectives, groups, sessions, job queue.

-- `gen_random_uuid()` has been native since PostgreSQL 13; the extension covers
-- older installations without changing a line of schema.
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name    TEXT NOT NULL,
    stage_name      TEXT,
    phone           TEXT,
    email           TEXT UNIQUE,
    password_hash   TEXT,
    telegram_id     BIGINT UNIQUE,
    telegram_username TEXT,
    is_instance_admin BOOLEAN NOT NULL DEFAULT FALSE,
    notif_quiet_from  SMALLINT,          -- quiet hours: start hour (0-23)
    notif_quiet_to    SMALLINT,
    notif_opt_out     TEXT[] NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Single-use invitation link: the only way to create an access.
CREATE TABLE invitations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code        TEXT NOT NULL UNIQUE,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at  TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_user_idx ON sessions(user_id);

CREATE TABLE collectives (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE memberships (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role          TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (collective_id, user_id)
);
CREATE INDEX memberships_user_idx ON memberships(user_id);

CREATE TABLE groups (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    slug          TEXT NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (collective_id, slug)
);

CREATE TABLE group_members (
    id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id  UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- free-form role: "MAO", "batterie", "live"… never a catalogue.
    role_label TEXT,
    is_admin  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (group_id, user_id)
);

-- Queue and scheduling. One single mechanism for reminders, follow-ups and
-- renders (§15). FOR UPDATE SKIP LOCKED.
CREATE TABLE jobs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind        TEXT NOT NULL,
    dedupe_key  TEXT UNIQUE,
    payload     JSONB NOT NULL DEFAULT '{}',
    run_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'running', 'done', 'failed')),
    attempts    INT NOT NULL DEFAULT 0,
    locked_at   TIMESTAMPTZ,
    last_error  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ
);
CREATE INDEX jobs_pending_idx ON jobs(status, run_at) WHERE status = 'pending';

CREATE TABLE notifications (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    collective_id UUID REFERENCES collectives(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,
    title         TEXT NOT NULL,
    body          TEXT NOT NULL DEFAULT '',
    payload       JSONB NOT NULL DEFAULT '{}',
    delivered_telegram BOOLEAN NOT NULL DEFAULT FALSE,
    read_at       TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX notifications_user_idx ON notifications(user_id, created_at DESC);

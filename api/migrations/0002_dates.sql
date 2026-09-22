-- Step 2 of §17: settling on a date. Venues, opportunities, poll, matrix.

CREATE TABLE venues (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    address       TEXT,
    city          TEXT,
    country       TEXT,
    capacity      INT,
    notes         TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX venues_collective_idx ON venues(collective_id);

CREATE TABLE venue_contacts (
    id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    venue_id UUID NOT NULL REFERENCES venues(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    role     TEXT,            -- programmateur, regie, production… texte libre
    phone    TEXT,
    email    TEXT
);

CREATE TABLE opportunities (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    venue_id      UUID REFERENCES venues(id) ON DELETE SET NULL,
    venue_contact_id UUID REFERENCES venue_contacts(id) ON DELETE SET NULL,
    title         TEXT NOT NULL,
    conditions    TEXT,
    status        TEXT NOT NULL DEFAULT 'discussing'
                  CHECK (status IN ('discussing', 'poll_open', 'date_chosen', 'confirmed', 'abandoned')),
    created_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX opportunities_collective_idx ON opportunities(collective_id);

-- Prospective hosts. No row = the collective hosts the opportunity.
CREATE TABLE opportunity_hosts (
    opportunity_id UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
    group_id       UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (opportunity_id, group_id)
);

CREATE TABLE candidate_dates (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    opportunity_id UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
    day            DATE NOT NULL,
    start_time     TIME,
    end_time       TIME,
    notes          TEXT,
    position       INT NOT NULL DEFAULT 0
);
CREATE INDEX candidate_dates_opportunity_idx ON candidate_dates(opportunity_id);

CREATE TABLE availability_polls (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    opportunity_id UUID NOT NULL UNIQUE REFERENCES opportunities(id) ON DELETE CASCADE,
    opened_by      UUID REFERENCES users(id) ON DELETE SET NULL,
    opened_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at      TIMESTAMPTZ
);

-- An availability is only written by its author (§5.2), admins included.
-- The rule lives in the access layer; this table only records the outcome.
CREATE TABLE availabilities (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    poll_id           UUID NOT NULL REFERENCES availability_polls(id) ON DELETE CASCADE,
    candidate_date_id UUID NOT NULL REFERENCES candidate_dates(id) ON DELETE CASCADE,
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status            TEXT NOT NULL CHECK (status IN ('yes', 'maybe', 'no')),
    wants_to_play     BOOLEAN NOT NULL DEFAULT FALSE,
    source            TEXT NOT NULL DEFAULT 'web' CHECK (source IN ('web', 'telegram')),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (candidate_date_id, user_id)
);
CREATE INDEX availabilities_poll_idx ON availabilities(poll_id);

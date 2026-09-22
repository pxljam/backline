-- Step 8 of §17: declarative video, render queue, members' machines (§10).

-- "The recipe, not the dish": a declarative, versioned description.
CREATE TABLE video_compositions (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    group_id      UUID REFERENCES groups(id) ON DELETE SET NULL,
    event_id      UUID REFERENCES events(id) ON DELETE SET NULL,
    name          TEXT NOT NULL,
    format_id     UUID NOT NULL REFERENCES formats(id),
    fps           INT NOT NULL DEFAULT 30 CHECK (fps > 0),
    spec          JSONB NOT NULL,     -- plans, calques, animations, audio
    version       INT NOT NULL DEFAULT 1,
    created_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX video_compositions_collective_idx ON video_compositions(collective_id);

-- A member's machine, revocable token, optional GPU.
CREATE TABLE render_machines (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,
    capabilities JSONB NOT NULL DEFAULT '{}',  -- { gpu: bool, concurrency: n, platform }
    last_seen_at TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The same queue as everything else: claimed with FOR UPDATE SKIP LOCKED.
CREATE TABLE render_jobs (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    collective_id  UUID NOT NULL REFERENCES collectives(id) ON DELETE CASCADE,
    composition_id UUID REFERENCES video_compositions(id) ON DELETE CASCADE,
    event_id       UUID REFERENCES events(id) ON DELETE SET NULL,
    publication_task_id UUID REFERENCES publication_tasks(id) ON DELETE SET NULL,
    kind           TEXT NOT NULL DEFAULT 'video' CHECK (kind IN ('video', 'preview')),
    status         TEXT NOT NULL DEFAULT 'queued'
                   CHECK (status IN ('queued', 'claimed', 'running', 'done', 'failed', 'cancelled')),
    claimed_by     UUID REFERENCES render_machines(id) ON DELETE SET NULL,
    progress       REAL NOT NULL DEFAULT 0,
    output_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL,
    error          TEXT,
    attempts       INT NOT NULL DEFAULT 0,
    -- a job left unclaimed past a delay alerts the admin, it does not sleep (§10.3)
    unclaimed_alert_sent_at TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at     TIMESTAMPTZ,
    finished_at    TIMESTAMPTZ
);
CREATE INDEX render_jobs_queue_idx ON render_jobs(status, created_at) WHERE status = 'queued';

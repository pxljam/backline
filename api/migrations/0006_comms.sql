-- Step 7 of §17: comms plan, publication tasks, assisted mode (§11.2).

CREATE TABLE comms_plans (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id   UUID NOT NULL UNIQUE REFERENCES events(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE publication_tasks (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    comms_plan_id UUID NOT NULL REFERENCES comms_plans(id) ON DELETE CASCADE,
    event_id      UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    milestone_key TEXT NOT NULL,        -- j-30, j-7, j0-15m, j+1…
    label         TEXT NOT NULL,
    -- every task is born in the draft status: a human always approves (§11.1)
    status        TEXT NOT NULL DEFAULT 'draft'
                  CHECK (status IN ('draft', 'ready', 'assigned', 'published', 'missed')),
    scheduled_at  TIMESTAMPTZ NOT NULL,
    social_account_id UUID REFERENCES social_accounts(id) ON DELETE SET NULL,
    -- constrained assignment: only someone with access to the account in question
    assignee_id        UUID REFERENCES users(id) ON DELETE SET NULL,
    backup_assignee_id UUID REFERENCES users(id) ON DELETE SET NULL,
    caption       TEXT NOT NULL DEFAULT '',
    hashtags      TEXT NOT NULL DEFAULT '',
    formats       JSONB NOT NULL DEFAULT '[]',  -- cles de format attendues
    published_at  TIMESTAMPTZ,
    published_url TEXT,
    reminder_count INT NOT NULL DEFAULT 0,
    admin_alerted_at TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX publication_tasks_event_idx ON publication_tasks(event_id, scheduled_at);
CREATE INDEX publication_tasks_due_idx ON publication_tasks(scheduled_at) WHERE status <> 'published';

-- Visuals attached to a task: produced in one pass from the templates (§9.4).
CREATE TABLE publication_assets (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    publication_task_id UUID NOT NULL REFERENCES publication_tasks(id) ON DELETE CASCADE,
    format_id           UUID NOT NULL REFERENCES formats(id),
    template_id         UUID REFERENCES templates(id) ON DELETE SET NULL,
    template_version    INT,
    asset_id            UUID REFERENCES assets(id) ON DELETE SET NULL,
    status              TEXT NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'rendering', 'ready', 'failed')),
    error               TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (publication_task_id, format_id)
);

import type { Layout, VideoSpec } from "@backline/layout";

export interface Me {
  id: string;
  display_name: string;
  stage_name: string | null;
  phone: string | null;
  email: string | null;
  telegram_linked: boolean;
  is_instance_admin: boolean;
  collectives: { id: string; slug: string; name: string; role: "admin" | "member" }[];
  groups: { id: string; collective_id: string; name: string; is_admin: boolean }[];
}

/** Jalon de com : une ligne de la timeline d'un type d'evenement (§11.1). */
export interface Milestone {
  key: string;
  label: string;
  /** Decalage en jours par rapport a l'ancre. Negatif = avant. */
  offset_days: number;
  offset_minutes: number;
  /** Heure locale imposee, « 18:00 ». */
  at: string | null;
  formats: string[];
  caption: string;
  anchor: "start" | "end";
}

export interface LogisticsSlot {
  label: string;
  quantity: number;
}

export interface EventType {
  id: string;
  key: string;
  label: string;
  requires_venue: boolean;
  is_range: boolean;
  comms_milestones: Milestone[];
  default_logistics_slots: LogisticsSlot[];
}

export interface CollectiveDetail {
  id: string;
  slug: string;
  name: string;
  role: "admin" | "member";
  event_types: EventType[];
}

export interface Member {
  user_id: string;
  display_name: string;
  stage_name: string | null;
  phone: string | null;
  email: string | null;
  role: "admin" | "member";
  telegram_linked: boolean;
  groups: string[];
  pending_invitation: string | null;
}

export interface Group {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  members: {
    user_id: string;
    display_name: string;
    stage_name: string | null;
    role_label: string | null;
    is_admin: boolean;
  }[];
  has_tech_rider: boolean;
}

export interface Venue {
  id: string;
  name: string;
  address: string | null;
  city: string | null;
  capacity: number | null;
  notes: string | null;
  contacts: {
    id: string;
    name: string;
    role: string | null;
    phone: string | null;
    email: string | null;
  }[];
  past_events: number;
}

export interface CandidateDate {
  id: string;
  day: string;
  start_time: string | null;
  end_time: string | null;
  notes: string | null;
}

export interface Opportunity {
  id: string;
  title: string;
  status: "discussing" | "poll_open" | "date_chosen" | "confirmed" | "abandoned";
  conditions: string | null;
  venue: { id: string; name: string; city: string | null } | null;
  host_group_ids: string[];
  candidate_dates: CandidateDate[];
  poll_open: boolean;
  event_id: string | null;
}

export interface Matrix {
  opportunity_id: string;
  poll_open: boolean;
  dates: {
    id: string;
    day: string;
    start_time: string | null;
    end_time: string | null;
    notes: string | null;
    yes: number;
    maybe: number;
    no: number;
    no_answer: number;
    volunteers: string[];
    complete_groups: string[];
    partial_groups: { group_id: string; missing: string[] }[];
  }[];
  members: {
    user_id: string;
    display_name: string;
    stage_name: string | null;
    group_ids: string[];
    answers: Record<string, { status: "yes" | "maybe" | "no"; wants_to_play: boolean }>;
  }[];
  groups: { id: string; name: string; member_ids: string[] }[];
}

export interface BacklineEvent {
  id: string;
  title: string;
  status: "draft" | "confirmed" | "past" | "cancelled";
  type_key: string;
  type_label: string;
  starts_at: string;
  ends_at: string | null;
  doors_at: string | null;
  soundcheck_at: string | null;
  set_times_state: "undefined" | "to_confirm" | "defined";
  set_times_public: boolean;
  venue: { id: string; name: string; city: string | null; address: string | null } | null;
  host_group_id: string | null;
  participations: {
    id: string;
    group_id: string | null;
    user_id: string | null;
    label: string;
    status: string;
    stage_role: string | null;
    slot_start: string | null;
    slot_end: string | null;
    acknowledged: boolean;
  }[];
  logistics: {
    id: string;
    label: string;
    quantity: number;
    notes: string | null;
    assignees: { user_id: string; name: string }[];
    vacant: number;
  }[];
  stream: {
    capture_location: string | null;
    planned_duration_min: number | null;
    replay_url: string | null;
    live_alert_sent_at: string | null;
    platforms: { platform: string; url: string | null }[];
  } | null;
  residency: {
    presences: { user_id: string; name: string; answer: string; days: string[] }[];
  } | null;
  tech_riders: {
    group_id: string;
    group_name: string;
    tech_rider_id: string | null;
    version: number | null;
    sent_at: string | null;
  }[];
  comms_summary: { total: number; published: number; late: number; unassigned: number };
  notes: string | null;
}

export interface RunSheet {
  event_id: string;
  title: string;
  starts_at: string;
  doors_at: string | null;
  soundcheck_at: string | null;
  venue: {
    name: string;
    address: string | null;
    city: string | null;
    contacts: { name: string; role: string | null; phone: string | null }[];
  } | null;
  line_up: {
    label: string;
    stage_role: string | null;
    slot: string | null;
    members: { user_id: string; name: string; phone: string | null }[];
  }[];
  logistics: {
    label: string;
    quantity: number;
    assignees: { user_id: string; name: string; phone: string | null }[];
    vacant: number;
  }[];
  tech_riders: {
    group_id: string;
    group_name: string;
    tech_rider_id: string | null;
    version: number | null;
  }[];
}

export interface PublicationTask {
  id: string;
  event_id: string;
  event_title: string;
  milestone_key: string;
  label: string;
  status: "draft" | "ready" | "assigned" | "published" | "missed";
  scheduled_at: string;
  social_account: { id: string; platform: string; handle: string } | null;
  assignee_id: string | null;
  backup_assignee_id: string | null;
  caption: string;
  hashtags: string;
  formats: string[];
  published_at: string | null;
  published_url: string | null;
  reminder_count: number;
  visuals: {
    format_id: string;
    format_key: string;
    status: string;
    asset_id: string | null;
    error: string | null;
  }[];
}

export interface SocialAccount {
  id: string;
  group_id: string | null;
  platform: string;
  handle: string;
  url: string | null;
  mode: "shared" | "personal";
  vault_url: string | null;
  notes: string | null;
  access: string[];
}

export interface Format {
  id: string;
  key: string;
  platform: string;
  label: string;
  width: number;
  height: number;
  kind: "image" | "video" | "print";
  ratio: string;
  custom: boolean;
}

export interface BrandToken {
  kind: "color" | "font" | "logo" | "grid" | "rule";
  key: string;
  label: string;
  value: Record<string, unknown>;
  position: number;
}

export interface BrandView {
  id: string;
  name: string;
  group_id: string | null;
  tokens: BrandToken[];
  inherited: BrandToken[] | null;
}

export interface Template {
  id: string;
  name: string;
  group_id: string | null;
  master_format_id: string;
  milestone_key: string | null;
  version: number;
  variants: {
    format_id: string;
    format_key: string;
    width: number;
    height: number;
    ratio: string;
    is_master: boolean;
    layout: Layout;
  }[];
}

export interface Asset {
  id: string;
  kind: string;
  filename: string;
  mime: string;
  bytes: number;
  width: number | null;
  height: number | null;
  duration_ms: number | null;
  tags: string[];
  group_id: string | null;
  created_at: string;
}

export interface VideoCompositionRow {
  id: string;
  name: string;
  group_id: string | null;
  event_id: string | null;
  format_id: string;
  fps: number;
  version: number;
  spec: VideoSpec;
  updated_at: string;
}

export interface RenderJob {
  id: string;
  composition_id: string | null;
  composition_name: string | null;
  kind: string;
  status: string;
  progress: number;
  error: string | null;
  output_asset_id: string | null;
  claimed_by: string | null;
  created_at: string;
  finished_at: string | null;
}

export interface RenderMachine {
  id: string;
  name: string;
  capabilities: { gpu?: boolean; gpuKind?: string | null; concurrency?: number };
  last_seen_at: string | null;
  online: boolean;
}

export interface Dashboard {
  pending_polls: { opportunity_id: string; title: string; dates: number; mine_missing: boolean }[];
  vacant_slots: {
    event_id: string;
    event_title: string;
    starts_at: string;
    label: string;
    vacant: number;
  }[];
  late_tasks: {
    task_id: string;
    event_title: string;
    label: string;
    scheduled_at: string;
    status: string;
    assignee_id: string | null;
  }[];
  upcoming: {
    id: string;
    title: string;
    starts_at: string;
    type_key: string;
    venue: string | null;
  }[];
  missing_tech_riders: { event_id: string; event_title: string; group_name: string }[];
  render_machines_online: number;
  unread_notifications: number;
  is_admin: boolean;
}

export interface Notification {
  id: string;
  collective_id: string | null;
  kind: string;
  title: string;
  body: string;
  payload: Record<string, unknown>;
  read_at: string | null;
  created_at: string;
}

export interface CalendarEntry {
  id: string;
  kind: "event" | "candidate_date";
  title: string;
  status: string;
  starts_at: string;
  ends_at: string | null;
  venue: string | null;
  city: string | null;
  type_key: string | null;
  opportunity_id: string | null;
}

export interface TechRider {
  id: string;
  version: number;
  status: "draft" | "published";
  data: Record<string, unknown>;
  created_at: string;
}

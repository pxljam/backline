//! Backline integration tests, against real PostgreSQL and MinIO.
//!
//! Each module covers one §18 invariant (acceptance criteria).

#[path = "it/harness.rs"]
mod harness;

#[path = "it/smoke.rs"]
mod smoke;

#[path = "it/isolation.rs"]
mod isolation;

#[path = "it/dates.rs"]
mod dates;

#[path = "it/logistics.rs"]
mod logistics;

#[path = "it/streams.rs"]
mod streams;

#[path = "it/riders.rs"]
mod riders;

#[path = "it/comms.rs"]
mod comms;

#[path = "it/studio.rs"]
mod studio;

#[path = "it/video.rs"]
mod video;

#[path = "it/calendar.rs"]
mod calendar;

#[path = "it/auth.rs"]
mod auth;

#[path = "it/bot.rs"]
mod bot;

#[path = "it/visuals.rs"]
mod visuals;

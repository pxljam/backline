use anyhow::{Context, Result};

/// All configuration comes from environment variables (§15); `.env.example` is
/// the reference.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub session_secret: String,
    pub public_base_url: String,
    pub s3: S3Config,
    pub telegram_bot_token: Option<String>,
    pub telegram_bot_username: Option<String>,
    pub stills_url: Option<String>,
    pub run_migrations: bool,
    pub run_seed: bool,
}

#[derive(Clone, Debug)]
pub struct S3Config {
    pub endpoint: String,
    /// Endpoint reachable from the browser and the render CLI — it differs from
    /// `endpoint` locally, where the API talks to MinIO over the Docker network.
    pub public_endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

fn var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn flag(key: &str, default: bool) -> bool {
    match var(key) {
        Some(v) => matches!(v.as_str(), "1" | "true" | "yes" | "on"),
        None => default,
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = var("DATABASE_URL").context("DATABASE_URL est obligatoire")?;
        let endpoint = var("S3_ENDPOINT").unwrap_or_else(|| "http://localhost:9100".into());
        Ok(Self {
            database_url,
            bind_addr: var("BIND_ADDR").unwrap_or_else(|| "0.0.0.0:8080".into()),
            session_secret: var("SESSION_SECRET")
                .unwrap_or_else(|| "dev-session-secret-change-me-in-production-please".into()),
            public_base_url: var("PUBLIC_BASE_URL")
                .unwrap_or_else(|| "http://localhost:8088".into()),
            s3: S3Config {
                public_endpoint: var("S3_PUBLIC_ENDPOINT").unwrap_or_else(|| endpoint.clone()),
                endpoint,
                bucket: var("S3_BUCKET").unwrap_or_else(|| "backline".into()),
                region: var("S3_REGION").unwrap_or_else(|| "us-east-1".into()),
                access_key: var("S3_ACCESS_KEY").unwrap_or_else(|| "backline".into()),
                secret_key: var("S3_SECRET_KEY").unwrap_or_else(|| "backline-dev-secret".into()),
            },
            telegram_bot_token: var("TELEGRAM_BOT_TOKEN"),
            telegram_bot_username: var("TELEGRAM_BOT_USERNAME"),
            stills_url: var("STILLS_URL"),
            run_migrations: flag("RUN_MIGRATIONS", true),
            run_seed: flag("RUN_SEED", false),
        })
    }

    /// Minimal configuration used by the integration tests.
    pub fn for_test(database_url: String, s3: S3Config) -> Self {
        Self {
            database_url,
            bind_addr: "127.0.0.1:0".into(),
            session_secret: "test-secret-test-secret-test-secret".into(),
            public_base_url: "http://localhost:8088".into(),
            s3,
            telegram_bot_token: None,
            telegram_bot_username: None,
            stills_url: None,
            run_migrations: true,
            run_seed: false,
        }
    }
}

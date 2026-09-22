//! Integration test harness.
//!
//! **Real PostgreSQL and MinIO, never a fake** (§15). The containers start once
//! for the whole suite; each test gets its own database and its own bucket,
//! which buys isolation without paying for a container start per test.

#![allow(dead_code)]

use backline::config::S3Config;
use backline::scope::Actor;
use backline::{AppState, Config};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;
use uuid::Uuid;

const MINIO_IMAGE: &str = "quay.io/minio/minio";
const MINIO_TAG: &str = "RELEASE.2025-09-07T16-13-09Z";
const MINIO_USER: &str = "backline";
const MINIO_PASSWORD: &str = "backline-test-secret";

struct Shared {
    _pg: ContainerAsync<Postgres>,
    _minio: ContainerAsync<GenericImage>,
    pg_url_root: String,
    s3_endpoint: String,
}

static SHARED: OnceCell<Shared> = OnceCell::const_new();

async fn shared() -> &'static Shared {
    SHARED
        .get_or_init(|| async {
            // The same version as production: local/production parity is a
            // stated constraint (§15), and it holds for the tests too.
            let pg = Postgres::default()
                .with_user("backline")
                .with_password("backline")
                .with_db_name("postgres")
                .with_tag("16-alpine")
                .start()
                .await
                .expect("test postgres");
            let pg_port = pg.get_host_port_ipv4(5432).await.expect("postgres port");

            let minio = GenericImage::new(MINIO_IMAGE, MINIO_TAG)
                .with_exposed_port(9000.tcp())
                .with_wait_for(WaitFor::message_on_stderr("Docs:"))
                .with_env_var("MINIO_ROOT_USER", MINIO_USER)
                .with_env_var("MINIO_ROOT_PASSWORD", MINIO_PASSWORD)
                .with_cmd(vec!["server", "/data"])
                .start()
                .await
                .expect("test minio");
            let minio_port = minio.get_host_port_ipv4(9000).await.expect("minio port");

            Shared {
                _pg: pg,
                _minio: minio,
                pg_url_root: format!("postgres://backline:backline@127.0.0.1:{pg_port}"),
                s3_endpoint: format!("http://127.0.0.1:{minio_port}"),
            }
        })
        .await
}

/// A complete application, on its own database and its own bucket.
pub struct TestApp {
    pub base: String,
    pub db: PgPool,
    pub state: AppState,
    pub http: reqwest::Client,
}

impl TestApp {
    pub async fn new() -> Self {
        // The Typst templates live in `pdf/` at the repository root; in a
        // container they are copied to /app/pdf.
        let templates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("pdf");
        std::env::set_var("TYPST_TEMPLATE_DIR", templates);

        let shared = shared().await;

        // A dedicated database: two tests never see each other.
        let name = format!("bl_{}", Uuid::new_v4().simple());
        let root = sqlx::PgPool::connect(&format!("{}/postgres", shared.pg_url_root))
            .await
            .expect("postgres connection");
        sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
            .execute(&root)
            .await
            .expect("creating the test database");
        root.close().await;

        let database_url = format!("{}/{name}", shared.pg_url_root);
        let s3 = S3Config {
            endpoint: shared.s3_endpoint.clone(),
            public_endpoint: shared.s3_endpoint.clone(),
            bucket: format!("bl-{}", Uuid::new_v4().simple()),
            region: "us-east-1".into(),
            access_key: MINIO_USER.into(),
            secret_key: MINIO_PASSWORD.into(),
        };

        let mut config = Config::for_test(database_url, s3);
        config.telegram_bot_token = Some("123456:TEST-TOKEN".into());

        let state = backline::build_state(config)
            .await
            .expect("application state");
        let db = state.db.clone();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("free port");
        let addr = listener.local_addr().unwrap();
        let app = backline::routes::router(state.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Self {
            base: format!("http://{addr}"),
            db,
            state,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        }
    }

    /// Plugs in (or unplugs) the still visuals render service.
    pub fn set_stills_url(&mut self, url: Option<String>) {
        let mut config = (*self.state.config).clone();
        config.stills_url = url;
        self.state.config = std::sync::Arc::new(config);
    }

    /// Application seeded with the §16 data.
    pub async fn seeded() -> Self {
        let app = Self::new().await;
        backline::seed::seed(&app.db).await.expect("seed data");
        app
    }

    /// Triggers one pass of the job loop rather than waiting on a clock: the
    /// tests stay deterministic.
    pub async fn tick(&self) -> usize {
        backline::services::scheduler::tick(&self.state)
            .await
            .expect("job loop")
    }

    /// Makes every pending job run as if it were due.
    pub async fn run_due_jobs(&self) {
        sqlx::query("UPDATE jobs SET run_at = now() WHERE status = 'pending'")
            .execute(&self.db)
            .await
            .unwrap();
        // Several passes: one job can schedule another.
        for _ in 0..3 {
            self.tick().await;
        }
    }

    pub async fn collective_id(&self, slug: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM collectives WHERE slug = $1")
            .bind(slug)
            .fetch_one(&self.db)
            .await
            .expect("collective")
            .0
    }

    pub async fn user_id(&self, display_name: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE display_name = $1")
            .bind(display_name)
            .fetch_one(&self.db)
            .await
            .expect("user")
            .0
    }

    pub async fn group_id(&self, slug: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM groups WHERE slug = $1")
            .bind(slug)
            .fetch_one(&self.db)
            .await
            .expect("group")
            .0
    }

    /// Creates a user, adds them to the collective and opens a session.
    pub async fn login_as(&self, user_id: Uuid) -> Client {
        let token = backline::auth::create_session(&self.db, user_id)
            .await
            .expect("session");
        Client {
            base: self.base.clone(),
            token,
            http: self.http.clone(),
        }
    }

    pub async fn login_named(&self, display_name: &str) -> Client {
        let id = self.user_id(display_name).await;
        self.login_as(id).await
    }

    /// Creates a complete collective outside the seed data: useful for testing
    /// isolation between two collectives.
    pub async fn make_collective(&self, slug: &str, name: &str) -> Uuid {
        backline::services::provisioning::create_collective(&self.db, slug, name)
            .await
            .expect("collective")
    }

    pub async fn make_user(&self, display_name: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>(
            "INSERT INTO users (display_name, phone) VALUES ($1, '+33600000000') RETURNING id",
        )
        .bind(display_name)
        .fetch_one(&self.db)
        .await
        .expect("user")
        .0
    }

    pub async fn join(&self, collective_id: Uuid, user_id: Uuid, role: &str) {
        sqlx::query(
            "INSERT INTO memberships (collective_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (collective_id, user_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(collective_id)
        .bind(user_id)
        .bind(role)
        .execute(&self.db)
        .await
        .expect("membership");
    }
}

/// An authenticated HTTP client. The tests talk to the API over the network,
/// just as the interface will: nothing is short-circuited.
pub struct Client {
    pub base: String,
    pub token: String,
    http: reqwest::Client,
}

pub struct Res {
    pub status: u16,
    pub body: Value,
    pub text: String,
}

impl Res {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Fails with the response body: a red test must say why.
    pub fn expect_ok(&self) -> &Value {
        assert!(
            self.ok(),
            "expected 2xx, got {}: {}",
            self.status,
            self.text
        );
        &self.body
    }

    pub fn expect_status(&self, status: u16) {
        assert_eq!(self.status, status, "corps : {}", self.text);
    }
}

impl Client {
    async fn send(&self, req: reqwest::RequestBuilder) -> Res {
        let resp = req
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .expect("request");
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        let body = serde_json::from_str(&text).unwrap_or(Value::Null);
        Res { status, body, text }
    }

    pub async fn get(&self, path: &str) -> Res {
        self.send(self.http.get(format!("{}{path}", self.base)))
            .await
    }

    pub async fn post(&self, path: &str, body: Value) -> Res {
        self.send(self.http.post(format!("{}{path}", self.base)).json(&body))
            .await
    }

    pub async fn put(&self, path: &str, body: Value) -> Res {
        self.send(self.http.put(format!("{}{path}", self.base)).json(&body))
            .await
    }

    pub async fn patch(&self, path: &str, body: Value) -> Res {
        self.send(self.http.patch(format!("{}{path}", self.base)).json(&body))
            .await
    }

    pub async fn delete(&self, path: &str) -> Res {
        self.send(self.http.delete(format!("{}{path}", self.base)))
            .await
    }
}

/// Anonymous client, for the public routes (iCal feeds, invitations).
pub async fn anonymous_get(base: &str, path: &str) -> (u16, String) {
    let resp = reqwest::get(format!("{base}{path}"))
        .await
        .expect("request");
    (
        resp.status().as_u16(),
        resp.text().await.unwrap_or_default(),
    )
}

pub fn actor(user_id: Uuid) -> Actor {
    Actor {
        user_id,
        is_instance_admin: false,
    }
}

pub fn _unused(_: Arc<()>) {}

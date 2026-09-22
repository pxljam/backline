//! Socle des tests d'integration.
//!
//! **PostgreSQL et MinIO reels, jamais de simulacre** (§15). Les conteneurs
//! sont demarres une fois pour toute la suite ; chaque test obtient sa propre
//! base et son propre bucket, ce qui lui garantit l'isolation sans payer un
//! demarrage de conteneur par test.

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
            // Meme version qu'en production : la parite locale/production est
            // une contrainte posee (§15), elle vaut aussi pour les tests.
            let pg = Postgres::default()
                .with_user("backline")
                .with_password("backline")
                .with_db_name("postgres")
                .with_tag("16-alpine")
                .start()
                .await
                .expect("postgres de test");
            let pg_port = pg.get_host_port_ipv4(5432).await.expect("port postgres");

            let minio = GenericImage::new(MINIO_IMAGE, MINIO_TAG)
                .with_exposed_port(9000.tcp())
                .with_wait_for(WaitFor::message_on_stderr("Docs:"))
                .with_env_var("MINIO_ROOT_USER", MINIO_USER)
                .with_env_var("MINIO_ROOT_PASSWORD", MINIO_PASSWORD)
                .with_cmd(vec!["server", "/data"])
                .start()
                .await
                .expect("minio de test");
            let minio_port = minio.get_host_port_ipv4(9000).await.expect("port minio");

            Shared {
                _pg: pg,
                _minio: minio,
                pg_url_root: format!("postgres://backline:backline@127.0.0.1:{pg_port}"),
                s3_endpoint: format!("http://127.0.0.1:{minio_port}"),
            }
        })
        .await
}

/// Une application complete, sur sa propre base et son propre bucket.
pub struct TestApp {
    pub base: String,
    pub db: PgPool,
    pub state: AppState,
    pub http: reqwest::Client,
}

impl TestApp {
    pub async fn new() -> Self {
        // Les modeles Typst vivent dans `pdf/` a la racine du depot ; en
        // conteneur ils sont copies dans /app/pdf.
        let templates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("pdf");
        std::env::set_var("TYPST_TEMPLATE_DIR", templates);

        let shared = shared().await;

        // Base dediee : deux tests ne se voient jamais.
        let name = format!("bl_{}", Uuid::new_v4().simple());
        let root = sqlx::PgPool::connect(&format!("{}/postgres", shared.pg_url_root))
            .await
            .expect("connexion postgres");
        sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
            .execute(&root)
            .await
            .expect("creation de la base de test");
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
            .expect("etat applicatif");
        let db = state.db.clone();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("port libre");
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

    /// Branche (ou debranche) le service de rendu des visuels fixes.
    pub fn set_stills_url(&mut self, url: Option<String>) {
        let mut config = (*self.state.config).clone();
        config.stills_url = url;
        self.state.config = std::sync::Arc::new(config);
    }

    /// Application amorcee avec les donnees du §16.
    pub async fn seeded() -> Self {
        let app = Self::new().await;
        backline::seed::seed(&app.db)
            .await
            .expect("donnees d'amorcage");
        app
    }

    /// Declenche un passage de la boucle de jobs, plutot que d'attendre une
    /// horloge : les tests restent deterministes.
    pub async fn tick(&self) -> usize {
        backline::services::scheduler::tick(&self.state)
            .await
            .expect("boucle de jobs")
    }

    /// Fait passer tous les jobs en attente comme s'ils etaient echus.
    pub async fn run_due_jobs(&self) {
        sqlx::query("UPDATE jobs SET run_at = now() WHERE status = 'pending'")
            .execute(&self.db)
            .await
            .unwrap();
        // Plusieurs passages : un job peut en programmer un autre.
        for _ in 0..3 {
            self.tick().await;
        }
    }

    pub async fn collective_id(&self, slug: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM collectives WHERE slug = $1")
            .bind(slug)
            .fetch_one(&self.db)
            .await
            .expect("collectif")
            .0
    }

    pub async fn user_id(&self, display_name: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE display_name = $1")
            .bind(display_name)
            .fetch_one(&self.db)
            .await
            .expect("utilisateur")
            .0
    }

    pub async fn group_id(&self, slug: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM groups WHERE slug = $1")
            .bind(slug)
            .fetch_one(&self.db)
            .await
            .expect("groupe")
            .0
    }

    /// Cree un utilisateur, l'ajoute au collectif et ouvre une session.
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

    /// Cree un collectif complet hors donnees d'amorcage : utile pour tester
    /// le cloisonnement entre deux collectifs.
    pub async fn make_collective(&self, slug: &str, name: &str) -> Uuid {
        backline::services::provisioning::create_collective(&self.db, slug, name)
            .await
            .expect("collectif")
    }

    pub async fn make_user(&self, display_name: &str) -> Uuid {
        sqlx::query_as::<_, (Uuid,)>(
            "INSERT INTO users (display_name, phone) VALUES ($1, '+33600000000') RETURNING id",
        )
        .bind(display_name)
        .fetch_one(&self.db)
        .await
        .expect("utilisateur")
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
        .expect("appartenance");
    }
}

/// Un client HTTP authentifie. Les tests parlent a l'API par le reseau, comme
/// le fera l'interface : rien n'est court-circuite.
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

    /// Echoue avec le corps de la reponse : un test rouge doit dire pourquoi.
    pub fn expect_ok(&self) -> &Value {
        assert!(
            self.ok(),
            "attendu 2xx, recu {} : {}",
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
            .expect("requete");
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

/// Client anonyme, pour les routes publiques (flux iCal, invitations).
pub async fn anonymous_get(base: &str, path: &str) -> (u16, String) {
    let resp = reqwest::get(format!("{base}{path}"))
        .await
        .expect("requete");
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

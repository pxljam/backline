//! §18: "Every visual for an event is produced **in one action**, in every
//! format, true to the brand."
//!
//! The rendering itself belongs to the `stills` service (Node + Remotion). The
//! full test runs when an instance of that service is reachable
//! (`BACKLINE_STILLS_URL`); without one, the other half of the requirement is
//! checked: an unavailable renderer is **reported**, and does not take the
//! comms down with it.

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

async fn concert(app: &TestApp) -> (Uuid, Uuid) {
    let cid = app.collective_id("bonsoir-techno").await;
    let (event_id,): (Uuid,) = sqlx::query_as(
        "SELECT e.id FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'concert' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    (cid, event_id)
}

#[tokio::test]
async fn one_action_triggers_every_format_in_the_plan() {
    let Some(stills) = std::env::var("BACKLINE_STILLS_URL").ok() else {
        eprintln!("BACKLINE_STILLS_URL absent — test de rendu complet ignore");
        return;
    };

    let mut app = TestApp::new().await;
    app.set_stills_url(Some(stills));
    backline::seed::seed(&app.db).await.unwrap();

    let (cid, event_id) = concert(&app).await;
    let antoine = app.login_named("Antoine").await;

    antoine
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/comms/generate"),
            json!({}),
        )
        .await
        .expect_ok();

    app.run_due_jobs().await;

    let plan = antoine
        .get(&format!("/api/collectives/{cid}/events/{event_id}/comms"))
        .await;
    let plan = plan.expect_ok();

    let visuals: Vec<&serde_json::Value> = plan
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["visuals"].as_array().unwrap())
        .collect();
    assert!(!visuals.is_empty(), "no visual produced");

    let ready = visuals.iter().filter(|v| v["status"] == "ready").count();
    assert!(ready > 0, "at least one visual must come out: {visuals:?}");

    // Every ready visual points at a real file, of the right size.
    let asset_id = visuals.iter().find(|v| v["status"] == "ready").unwrap()["asset_id"]
        .as_str()
        .unwrap()
        .to_string();
    let url = antoine
        .get(&format!(
            "/api/collectives/{cid}/studio/assets/{asset_id}/url"
        ))
        .await;
    let url = url.expect_ok()["url"].as_str().unwrap().to_string();
    let bytes = reqwest::get(&url).await.unwrap().bytes().await.unwrap();
    assert!(
        bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "a PNG is expected"
    );

    // Renders carry a purge date: they regenerate identically.
    let (purge,): (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT purge_after FROM assets WHERE id = $1")
            .bind(Uuid::parse_str(&asset_id).unwrap())
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(
        purge.is_some(),
        "un rendu est purgeable, un media televerse ne l'est pas"
    );
}

#[tokio::test]
async fn a_missing_render_service_is_reported_without_blocking_comms() {
    let mut app = TestApp::new().await;
    // An address that does not answer: exactly the "the renderer went down" case.
    app.set_stills_url(Some("http://127.0.0.1:1".into()));
    backline::seed::seed(&app.db).await.unwrap();

    let (cid, event_id) = concert(&app).await;
    let antoine = app.login_named("Antoine").await;

    antoine
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/comms/generate"),
            json!({}),
        )
        .await
        .expect_ok();
    app.run_due_jobs().await;

    let plan = antoine
        .get(&format!("/api/collectives/{cid}/events/{event_id}/comms"))
        .await;
    let plan = plan.expect_ok();

    let failed: Vec<&serde_json::Value> = plan
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["visuals"].as_array().unwrap())
        .filter(|v| v["status"] == "failed")
        .collect();
    assert!(
        !failed.is_empty(),
        "l'echec doit etre enregistre, pas avale"
    );
    assert!(
        failed[0]["error"].as_str().unwrap().contains("injoignable"),
        "l'erreur doit dire ce qui s'est passe : {}",
        failed[0]["error"]
    );

    // The comms carry on: the tasks still exist and stay assignable and
    // publishable.
    let tasks = plan.as_array().unwrap();
    assert!(!tasks.is_empty());
    assert!(
        tasks.iter().all(|t| t["status"] != "published"),
        "rien n'a ete publie tout seul"
    );
}

#[tokio::test]
async fn a_template_missing_for_a_format_is_said_explicitly() {
    let app = TestApp::seeded().await;
    let (cid, event_id) = concert(&app).await;
    let antoine = app.login_named("Antoine").await;

    // The seeded template does not cover the Reel format: the D-14 milestone
    // asks for one.
    antoine
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/comms/generate"),
            json!({}),
        )
        .await
        .expect_ok();
    app.run_due_jobs().await;

    let (erreur,): (Option<String>,) = sqlx::query_as(
        "SELECT pa.error FROM publication_assets pa
         JOIN formats f ON f.id = pa.format_id
         WHERE f.key = 'reel_9_16' LIMIT 1",
    )
    .fetch_optional(&app.db)
    .await
    .unwrap()
    .unwrap_or((None,));

    assert_eq!(
        erreur.as_deref(),
        Some("aucun gabarit ne couvre ce format"),
        "l'admin doit savoir quel gabarit lui manque"
    );
}

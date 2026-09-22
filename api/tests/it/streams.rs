//! §18 : « Un stream declenche l'alerte "on est en ligne" a tous les membres
//! 15 minutes avant, sans intervention. »

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn l_alerte_de_mise_en_ligne_part_quinze_minutes_avant() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let (event_id, starts_at): (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT e.id, e.starts_at FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'stream' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    // Le job est programme a l'heure exacte, pas « quelque part avant ».
    let (run_at,): (chrono::DateTime<chrono::Utc>,) = sqlx::query_as(
        "SELECT run_at FROM jobs WHERE kind = 'stream_live_alert' AND payload->>'event_id' = $1",
    )
    .bind(event_id.to_string())
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(run_at, starts_at - chrono::Duration::minutes(15));

    app.run_due_jobs().await;

    // Tous les membres, sans exception.
    let (destinataires,): (i64,) = sqlx::query_as(
        "SELECT count(DISTINCT user_id) FROM notifications WHERE kind = 'stream_live'",
    )
    .fetch_one(&app.db)
    .await
    .unwrap();
    let (membres,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM memberships WHERE collective_id = $1")
            .bind(cid)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(destinataires, membres);

    // Avec les liens des plateformes : l'alerte sert a aller voir.
    let (body,): (String,) =
        sqlx::query_as("SELECT body FROM notifications WHERE kind = 'stream_live' LIMIT 1")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(body.contains("twitch.tv/bonsoirtechno"), "{body}");

    // La trace est posee : l'alerte ne repartira pas deux fois.
    let (sent,): (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT live_alert_sent_at FROM event_streams WHERE event_id = $1")
            .bind(event_id)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(sent.is_some());
}

#[tokio::test]
async fn un_stream_se_cree_sans_opportunite_et_porte_ses_plateformes() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let created = antoine
        .post(
            &format!("/api/collectives/{cid}/events"),
            json!({
                "event_type_key": "stream",
                "title": "Live de printemps",
                "starts_at": "2026-04-18T19:00:00Z",
                "ends_at": "2026-04-18T21:00:00Z",
                "capture_location": "Studio",
                "planned_duration_min": 120,
                "platforms": [
                    { "platform": "twitch", "url": "https://twitch.tv/bt" },
                    { "platform": "youtube", "url": "https://youtube.com/@bt/live" }
                ]
            }),
        )
        .await;
    let eid = created.expect_ok()["id"].as_str().unwrap().to_string();

    let event = antoine
        .get(&format!("/api/collectives/{cid}/events/{eid}"))
        .await;
    let event = event.expect_ok();
    assert_eq!(event["stream"]["platforms"].as_array().unwrap().len(), 2);
    // Pas de lieu a negocier : aucune opportunite n'a ete creee.
    assert!(event["venue"].is_null());

    // Les postes techniques d'un stream sont crees, pas ceux d'un concert.
    let labels: Vec<String> = event["logistics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["label"].as_str().unwrap().to_string())
        .collect();
    assert!(labels.contains(&"camera".to_string()), "{labels:?}");
    assert!(
        labels.contains(&"regie / encodage".to_string()),
        "{labels:?}"
    );

    // La timeline de com d'un stream est courte et contient le jalon a -15 min.
    let plan = antoine
        .get(&format!("/api/collectives/{cid}/events/{eid}/comms"))
        .await;
    let plan = plan.expect_ok();
    let jalons: Vec<String> = plan
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["milestone_key"].as_str().unwrap().to_string())
        .collect();
    assert!(jalons.contains(&"j0-15m".to_string()), "{jalons:?}");
    assert!(jalons.contains(&"j+1".to_string()), "replay : {jalons:?}");
}

#[tokio::test]
async fn le_lien_de_replay_se_renseigne_apres_coup() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let (event_id,): (Uuid,) = sqlx::query_as(
        "SELECT e.id FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'stream' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    let antoine = app.login_named("Antoine").await;
    antoine
        .patch(
            &format!("/api/collectives/{cid}/events/{event_id}/stream"),
            json!({ "replay_url": "https://twitch.tv/videos/12345" }),
        )
        .await
        .expect_ok();

    let event = antoine
        .get(&format!("/api/collectives/{cid}/events/{event_id}"))
        .await;
    assert_eq!(
        event.expect_ok()["stream"]["replay_url"],
        "https://twitch.tv/videos/12345"
    );
}

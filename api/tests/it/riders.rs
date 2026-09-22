//! §18: "A group's tech rider goes out as a PDF to a venue in under a minute"
//! and "An event is confirmed, filled in and communicated even if no group in
//! the line-up has a tech rider; the absence is reported, never blocking"

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn the_tech_rider_comes_out_as_a_pdf() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let ramas = app.group_id("ramas").await;
    let romain = app.login_named("Romain").await;

    let riders = romain
        .get(&format!(
            "/api/collectives/{cid}/groups/{ramas}/tech-riders"
        ))
        .await;
    let riders = riders.expect_ok();
    assert_eq!(riders[0]["status"], "published");
    let rid = riders[0]["id"].as_str().unwrap().to_string();

    // The PDF is a real PDF, not an error page.
    let resp = reqwest::Client::new()
        .get(format!(
            "{}/api/collectives/{cid}/groups/{ramas}/tech-riders/{rid}/pdf",
            app.base
        ))
        .header("Authorization", format!("Bearer {}", romain.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "typst doit etre disponible dans l'environnement"
    );
    assert_eq!(resp.headers()["content-type"], "application/pdf");
    let disposition = resp.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disposition.contains("fiche-technique-ramas-v1.pdf"),
        "{disposition}"
    );

    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.starts_with(b"%PDF"), "en-tete PDF attendue");
    assert!(
        bytes.len() > 5_000,
        "document trop leger : {} octets",
        bytes.len()
    );
}

#[tokio::test]
async fn a_published_version_is_frozen_and_the_next_one_starts_from_it() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let ramas = app.group_id("ramas").await;
    let romain = app.login_named("Romain").await;

    let riders = romain
        .get(&format!(
            "/api/collectives/{cid}/groups/{ramas}/tech-riders"
        ))
        .await;
    let rid = riders.expect_ok()[0]["id"].as_str().unwrap().to_string();

    // Editing a published version is refused: an already-sent PDF must stay
    // reproducible.
    let res = romain
        .patch(
            &format!("/api/collectives/{cid}/groups/{ramas}/tech-riders/{rid}"),
            json!({ "data": { "light": "autre chose" } }),
        )
        .await;
    res.expect_status(409);

    // Start again from v1 to write a v2.
    let created = romain
        .post(
            &format!("/api/collectives/{cid}/groups/{ramas}/tech-riders"),
            json!({ "from_version": 1 }),
        )
        .await;
    let created = created.expect_ok();
    assert_eq!(created["version"], 2);
    let v2 = created["id"].as_str().unwrap().to_string();

    let show = romain
        .get(&format!(
            "/api/collectives/{cid}/groups/{ramas}/tech-riders/{v2}"
        ))
        .await;
    let show = show.expect_ok();
    assert_eq!(show["status"], "draft");
    assert_eq!(
        show["data"]["identity"]["set_duration_min"], 60,
        "la v2 herite du contenu de la v1"
    );
}

#[tokio::test]
async fn a_missing_tech_rider_is_reported_never_blocking() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    // Dante3p has no tech rider. Confirm an event with them anyway.
    let dante = app.group_id("dante3p").await;
    let created = antoine
        .post(
            &format!("/api/collectives/{cid}/events"),
            json!({
                "event_type_key": "concert",
                "title": "Concert sans fiche",
                "starts_at": "2026-12-05T20:00:00Z"
            }),
        )
        .await;
    let eid = created.expect_ok()["id"].as_str().unwrap().to_string();

    // Nothing is refused: not the line-up…
    antoine
        .post(
            &format!("/api/collectives/{cid}/events/{eid}/participations"),
            json!({ "group_id": dante, "stage_role": "dj set" }),
        )
        .await
        .expect_ok();

    // …nor the comms.
    let plan = antoine
        .get(&format!("/api/collectives/{cid}/events/{eid}/comms"))
        .await;
    assert!(!plan.expect_ok().as_array().unwrap().is_empty());

    // But the absence is visible on the event's page…
    let event = antoine
        .get(&format!("/api/collectives/{cid}/events/{eid}"))
        .await;
    let event = event.expect_ok();
    let rider = &event["tech_riders"][0];
    assert_eq!(rider["group_name"], "Dante3p");
    assert!(
        rider["tech_rider_id"].is_null(),
        "fiche technique manquante"
    );

    // …and on the dashboard.
    let dash = antoine
        .get(&format!("/api/collectives/{cid}/dashboard"))
        .await;
    let dash = dash.expect_ok();
    let signale = dash["missing_tech_riders"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["group_name"] == "Dante3p");
    assert!(
        signale,
        "le tableau de bord doit signaler la fiche manquante"
    );

    // When sending to the venue, groups without a rider are listed explicitly.
    let sent = antoine
        .post(
            &format!("/api/collectives/{cid}/events/{eid}/tech-riders/send"),
            json!({ "sent_to": "claire@lesonic.fr" }),
        )
        .await;
    let sent = sent.expect_ok();
    assert!(sent["sent"].as_array().unwrap().is_empty());
    assert_eq!(sent["missing"][0]["name"], "Dante3p");

    // A single reminder, at D-14, then nothing more.
    app.run_due_jobs().await;
    let (reminders,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'tech_rider_missing'
           AND payload->>'event_id' = $1",
    )
    .bind(&eid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(reminders, 1, "Mathieu, Dante3p's lead, only once");

    app.run_due_jobs().await;
    let (reminders_again,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'tech_rider_missing'
           AND payload->>'event_id' = $1",
    )
    .bind(&eid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(reminders_again, 1, "no nagging");
}

#[tokio::test]
async fn publishing_a_rider_attaches_it_to_upcoming_events() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let dante = app.group_id("dante3p").await;
    let antoine = app.login_named("Antoine").await;
    let mathieu = app.login_named("Mathieu").await;

    let (event_id,): (Uuid,) = sqlx::query_as(
        "SELECT etr.event_id FROM event_tech_riders etr
         WHERE etr.group_id = $1 AND etr.tech_rider_id IS NULL LIMIT 1",
    )
    .bind(dante)
    .fetch_one(&app.db)
    .await
    .unwrap();

    let created = mathieu
        .post(
            &format!("/api/collectives/{cid}/groups/{dante}/tech-riders"),
            json!({ "data": { "identity": { "style": "techno" } } }),
        )
        .await;
    let rid = created.expect_ok()["id"].as_str().unwrap().to_string();
    mathieu
        .post(
            &format!("/api/collectives/{cid}/groups/{dante}/tech-riders/{rid}/publish"),
            json!({}),
        )
        .await
        .expect_ok();

    let event = antoine
        .get(&format!("/api/collectives/{cid}/events/{event_id}"))
        .await;
    let event = event.expect_ok();
    let rider = event["tech_riders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["group_name"] == "Dante3p")
        .unwrap();
    assert!(
        !rider["tech_rider_id"].is_null(),
        "la fiche fraiche est rattachee"
    );
}

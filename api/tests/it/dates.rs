//! §18: "A date is settled with a venue **without a single manual message**:
//! poll opened → matrix → date chosen."
//!
//! Plus the invariant that gives the matrix its value: "An availability can
//! only be written by the person it belongs to, an admin included"

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

/// Sets up an opportunity with three dates and an open poll.
async fn opportunity(app: &TestApp) -> (Uuid, Uuid, Vec<Uuid>) {
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.user_id("Antoine").await;
    let admin = app.login_as(antoine).await;

    let venue = admin
        .post(
            &format!("/api/collectives/{cid}/venues"),
            json!({ "name": "Le Sucre", "city": "Lyon", "capacity": 400 }),
        )
        .await;
    let venue_id = venue.expect_ok()["id"].as_str().unwrap().to_string();

    let opp = admin
        .post(
            &format!("/api/collectives/{cid}/opportunities"),
            json!({
                "title": "Carte blanche au Sucre",
                "venue_id": venue_id,
                "conditions": "Trois samedis proposes.",
                "candidate_dates": [
                    { "day": "2026-11-07", "start_time": "22:00:00" },
                    { "day": "2026-11-14", "start_time": "22:00:00" },
                    { "day": "2026-11-21", "start_time": "22:00:00" }
                ]
            }),
        )
        .await;
    let oid: Uuid = opp.expect_ok()["id"].as_str().unwrap().parse().unwrap();

    admin
        .post(
            &format!("/api/collectives/{cid}/opportunities/{oid}/poll"),
            json!({}),
        )
        .await
        .expect_ok();

    let detail = admin
        .get(&format!("/api/collectives/{cid}/opportunities/{oid}"))
        .await;
    let dates: Vec<Uuid> = detail.expect_ok()["candidate_dates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap().parse().unwrap())
        .collect();

    (cid, oid, dates)
}

#[tokio::test]
async fn from_poll_to_event_without_a_single_manual_message() {
    let app = TestApp::seeded().await;
    let (cid, oid, dates) = opportunity(&app).await;

    // 1. Opening the poll told everyone, on its own.
    let (notices,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE kind = 'poll_open'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(notices, 5, "one message per member of the collective");

    // 2. Everyone answers for themselves, web or bot, it makes no difference.
    for (nom, reponses, veut_jouer) in [
        ("Anas", ["no", "yes", "yes"], true),
        ("Romain", ["maybe", "yes", "yes"], true),
        ("Mathieu", ["no", "yes", "no"], true),
        ("Antoine", ["yes", "yes", "yes"], false),
    ] {
        let client = app.login_named(nom).await;
        let answers: Vec<_> = dates
            .iter()
            .zip(reponses.iter())
            .map(|(d, s)| json!({ "candidate_date_id": d, "status": s, "wants_to_play": veut_jouer }))
            .collect();
        client
            .put(
                &format!("/api/collectives/{cid}/opportunities/{oid}/availabilities"),
                json!({ "answers": answers }),
            )
            .await
            .expect_ok();
    }

    // 3. The matrix points at the date: the one where a group is **complete**.
    let anas = app.login_named("Anas").await;
    let matrix = anas
        .get(&format!(
            "/api/collectives/{cid}/opportunities/{oid}/matrix"
        ))
        .await;
    let matrix = matrix.expect_ok();

    let ramas = app.group_id("ramas").await;
    let d0 = &matrix["dates"][0];
    let d1 = &matrix["dates"][1];
    let d2 = &matrix["dates"][2];

    assert_eq!(d1["yes"], 4, "on the 14th: everyone is available");
    let complete: Vec<String> = d1["complete_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap().to_string())
        .collect();
    assert!(
        complete.contains(&ramas.to_string()),
        "Ramas doit etre complet le 14"
    );

    // On the 7th, Anas says no: Ramas is incomplete, and the app says **who** is missing.
    let partial = d0["partial_groups"].as_array().unwrap();
    let ramas_partial = partial
        .iter()
        .find(|p| p["group_id"] == json!(ramas.to_string()))
        .expect("Ramas must show up as incomplete on the 7th");
    let anas_id = app.user_id("Anas").await;
    assert_eq!(ramas_partial["missing"][0], json!(anas_id.to_string()));

    // On the 21st, Mathieu is missing: Dante3p is incomplete, Ramas is not.
    let complete_on_the_21st: Vec<String> = d2["complete_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap().to_string())
        .collect();
    assert!(complete_on_the_21st.contains(&ramas.to_string()));

    // The volunteers are identified: that is the possible line-up.
    assert_eq!(d1["volunteers"].as_array().unwrap().len(), 3);

    // 4. Conversion: the date is settled.
    let antoine = app.login_named("Antoine").await;
    let converted = antoine
        .post(
            &format!("/api/collectives/{cid}/opportunities/{oid}/convert"),
            json!({
                "candidate_date_id": dates[1],
                "event_type_key": "dj_night",
                "line_up": [ { "group_id": ramas, "stage_role": "live" } ]
            }),
        )
        .await;
    let event_id = converted.expect_ok()["event_id"]
        .as_str()
        .unwrap()
        .to_string();

    // 5. Everything confirmation must set off (§5.3) is there.
    let event = antoine
        .get(&format!("/api/collectives/{cid}/events/{event_id}"))
        .await;
    let event = event.expect_ok();
    assert_eq!(event["status"], "confirmed");
    assert!(
        !event["logistics"].as_array().unwrap().is_empty(),
        "postes crees vides"
    );
    assert!(
        event["logistics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["assignees"].as_array().unwrap().is_empty()),
        "aucun poste n'est assigne d'office"
    );
    assert!(
        event["comms_summary"]["total"].as_i64().unwrap() > 0,
        "plan de com instancie"
    );
    assert_eq!(
        event["tech_riders"].as_array().unwrap().len(),
        1,
        "fiche technique rattachee"
    );

    // Those selected **and** those not are told.
    let (selected,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE kind = 'lineup_retained'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    let (non_retenus,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE kind = 'lineup_not_retained'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(selected, 2, "Anas and Romain");
    assert_eq!(non_retenus, 1, "Mathieu s'etait porte volontaire");

    // The poll closes, the opportunity is confirmed.
    let opp = antoine
        .get(&format!("/api/collectives/{cid}/opportunities/{oid}"))
        .await;
    let opp = opp.expect_ok();
    assert_eq!(opp["status"], "confirmed");
    assert_eq!(opp["poll_open"], false);
}

#[tokio::test]
async fn an_availability_is_written_only_by_its_author_admins_included() {
    let app = TestApp::seeded().await;
    let (cid, oid, dates) = opportunity(&app).await;

    let anas = app.user_id("Anas").await;
    let antoine = app.login_named("Antoine").await; // admin du collectif

    // The admin tries to answer in Anas's place.
    let res = antoine
        .put(
            &format!("/api/collectives/{cid}/opportunities/{oid}/availabilities"),
            json!({
                "user_id": anas,
                "answers": [ { "candidate_date_id": dates[0], "status": "yes" } ]
            }),
        )
        .await;
    res.expect_status(403);
    assert!(
        res.text.contains("personne concernee"),
        "le refus doit dire pourquoi : {}",
        res.text
    );

    // Nothing was written on THIS poll (the seed data contains others, already
    // filled in by their authors).
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM availabilities a
         JOIN candidate_dates d ON d.id = a.candidate_date_id
         WHERE a.user_id = $1 AND d.opportunity_id = $2",
    )
    .bind(anas)
    .bind(oid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(n, 0);

    // Anas answers for themselves: accepted.
    let anas_client = app.login_as(anas).await;
    anas_client
        .put(
            &format!("/api/collectives/{cid}/opportunities/{oid}/availabilities"),
            json!({
                "user_id": anas,
                "answers": [ { "candidate_date_id": dates[0], "status": "yes" } ]
            }),
        )
        .await
        .expect_ok();
}

#[tokio::test]
async fn availabilities_are_visible_to_every_member() {
    let app = TestApp::seeded().await;
    let (cid, oid, dates) = opportunity(&app).await;

    let romain = app.login_named("Romain").await;
    romain
        .put(
            &format!("/api/collectives/{cid}/opportunities/{oid}/availabilities"),
            json!({ "answers": [ { "candidate_date_id": dates[0], "status": "yes" } ] }),
        )
        .await
        .expect_ok();

    // Mathieu, a plain member, sees Romain's answer: everyone can see the date
    // being decided (§5.2).
    let mathieu = app.login_named("Mathieu").await;
    let matrix = mathieu
        .get(&format!(
            "/api/collectives/{cid}/opportunities/{oid}/matrix"
        ))
        .await;
    let matrix = matrix.expect_ok();
    let romain_id = app.user_id("Romain").await;
    let line = matrix["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["user_id"] == json!(romain_id.to_string()))
        .unwrap();
    assert_eq!(line["answers"][dates[0].to_string()]["status"], "yes");
}

#[tokio::test]
async fn you_cannot_write_into_another_opportunitys_poll() {
    let app = TestApp::seeded().await;
    let (cid, oid, _) = opportunity(&app).await;

    // A candidate date belonging to the seeded opportunity, not this one.
    let (autre_date,): (Uuid,) = sqlx::query_as(
        "SELECT d.id FROM candidate_dates d JOIN opportunities o ON o.id = d.opportunity_id
         WHERE o.id <> $1 LIMIT 1",
    )
    .bind(oid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    let anas = app.login_named("Anas").await;
    let res = anas
        .put(
            &format!("/api/collectives/{cid}/opportunities/{oid}/availabilities"),
            json!({ "answers": [ { "candidate_date_id": autre_date, "status": "yes" } ] }),
        )
        .await;
    res.expect_status(400);
}

#[tokio::test]
async fn a_poll_without_a_candidate_date_does_not_open() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let opp = antoine
        .post(
            &format!("/api/collectives/{cid}/opportunities"),
            json!({ "title": "Sans date" }),
        )
        .await;
    let oid = opp.expect_ok()["id"].as_str().unwrap().to_string();

    let res = antoine
        .post(
            &format!("/api/collectives/{cid}/opportunities/{oid}/poll"),
            json!({}),
        )
        .await;
    res.expect_status(400);
}

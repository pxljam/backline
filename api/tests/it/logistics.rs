//! §18: "No logistics slot reaches the day vacant without at least **three
//! reminders** having been sent." Plus the run sheet (§5.5) and phone number
//! visibility (§3).

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
async fn three_reminders_go_out_before_the_day() {
    let app = TestApp::seeded().await;
    let (_cid, event_id) = concert(&app).await;

    // All three milestones are scheduled as soon as the event is created.
    let milestones: Vec<(String,)> = sqlx::query_as(
        "SELECT dedupe_key FROM jobs WHERE kind = 'logistics_reminder'
           AND payload->>'event_id' = $1 ORDER BY run_at",
    )
    .bind(event_id.to_string())
    .fetch_all(&app.db)
    .await
    .unwrap();
    assert_eq!(milestones.len(), 3, "J-14, J-7, J-2");
    assert!(milestones[0].0.ends_with("j-14"));
    assert!(milestones[1].0.ends_with("j-7"));
    assert!(milestones[2].0.ends_with("j-2"));

    // They go out on their own when their time comes.
    app.run_due_jobs().await;

    let sent: Vec<(String, i32)> = sqlx::query_as(
        "SELECT milestone, recipients FROM logistics_reminders WHERE event_id = $1 ORDER BY sent_at",
    )
    .bind(event_id)
    .fetch_all(&app.db)
    .await
    .unwrap();
    assert_eq!(sent.len(), 3, "trois relances effectivement envoyees");
    assert!(
        sent.iter().all(|(_, n)| *n > 0),
        "chaque relance a des destinataires"
    );
}

#[tokio::test]
async fn the_reminder_only_wakes_those_who_committed_to_nothing() {
    let app = TestApp::seeded().await;
    let (cid, _event_id) = concert(&app).await;

    // In the seed data: Romain holds "son", Antoine "photo", and Ramas (so
    // Anas and Romain) plays. That leaves Mathieu — who also plays, with
    // Dante3p. So add someone genuinely free.
    let free_member = app.make_user("Membre libre").await;
    app.join(cid, free_member, "member").await;

    app.run_due_jobs().await;

    let targets: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT user_id FROM notifications WHERE kind = 'logistics_vacant'",
    )
    .fetch_all(&app.db)
    .await
    .unwrap();
    let targets: Vec<Uuid> = targets.into_iter().map(|(u,)| u).collect();

    assert!(
        targets.contains(&free_member),
        "the free member must be reminded"
    );
    let romain = app.user_id("Romain").await;
    assert!(
        !targets.contains(&romain),
        "Romain tient deja un poste et joue : on ne le harcele pas"
    );
}

#[tokio::test]
async fn a_slot_is_taken_and_given_back_and_never_overflows() {
    let app = TestApp::seeded().await;
    let (cid, event_id) = concert(&app).await;

    let antoine = app.login_named("Antoine").await;
    let slot = antoine
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/logistics"),
            json!({ "label": "transport retour", "quantity": 1 }),
        )
        .await;
    let sid = slot.expect_ok()["id"].as_str().unwrap().to_string();

    let anas = app.login_named("Anas").await;
    anas.post(
        &format!("/api/collectives/{cid}/events/{event_id}/logistics/{sid}/take"),
        json!({}),
    )
    .await
    .expect_ok();

    // The slot is full: the next one is refused, unambiguously.
    let mathieu = app.login_named("Mathieu").await;
    let res = mathieu
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/logistics/{sid}/take"),
            json!({}),
        )
        .await;
    res.expect_status(409);

    // Anas steps back, Mathieu can take it.
    anas.delete(&format!(
        "/api/collectives/{cid}/events/{event_id}/logistics/{sid}/take"
    ))
    .await
    .expect_ok();
    mathieu
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/logistics/{sid}/take"),
            json!({}),
        )
        .await
        .expect_ok();
}

#[tokio::test]
async fn labels_already_used_are_offered_as_autocomplete() {
    let app = TestApp::seeded().await;
    let (cid, event_id) = concert(&app).await;
    let antoine = app.login_named("Antoine").await;

    let res = antoine
        .get(&format!("/api/collectives/{cid}/events/{event_id}/labels"))
        .await;
    let labels: Vec<String> = res
        .expect_ok()
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_string())
        .collect();
    // There is no catalogue: these are the labels actually entered.
    assert!(labels.contains(&"son".to_string()), "{labels:?}");
    assert!(
        labels.contains(&"camera".to_string()),
        "venus du stream : {labels:?}"
    );
}

#[tokio::test]
async fn the_run_sheet_carries_phone_numbers_for_those_entitled_to_them() {
    let app = TestApp::seeded().await;
    let (cid, event_id) = concert(&app).await;

    // An admin sees the phone numbers.
    let antoine = app.login_named("Antoine").await;
    let sheet = antoine
        .get(&format!(
            "/api/collectives/{cid}/events/{event_id}/run-sheet"
        ))
        .await;
    let sheet = sheet.expect_ok();
    assert_eq!(sheet["venue"]["name"], "Le Sonic");
    let phone_present = sheet["line_up"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l["members"].as_array().unwrap())
        .any(|m| m["phone"].is_string());
    assert!(phone_present, "the admin sees the phone numbers");

    // The venue contact too: this is the only moment they are needed.
    assert!(sheet["venue"]["contacts"][0]["phone"].is_string());

    // A member of the collective unconnected to the event does not see them.
    let outsider = app.make_user("Membre lointain").await;
    app.join(cid, outsider, "member").await;
    let client = app.login_as(outsider).await;
    let sheet = client
        .get(&format!(
            "/api/collectives/{cid}/events/{event_id}/run-sheet"
        ))
        .await;
    let sheet = sheet.expect_ok();
    let phone_present = sheet["line_up"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l["members"].as_array().unwrap())
        .any(|m| m["phone"].is_string());
    assert!(
        !phone_present,
        "les telephones ne sortent pas du cercle de l'evenement"
    );
    assert!(sheet["venue"]["contacts"][0]["phone"].is_null());
}

#[tokio::test]
async fn the_run_sheet_goes_out_the_day_before() {
    let app = TestApp::seeded().await;
    let (_cid, event_id) = concert(&app).await;

    app.run_due_jobs().await;

    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'run_sheet' AND payload->>'event_id' = $1",
    )
    .bind(event_id.to_string())
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(
        n > 0,
        "la feuille de route doit partir a tous les concernes"
    );

    // Filter on the event: the seed data also carries a residency, whose own
    // run sheet would otherwise be picked up here.
    let (body,): (String,) = sqlx::query_as(
        "SELECT body FROM notifications
         WHERE kind = 'run_sheet' AND payload->>'event_id' = $1 LIMIT 1",
    )
    .bind(event_id.to_string())
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(body.contains("Le Sonic"), "venue address: {body}");
    assert!(body.contains("Postes"), "who holds which slot: {body}");
}

#[tokio::test]
async fn a_residency_is_declared_in_one_word() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let (event_id,): (Uuid,) = sqlx::query_as(
        "SELECT e.id FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'residency' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    let mathieu = app.login_named("Mathieu").await;
    // "Je viens" is enough: no day to specify.
    mathieu
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/presence"),
            json!({ "answer": "coming" }),
        )
        .await
        .expect_ok();

    let antoine = app.login_named("Antoine").await;
    let event = antoine
        .get(&format!("/api/collectives/{cid}/events/{event_id}"))
        .await;
    let event = event.expect_ok();
    let mathieu_id = app.user_id("Mathieu").await;
    let presence = event["residency"]["presences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["user_id"] == json!(mathieu_id.to_string()))
        .unwrap();
    assert_eq!(presence["answer"], "coming");
    assert!(
        presence["days"].as_array().unwrap().is_empty(),
        "present, dates libres"
    );

    // And a residency refuses to exist without a range.
    let res = antoine
        .post(
            &format!("/api/collectives/{cid}/events"),
            json!({
                "event_type_key": "residency",
                "title": "Residence sans fin",
                "starts_at": "2026-05-01T09:00:00Z"
            }),
        )
        .await;
    res.expect_status(400);
}

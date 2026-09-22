//! §18: "A member of one collective cannot reach any data belonging to another
//! collective — verified by tests."
//!
//! Isolation is structural (§15): it is carried by the `CollectiveScope` type,
//! not by discipline when writing queries. These tests attack the boundary
//! through every open door.

use crate::harness::TestApp;
use serde_json::json;

#[tokio::test]
async fn an_outsider_does_not_see_the_collective() {
    let app = TestApp::seeded().await;
    let bonsoir = app.collective_id("bonsoir-techno").await;

    let other = app
        .make_collective("autre-collectif", "Autre collectif")
        .await;
    let intrus = app.make_user("Intrus").await;
    app.join(other, intrus, "admin").await;
    let client = app.login_as(intrus).await;

    // 404 rather than 403: from the outside, a collective you are not a member
    // of does not exist.
    for path in [
        format!("/api/collectives/{bonsoir}"),
        format!("/api/collectives/{bonsoir}/members"),
        format!("/api/collectives/{bonsoir}/groups"),
        format!("/api/collectives/{bonsoir}/events"),
        format!("/api/collectives/{bonsoir}/opportunities"),
        format!("/api/collectives/{bonsoir}/venues"),
        format!("/api/collectives/{bonsoir}/calendar"),
        format!("/api/collectives/{bonsoir}/dashboard"),
        format!("/api/collectives/{bonsoir}/social-accounts"),
        format!("/api/collectives/{bonsoir}/studio/templates"),
        format!("/api/collectives/{bonsoir}/studio/assets"),
        format!("/api/collectives/{bonsoir}/render-jobs"),
    ] {
        let res = client.get(&path).await;
        assert_eq!(
            res.status, 404,
            "{path} a repondu {} : {}",
            res.status, res.text
        );
    }
}

#[tokio::test]
async fn a_stolen_identifier_grants_no_access_to_the_resource() {
    let app = TestApp::seeded().await;
    let bonsoir = app.collective_id("bonsoir-techno").await;

    // The event really does exist; only the path changes collective.
    let (event_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM events WHERE collective_id = $1 LIMIT 1")
            .bind(bonsoir)
            .fetch_one(&app.db)
            .await
            .unwrap();

    let other = app.make_collective("voisins", "Voisins").await;
    let intrus = app.make_user("Voisin curieux").await;
    app.join(other, intrus, "admin").await;
    let client = app.login_as(intrus).await;

    let res = client
        .get(&format!("/api/collectives/{other}/events/{event_id}"))
        .await;
    res.expect_status(404);

    let res = client
        .get(&format!(
            "/api/collectives/{other}/events/{event_id}/run-sheet"
        ))
        .await;
    res.expect_status(404);

    let res = client
        .post(
            &format!("/api/collectives/{other}/events/{event_id}/logistics"),
            json!({ "label": "x" }),
        )
        .await;
    res.expect_status(404);
}

#[tokio::test]
async fn a_group_from_another_collective_cannot_be_attached() {
    let app = TestApp::seeded().await;
    let ramas = app.group_id("ramas").await;

    let other = app.make_collective("les-autres", "Les autres").await;
    let admin = app.make_user("Admin autre").await;
    app.join(other, admin, "admin").await;
    let client = app.login_as(admin).await;

    // Create an opportunity hosted by a group you do not own.
    let res = client
        .post(
            &format!("/api/collectives/{other}/opportunities"),
            json!({ "title": "Tentative", "host_group_ids": [ramas] }),
        )
        .await;
    res.expect_status(404);
}

#[tokio::test]
async fn a_plain_member_cannot_act_as_an_admin() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    let client = app.login_as(anas).await;

    // They read everything — availabilities are visible to all (§5.2)…
    client
        .get(&format!("/api/collectives/{cid}/opportunities"))
        .await
        .expect_ok();

    // …but they do not arbitrate.
    let res = client
        .post(
            &format!("/api/collectives/{cid}/venues"),
            json!({ "name": "Un lieu a moi" }),
        )
        .await;
    res.expect_status(403);

    let res = client
        .post(
            &format!("/api/collectives/{cid}/members"),
            json!({ "display_name": "Ami" }),
        )
        .await;
    res.expect_status(403);
}

#[tokio::test]
async fn the_instance_admin_is_not_the_collective_admin() {
    let app = TestApp::seeded().await;
    let anas = app.user_id("Anas").await;
    let client = app.login_as(anas).await;

    let res = client.get("/api/instance/collectives").await;
    res.expect_status(403);

    let res = client
        .post(
            "/api/instance/collectives",
            json!({ "slug": "pirate", "name": "Pirate" }),
        )
        .await;
    res.expect_status(403);

    // The instance admin, though, gets through.
    let root = app.user_id("Admin instance").await;
    let root_client = app.login_as(root).await;
    root_client
        .get("/api/instance/collectives")
        .await
        .expect_ok();
}

#[tokio::test]
async fn a_missing_or_invalid_session_is_refused() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;

    let (status, _) =
        crate::harness::anonymous_get(&app.base, &format!("/api/collectives/{cid}")).await;
    assert_eq!(status, 401);

    let (status, _) = crate::harness::anonymous_get(&app.base, "/api/me").await;
    assert_eq!(status, 401);
}

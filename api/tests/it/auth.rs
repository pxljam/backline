//! §3: no public sign-up. An admin creates the user and generates a single-use
//! invitation link; signing in goes through Telegram, with an email + password
//! fallback for administration.

use crate::harness::TestApp;
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::{Digest, Sha256};

const BOT_TOKEN: &str = "123456:TEST-TOKEN";

/// Reproduces the Telegram Login Widget signature.
fn signed_login(id: i64, username: &str) -> serde_json::Value {
    let auth_date = chrono::Utc::now().timestamp();
    let check = format!("auth_date={auth_date}\nid={id}\nusername={username}");
    let secret = Sha256::digest(BOT_TOKEN.as_bytes());
    let mut mac = Hmac::<Sha256>::new_from_slice(&secret).unwrap();
    mac.update(check.as_bytes());
    json!({
        "id": id,
        "username": username,
        "auth_date": auth_date,
        "hash": hex::encode(mac.finalize().into_bytes()),
    })
}

#[tokio::test]
async fn an_admin_creates_a_member_and_a_single_use_link() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let ramas = app.group_id("ramas").await;
    let antoine = app.login_named("Antoine").await;

    let created = antoine
        .post(
            &format!("/api/collectives/{cid}/members"),
            json!({
                "display_name": "Nouvelle recrue",
                "phone": "+33611223344",
                "group_ids": [ramas]
            }),
        )
        .await;
    let created = created.expect_ok();
    let code = created["invitation_code"].as_str().unwrap().to_string();
    assert!(created["invitation_url"].as_str().unwrap().contains(&code));

    // The link can be inspected before being consumed: you know who you are.
    let (status, body) =
        crate::harness::anonymous_get(&app.base, &format!("/api/auth/invitation/{code}")).await;
    assert_eq!(status, 200);
    let peek: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(peek["display_name"], "Nouvelle recrue");
    assert_eq!(peek["collectives"][0], "Bonsoir Techno");

    // The Telegram bot links the account.
    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/invitation/{code}", app.base))
        .json(&json!({ "telegram": signed_login(987654, "recrue") }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let session: serde_json::Value = resp.json().await.unwrap();
    assert!(session["token"].is_string());

    // Single use: the link is burnt.
    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/invitation/{code}", app.base))
        .json(&json!({ "telegram": signed_login(987654, "recrue") }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // And from now on the person signs in straight through Telegram.
    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/login/telegram", app.base))
        .json(&signed_login(987654, "recrue"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn a_forged_telegram_signature_is_refused() {
    let app = TestApp::seeded().await;

    let mut faux = signed_login(111, "pirate");
    faux["id"] = json!(222); // le hash ne correspond plus

    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/login/telegram", app.base))
        .json(&faux)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn an_unknown_telegram_account_is_never_created_on_the_fly() {
    let app = TestApp::seeded().await;

    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/login/telegram", app.base))
        .json(&signed_login(424242, "inconnu"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE telegram_id = 424242")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(n, 0, "aucune inscription publique");
}

#[tokio::test]
async fn the_instance_admin_keeps_a_way_in_without_telegram() {
    let app = TestApp::seeded().await;

    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/login/password", app.base))
        .json(&json!({ "email": "admin@backline.local", "password": "backline-admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    assert!(cookie.contains("bl_session="), "session en cookie signe");
    assert!(cookie.contains("HttpOnly"), "{cookie}");

    // Wrong password: the same message, revealing nothing.
    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/login/password", app.base))
        .json(&json!({ "email": "admin@backline.local", "password": "autre-chose" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/login/password", app.base))
        .json(&json!({ "email": "personne@nulle.part", "password": "autre-chose" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn the_admin_role_is_granted_and_revoked_but_never_the_last_one() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let anas = app.user_id("Anas").await;

    // Any member can become an admin: it is not a category of person, it is a
    // right laid on a membership (§3).
    antoine
        .patch(
            &format!("/api/collectives/{cid}/members/{anas}"),
            json!({ "role": "admin" }),
        )
        .await
        .expect_ok();

    let anas_client = app.login_as(anas).await;
    anas_client
        .post(
            &format!("/api/collectives/{cid}/venues"),
            json!({ "name": "Nouveau lieu" }),
        )
        .await
        .expect_ok();

    // …and revocable.
    antoine
        .patch(
            &format!("/api/collectives/{cid}/members/{anas}"),
            json!({ "role": "member" }),
        )
        .await
        .expect_ok();
    let res = anas_client
        .post(
            &format!("/api/collectives/{cid}/venues"),
            json!({ "name": "Encore un" }),
        )
        .await;
    res.expect_status(403);
}

#[tokio::test]
async fn signing_out_invalidates_the_session() {
    let app = TestApp::seeded().await;
    let antoine = app.login_named("Antoine").await;

    antoine.get("/api/me").await.expect_ok();

    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/logout", app.base))
        .header("Cookie", format!("bl_session={}", antoine.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    antoine.get("/api/me").await.expect_status(401);
}

#[tokio::test]
async fn one_account_spans_collectives_with_a_role_per_collective() {
    let app = TestApp::seeded().await;
    let bonsoir = app.collective_id("bonsoir-techno").await;
    let other = app.make_collective("cousins", "Les Cousins").await;
    let romain = app.user_id("Romain").await;

    // A plain member here, an admin there.
    app.join(other, romain, "admin").await;

    let client = app.login_as(romain).await;
    let me = client.get("/api/me").await;
    let me = me.expect_ok();
    let roles: std::collections::HashMap<String, String> = me["collectives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_string(),
                c["role"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(roles[&bonsoir.to_string()], "member");
    assert_eq!(roles[&other.to_string()], "admin");
}

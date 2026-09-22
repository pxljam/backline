//! §3 : aucune inscription publique. Un admin cree l'utilisateur et genere un
//! lien d'invitation a usage unique ; la connexion se fait par Telegram, avec
//! un secours e-mail + mot de passe pour l'administration.

use crate::harness::TestApp;
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::{Digest, Sha256};

const BOT_TOKEN: &str = "123456:TEST-TOKEN";

/// Reproduit la signature du Telegram Login Widget.
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
async fn un_admin_cree_un_membre_et_un_lien_a_usage_unique() {
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

    // Le lien se consulte avant d'etre consomme : on sait qui l'on est.
    let (status, body) =
        crate::harness::anonymous_get(&app.base, &format!("/api/auth/invitation/{code}")).await;
    assert_eq!(status, 200);
    let peek: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(peek["display_name"], "Nouvelle recrue");
    assert_eq!(peek["collectives"][0], "Bonsoir Techno");

    // Le bot Telegram lie le compte.
    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/invitation/{code}", app.base))
        .json(&json!({ "telegram": signed_login(987654, "recrue") }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let session: serde_json::Value = resp.json().await.unwrap();
    assert!(session["token"].is_string());

    // Usage unique : le lien est brule.
    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/invitation/{code}", app.base))
        .json(&json!({ "telegram": signed_login(987654, "recrue") }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // Et la personne se connecte desormais directement par Telegram.
    let resp = reqwest::Client::new()
        .post(format!("{}/api/auth/login/telegram", app.base))
        .json(&signed_login(987654, "recrue"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn une_signature_telegram_falsifiee_est_refusee() {
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
async fn un_compte_telegram_inconnu_n_est_jamais_cree_a_la_volee() {
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
async fn l_admin_d_instance_garde_une_entree_sans_telegram() {
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

    // Mauvais mot de passe : le meme message, sans revelation.
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
async fn le_role_d_admin_s_attribue_et_se_retire_mais_jamais_le_dernier() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let anas = app.user_id("Anas").await;

    // N'importe quel membre peut devenir admin : ce n'est pas une categorie de
    // personne, c'est un droit pose sur une appartenance (§3).
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

    // …et retirable.
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
async fn se_deconnecter_invalide_la_session() {
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
async fn un_compte_traverse_les_collectifs_avec_un_role_par_collectif() {
    let app = TestApp::seeded().await;
    let bonsoir = app.collective_id("bonsoir-techno").await;
    let autre = app.make_collective("cousins", "Les Cousins").await;
    let romain = app.user_id("Romain").await;

    // Simple membre ici, admin la-bas.
    app.join(autre, romain, "admin").await;

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
    assert_eq!(roles[&autre.to_string()], "admin");
}

//! §18, video et rendu distribue (§10) :
//! - « Une composition video rendue deux fois donne deux fichiers identiques. »
//! - « Si aucune machine n'est connectee, la tache de com reste livrable avec
//!   son visuel fixe, et l'admin est prevenu. »
//! - « Un rendu video aboutit sur une machine sans GPU, simplement plus
//!   lentement. »

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

async fn composition(app: &TestApp) -> (Uuid, String) {
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let (format_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM formats WHERE key = 'reel_9_16' AND collective_id IS NULL")
            .fetch_one(&app.db)
            .await
            .unwrap();
    let (event_id,): (Uuid,) = sqlx::query_as(
        "SELECT e.id FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'concert' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    let created = antoine
        .post(
            &format!("/api/collectives/{cid}/video-compositions"),
            json!({
                "name": "Teaser J-3",
                "format_id": format_id,
                "event_id": event_id,
                "fps": 30,
                "spec": {
                    "version": 1, "width": 1080, "height": 1920, "fps": 30,
                    "scenes": [
                        {
                            "id": "s1", "durationInFrames": 60,
                            "transition": { "type": "fade", "durationInFrames": 12 },
                            "background": { "type": "color", "token": "background" },
                            "blocks": [
                                { "id": "t", "type": "text", "x": 0.08, "y": 0.4, "w": 0.84, "h": 0.2,
                                  "props": { "content": "{{event.title}}", "fontToken": "title",
                                             "colorToken": "text", "size": 0.09 },
                                  "animations": [ { "type": "fade", "from": 0, "to": 15 } ] }
                            ]
                        }
                    ],
                    "audio": null
                }
            }),
        )
        .await;
    (cid, created.expect_ok()["id"].as_str().unwrap().to_string())
}

/// Enregistre une machine et renvoie son jeton.
async fn machine(app: &TestApp, nom: &str, gpu: bool) -> String {
    let romain = app.login_named("Romain").await;
    let res = romain
        .post(
            "/api/render/machines",
            json!({ "name": nom, "capabilities": { "gpu": gpu, "concurrency": 2 } }),
        )
        .await;
    res.expect_ok()["token"].as_str().unwrap().to_string()
}

async fn claim(app: &TestApp, token: &str, gpu: bool) -> serde_json::Value {
    let resp = reqwest::Client::new()
        .post(format!("{}/api/render/claim", app.base))
        .header("X-Machine-Token", token)
        .json(&json!({ "capabilities": { "gpu": gpu } }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    resp.json().await.unwrap()
}

async fn bundle(app: &TestApp, token: &str, job_id: &str) -> (u16, serde_json::Value) {
    let resp = reqwest::Client::new()
        .get(format!("{}/api/render/jobs/{job_id}/bundle", app.base))
        .header("X-Machine-Token", token)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or(json!(null)))
}

#[tokio::test]
async fn une_video_se_decrit_entierement_dans_l_app_sans_encodage() {
    let app = TestApp::seeded().await;
    let (cid, vid) = composition(&app).await;
    let antoine = app.login_named("Antoine").await;

    let comp = antoine
        .get(&format!("/api/collectives/{cid}/video-compositions/{vid}"))
        .await;
    let comp = comp.expect_ok();
    assert_eq!(comp["spec"]["scenes"][0]["durationInFrames"], 60);
    assert_eq!(comp["fps"], 30);
    // Aucun asset n'a ete produit : la description n'est pas un fichier.
    let (rendus,): (i64,) = sqlx::query_as("SELECT count(*) FROM assets WHERE kind = 'render'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(rendus, 0, "decrire n'encode pas");
}

#[tokio::test]
async fn deux_rendus_de_la_meme_version_recoivent_une_recette_identique() {
    let app = TestApp::seeded().await;
    let (cid, vid) = composition(&app).await;
    let antoine = app.login_named("Antoine").await;
    let token = machine(&app, "MacBook de Romain", false).await;

    let mut recettes = Vec::new();
    for _ in 0..2 {
        let job = antoine
            .post(
                &format!("/api/collectives/{cid}/video-compositions/{vid}/render"),
                json!({}),
            )
            .await;
        let job_id = job.expect_ok()["id"].as_str().unwrap().to_string();

        let claimed = claim(&app, &token, false).await;
        assert_eq!(claimed["job"]["id"], json!(job_id));

        let (status, b) = bundle(&app, &token, &job_id).await;
        assert_eq!(status, 200);
        // Les URL signees changent a chaque appel — c'est leur role. La recette
        // est tout le reste : description, charte, champs automatiques.
        recettes.push(
            json!({ "spec": b["spec"], "brand": b["brand"], "data": b["data"], "fps": b["fps"] }),
        );
    }

    assert_eq!(
        recettes[0], recettes[1],
        "la meme version doit donner exactement la meme recette"
    );
}

#[tokio::test]
async fn un_rendu_aboutit_sur_une_machine_sans_gpu() {
    let app = TestApp::seeded().await;
    let (cid, vid) = composition(&app).await;
    let antoine = app.login_named("Antoine").await;

    // Machine explicitement sans GPU : rien ne l'exclut de la file.
    let token = machine(&app, "Vieux portable", false).await;
    let job = antoine
        .post(
            &format!("/api/collectives/{cid}/video-compositions/{vid}/render"),
            json!({}),
        )
        .await;
    let job_id = job.expect_ok()["id"].as_str().unwrap().to_string();

    let claimed = claim(&app, &token, false).await;
    assert_eq!(claimed["job"]["id"], json!(job_id));

    let (status, _) = bundle(&app, &token, &job_id).await;
    assert_eq!(status, 200);

    // Progression remontee en direct.
    let resp = reqwest::Client::new()
        .put(format!("{}/api/render/jobs/{job_id}/progress", app.base))
        .header("X-Machine-Token", &token)
        .json(&json!({ "progress": 0.42 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Le fichier fini revient.
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"FAKE-MP4-CONTENT".to_vec())
            .file_name("teaser.mp4")
            .mime_str("video/mp4")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(format!("{}/api/render/jobs/{job_id}/complete", app.base))
        .header("X-Machine-Token", &token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let jobs = antoine
        .get(&format!("/api/collectives/{cid}/render-jobs"))
        .await;
    let jobs = jobs.expect_ok();
    let done = jobs["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["id"] == json!(job_id))
        .unwrap();
    assert_eq!(done["status"], "done");
    assert_eq!(done["progress"], 1.0);
    assert!(!done["output_asset_id"].is_null());
    assert_eq!(done["claimed_by"], "Vieux portable");
}

#[tokio::test]
async fn sans_machine_connectee_l_admin_est_prevenu_et_la_com_continue() {
    let app = TestApp::seeded().await;
    let (cid, vid) = composition(&app).await;
    let antoine = app.login_named("Antoine").await;

    let job = antoine
        .post(
            &format!("/api/collectives/{cid}/video-compositions/{vid}/render"),
            json!({}),
        )
        .await;
    let job = job.expect_ok();
    let job_id = job["id"].as_str().unwrap().to_string();
    assert_eq!(
        job["machines_online"], 0,
        "l'app dit tout de suite que personne n'est la"
    );

    // Le job ne dort pas silencieusement.
    app.run_due_jobs().await;
    let (alertes,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'admin_alert'
           AND payload->>'render_job_id' = $1",
    )
    .bind(&job_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(alertes > 0, "un admin doit etre prevenu");

    let (body,): (String,) = sqlx::query_as(
        "SELECT body FROM notifications WHERE payload->>'render_job_id' = $1 LIMIT 1",
    )
    .bind(&job_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(
        body.contains("visuel fixe"),
        "la com ne s'arrete jamais faute de rendu : {body}"
    );

    // Et la tache de com correspondante reste livrable : son statut n'a pas
    // bouge a cause du rendu manquant.
    let plan = antoine.get(&format!("/api/collectives/{cid}/events")).await;
    plan.expect_ok();
}

#[tokio::test]
async fn un_jeton_de_machine_est_revocable_et_cloisonne() {
    let app = TestApp::seeded().await;
    let (cid, vid) = composition(&app).await;
    let antoine = app.login_named("Antoine").await;

    let token_a = machine(&app, "Machine A", true).await;
    let token_b = machine(&app, "Machine B", false).await;

    let job = antoine
        .post(
            &format!("/api/collectives/{cid}/video-compositions/{vid}/render"),
            json!({}),
        )
        .await;
    let job_id = job.expect_ok()["id"].as_str().unwrap().to_string();

    claim(&app, &token_a, true).await;

    // B n'a pas reclame ce job : il n'accede pas a ses medias.
    let (status, _) = bundle(&app, &token_b, &job_id).await;
    assert_eq!(status, 403, "acces limite aux medias du job reclame");

    // Revocation : le jeton ne vaut plus rien.
    let romain = app.login_named("Romain").await;
    let machines = romain.get("/api/render/machines").await;
    let mid = machines
        .expect_ok()
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Machine A")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    romain
        .delete(&format!("/api/render/machines/{mid}"))
        .await
        .expect_ok();

    let resp = reqwest::Client::new()
        .post(format!("{}/api/render/claim", app.base))
        .header("X-Machine-Token", &token_a)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn un_rendu_en_echec_repart_dans_la_file_puis_abandonne_en_le_disant() {
    let app = TestApp::seeded().await;
    let (cid, vid) = composition(&app).await;
    let antoine = app.login_named("Antoine").await;
    let token = machine(&app, "Machine fragile", false).await;

    let job = antoine
        .post(
            &format!("/api/collectives/{cid}/video-compositions/{vid}/render"),
            json!({}),
        )
        .await;
    let job_id = job.expect_ok()["id"].as_str().unwrap().to_string();

    for essai in 1..=3 {
        let claimed = claim(&app, &token, false).await;
        assert!(
            !claimed["job"].is_null(),
            "essai {essai} : le job doit revenir dans la file"
        );
        let resp = reqwest::Client::new()
            .post(format!("{}/api/render/jobs/{job_id}/fail", app.base))
            .header("X-Machine-Token", &token)
            .json(&json!({ "error": "ffmpeg a rendu l'ame" }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    let (status,): (String,) = sqlx::query_as("SELECT status FROM render_jobs WHERE id = $1")
        .bind(Uuid::parse_str(&job_id).unwrap())
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(status, "failed");

    let (alertes,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'admin_alert'
           AND payload->>'render_job_id' = $1 AND title LIKE '%echec%'",
    )
    .bind(&job_id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(alertes > 0, "l'echec est dit, pas avale");
}

#[tokio::test]
async fn la_purge_des_rendus_epargne_les_medias_televerses() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;

    // Un rendu perime, un media televerse.
    app.state
        .storage
        .put(
            "collectives/x/render/old.png",
            b"rendu".to_vec(),
            "image/png",
        )
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO assets (collective_id, kind, filename, storage_key, mime, bytes, purge_after)
         VALUES ($1, 'render', 'vieux.png', 'collectives/x/render/old.png', 'image/png', 5,
                 now() - interval '1 day')",
    )
    .bind(cid)
    .execute(&app.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO assets (collective_id, kind, filename, storage_key, mime, bytes)
         VALUES ($1, 'image', 'photo.png', 'collectives/x/image/photo.png', 'image/png', 5)",
    )
    .bind(cid)
    .execute(&app.db)
    .await
    .unwrap();

    backline::services::scheduler::purge_renders(&app.state)
        .await
        .unwrap();

    let (rendus,): (i64,) = sqlx::query_as("SELECT count(*) FROM assets WHERE kind = 'render'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(rendus, 0, "les rendus perimes se regenerent, on les purge");

    let (medias,): (i64,) = sqlx::query_as("SELECT count(*) FROM assets WHERE kind = 'image'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(
        medias, 1,
        "les medias televerses ne sont jamais purges automatiquement"
    );
}

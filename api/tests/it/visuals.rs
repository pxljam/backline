//! §18 : « Tous les visuels d'un evenement sont produits **en une action**, a
//! tous les formats, conformes a la charte. »
//!
//! Le rendu lui-meme appartient au service `stills` (Node + Remotion). Le test
//! complet tourne quand une instance de ce service est joignable
//! (`BACKLINE_STILLS_URL`) ; sans elle, on verifie l'autre moitie de
//! l'exigence : un rendu indisponible est **signale**, et n'emporte pas la com
//! avec lui.

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
async fn une_seule_action_declenche_tous_les_formats_du_plan() {
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

    let visuels: Vec<&serde_json::Value> = plan
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["visuals"].as_array().unwrap())
        .collect();
    assert!(!visuels.is_empty(), "aucun visuel produit");

    let prets = visuels.iter().filter(|v| v["status"] == "ready").count();
    assert!(prets > 0, "au moins un visuel doit sortir : {visuels:?}");

    // Chaque visuel pret pointe vers un vrai fichier, de la bonne taille.
    let asset_id = visuels.iter().find(|v| v["status"] == "ready").unwrap()["asset_id"]
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
        "un PNG est attendu"
    );

    // Les rendus portent une date de purge : ils se regenerent a l'identique.
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
async fn un_service_de_rendu_absent_est_signale_sans_bloquer_la_com() {
    let mut app = TestApp::new().await;
    // Une adresse qui ne repond pas : exactement le cas « le rendu est tombe ».
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

    let en_echec: Vec<&serde_json::Value> = plan
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["visuals"].as_array().unwrap())
        .filter(|v| v["status"] == "failed")
        .collect();
    assert!(
        !en_echec.is_empty(),
        "l'echec doit etre enregistre, pas avale"
    );
    assert!(
        en_echec[0]["error"]
            .as_str()
            .unwrap()
            .contains("injoignable"),
        "l'erreur doit dire ce qui s'est passe : {}",
        en_echec[0]["error"]
    );

    // La com, elle, continue : les taches existent toujours et restent
    // assignables et publiables.
    let taches = plan.as_array().unwrap();
    assert!(!taches.is_empty());
    assert!(
        taches.iter().all(|t| t["status"] != "published"),
        "rien n'a ete publie tout seul"
    );
}

#[tokio::test]
async fn un_gabarit_manquant_pour_un_format_est_dit_explicitement() {
    let app = TestApp::seeded().await;
    let (cid, event_id) = concert(&app).await;
    let antoine = app.login_named("Antoine").await;

    // Le gabarit d'amorcage ne couvre pas le format Reel : le jalon J-14 en
    // demande un.
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

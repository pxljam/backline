//! §18 : « La fiche technique d'un groupe part en PDF vers un lieu en moins
//! d'une minute » et « Un evenement se confirme, se remplit et se communique
//! meme si aucun groupe du line-up n'a de fiche technique ; l'absence est
//! signalee, jamais bloquante. »

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn la_fiche_technique_sort_en_pdf() {
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

    // Le PDF est un vrai PDF, pas une page d'erreur.
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
async fn une_version_publiee_est_figee_et_la_suivante_repart_d_elle() {
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

    // Modifier une version publiee est refuse : un PDF deja envoye doit rester
    // reproductible.
    let res = romain
        .patch(
            &format!("/api/collectives/{cid}/groups/{ramas}/tech-riders/{rid}"),
            json!({ "data": { "light": "autre chose" } }),
        )
        .await;
    res.expect_status(409);

    // On repart de la v1 pour ecrire une v2.
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
async fn l_absence_de_fiche_technique_est_signalee_jamais_bloquante() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    // Dante3p n'a pas de fiche technique. On confirme un evenement avec lui.
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

    // Rien n'est refuse : ni le line-up…
    antoine
        .post(
            &format!("/api/collectives/{cid}/events/{eid}/participations"),
            json!({ "group_id": dante, "stage_role": "dj set" }),
        )
        .await
        .expect_ok();

    // …ni la com.
    let plan = antoine
        .get(&format!("/api/collectives/{cid}/events/{eid}/comms"))
        .await;
    assert!(!plan.expect_ok().as_array().unwrap().is_empty());

    // Mais l'absence est visible sur la fiche de l'evenement…
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

    // …et dans le tableau de bord.
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

    // A l'envoi au lieu, les groupes sans fiche sont listes explicitement.
    let sent = antoine
        .post(
            &format!("/api/collectives/{cid}/events/{eid}/tech-riders/send"),
            json!({ "sent_to": "claire@lesonic.fr" }),
        )
        .await;
    let sent = sent.expect_ok();
    assert!(sent["sent"].as_array().unwrap().is_empty());
    assert_eq!(sent["missing"][0]["name"], "Dante3p");

    // Un seul rappel, a J-14, puis plus rien.
    app.run_due_jobs().await;
    let (rappels,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'tech_rider_missing'
           AND payload->>'event_id' = $1",
    )
    .bind(&eid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(rappels, 1, "Mathieu, referent de Dante3p, une seule fois");

    app.run_due_jobs().await;
    let (rappels2,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'tech_rider_missing'
           AND payload->>'event_id' = $1",
    )
    .bind(&eid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(rappels2, 1, "pas de harcelement");
}

#[tokio::test]
async fn publier_une_fiche_la_rattache_aux_evenements_a_venir() {
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

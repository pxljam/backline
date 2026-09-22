//! §18 : « Aucun poste logistique n'arrive vacant au jour J sans qu'au moins
//! **trois relances** aient ete envoyees. » Plus la feuille de route (§5.5) et
//! la visibilite des telephones (§3).

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
async fn trois_relances_partent_avant_le_jour_j() {
    let app = TestApp::seeded().await;
    let (_cid, event_id) = concert(&app).await;

    // Les trois jalons sont programmes des la creation de l'evenement.
    let jalons: Vec<(String,)> = sqlx::query_as(
        "SELECT dedupe_key FROM jobs WHERE kind = 'logistics_reminder'
           AND payload->>'event_id' = $1 ORDER BY run_at",
    )
    .bind(event_id.to_string())
    .fetch_all(&app.db)
    .await
    .unwrap();
    assert_eq!(jalons.len(), 3, "J-14, J-7, J-2");
    assert!(jalons[0].0.ends_with("j-14"));
    assert!(jalons[1].0.ends_with("j-7"));
    assert!(jalons[2].0.ends_with("j-2"));

    // Ils partent tout seuls quand leur heure arrive.
    app.run_due_jobs().await;

    let envoyees: Vec<(String, i32)> = sqlx::query_as(
        "SELECT milestone, recipients FROM logistics_reminders WHERE event_id = $1 ORDER BY sent_at",
    )
    .bind(event_id)
    .fetch_all(&app.db)
    .await
    .unwrap();
    assert_eq!(envoyees.len(), 3, "trois relances effectivement envoyees");
    assert!(
        envoyees.iter().all(|(_, n)| *n > 0),
        "chaque relance a des destinataires"
    );
}

#[tokio::test]
async fn la_relance_ne_reveille_que_ceux_qui_ne_se_sont_engages_sur_rien() {
    let app = TestApp::seeded().await;
    let (cid, _event_id) = concert(&app).await;

    // Dans les donnees d'amorcage : Romain tient « son », Antoine « photo »,
    // et Ramas (donc Anas et Romain) joue. Reste Mathieu — qui joue aussi,
    // avec Dante3p. On ajoute donc quelqu'un de reellement libre.
    let libre = app.make_user("Membre libre").await;
    app.join(cid, libre, "member").await;

    app.run_due_jobs().await;

    let cibles: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT user_id FROM notifications WHERE kind = 'logistics_vacant'",
    )
    .fetch_all(&app.db)
    .await
    .unwrap();
    let cibles: Vec<Uuid> = cibles.into_iter().map(|(u,)| u).collect();

    assert!(cibles.contains(&libre), "le membre libre doit etre relance");
    let romain = app.user_id("Romain").await;
    assert!(
        !cibles.contains(&romain),
        "Romain tient deja un poste et joue : on ne le harcele pas"
    );
}

#[tokio::test]
async fn un_poste_se_prend_et_se_rend_et_ne_deborde_pas() {
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

    // Le poste est complet : le suivant est refuse, sans ambiguite.
    let mathieu = app.login_named("Mathieu").await;
    let res = mathieu
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/logistics/{sid}/take"),
            json!({}),
        )
        .await;
    res.expect_status(409);

    // Anas se retire, Mathieu peut prendre.
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
async fn les_libelles_deja_utilises_sont_proposes_en_autocompletion() {
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
    // Un catalogue n'existe pas : ce sont les libelles reellement saisis.
    assert!(labels.contains(&"son".to_string()), "{labels:?}");
    assert!(
        labels.contains(&"camera".to_string()),
        "venus du stream : {labels:?}"
    );
}

#[tokio::test]
async fn la_feuille_de_route_porte_les_telephones_pour_ceux_qui_y_ont_droit() {
    let app = TestApp::seeded().await;
    let (cid, event_id) = concert(&app).await;

    // Un admin voit les telephones.
    let antoine = app.login_named("Antoine").await;
    let sheet = antoine
        .get(&format!(
            "/api/collectives/{cid}/events/{event_id}/run-sheet"
        ))
        .await;
    let sheet = sheet.expect_ok();
    assert_eq!(sheet["venue"]["name"], "Le Sonic");
    let telephone_present = sheet["line_up"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l["members"].as_array().unwrap())
        .any(|m| m["phone"].is_string());
    assert!(telephone_present, "l'admin voit les telephones");

    // Le contact du lieu aussi : c'est le seul moment ou on en a besoin.
    assert!(sheet["venue"]["contacts"][0]["phone"].is_string());

    // Un membre du collectif etranger a l'evenement ne les voit pas.
    let etranger = app.make_user("Membre lointain").await;
    app.join(cid, etranger, "member").await;
    let client = app.login_as(etranger).await;
    let sheet = client
        .get(&format!(
            "/api/collectives/{cid}/events/{event_id}/run-sheet"
        ))
        .await;
    let sheet = sheet.expect_ok();
    let telephone_present = sheet["line_up"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|l| l["members"].as_array().unwrap())
        .any(|m| m["phone"].is_string());
    assert!(
        !telephone_present,
        "les telephones ne sortent pas du cercle de l'evenement"
    );
    assert!(sheet["venue"]["contacts"][0]["phone"].is_null());
}

#[tokio::test]
async fn la_feuille_de_route_part_la_veille() {
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

    let (body,): (String,) =
        sqlx::query_as("SELECT body FROM notifications WHERE kind = 'run_sheet' LIMIT 1")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(body.contains("Le Sonic"), "adresse du lieu : {body}");
    assert!(body.contains("Postes"), "qui tient quel poste : {body}");
}

#[tokio::test]
async fn une_residence_se_declare_d_un_seul_mot() {
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
    // « Je viens » suffit : aucun jour a preciser.
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

    // Et une residence refuse d'exister sans plage.
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

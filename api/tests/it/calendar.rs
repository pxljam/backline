//! §18 : « Un membre voit ses evenements dans son Google Calendar sans s'etre
//! connecte a autre chose que le bot. » L'abonnement iCal porte un jeton
//! secret : c'est toute l'authentification (§7).

use crate::harness::TestApp;
use serde_json::json;

#[tokio::test]
async fn chaque_membre_chaque_groupe_et_le_collectif_ont_leur_flux() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let romain = app.login_named("Romain").await;

    let feeds = romain
        .get(&format!("/api/collectives/{cid}/calendar/feeds"))
        .await;
    let feeds = feeds.expect_ok();
    let scopes: Vec<&str> = feeds
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["scope"].as_str().unwrap())
        .collect();
    assert!(scopes.contains(&"collective"));
    assert!(scopes.contains(&"user"));
    assert!(
        scopes.contains(&"group"),
        "Romain est dans Ramas : {scopes:?}"
    );
}

#[tokio::test]
async fn le_flux_personnel_est_lisible_par_un_agenda_sans_aucune_connexion() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let romain = app.login_named("Romain").await;

    let feeds = romain
        .get(&format!("/api/collectives/{cid}/calendar/feeds"))
        .await;
    let url = feeds
        .expect_ok()
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["scope"] == "user")
        .unwrap()["url"]
        .as_str()
        .unwrap()
        .to_string();
    let path = url.split("/ical/").nth(1).unwrap().to_string();

    // Aucune session, aucun cookie : juste le jeton dans l'URL.
    let (status, body) = crate::harness::anonymous_get(&app.base, &format!("/ical/{path}")).await;
    assert_eq!(status, 200);
    assert!(
        body.starts_with("BEGIN:VCALENDAR"),
        "{}",
        &body[..60.min(body.len())]
    );
    assert!(
        body.contains("Ramas"),
        "Romain joue avec Ramas, il doit voir le concert"
    );
    assert!(body.contains("LOCATION:Le Sonic"), "{body}");

    // Les lignes respectent le pliage de la RFC 5545 : sinon Google ignore le
    // flux en silence.
    for ligne in body.split("\r\n") {
        assert!(ligne.len() <= 75, "ligne trop longue : {ligne}");
    }
}

#[tokio::test]
async fn un_jeton_inconnu_ne_donne_rien() {
    let app = TestApp::seeded().await;
    let (status, _) = crate::harness::anonymous_get(&app.base, "/ical/nimportequoi.ics").await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn le_calendrier_montre_les_dates_candidates_en_arbitrage() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.login_named("Anas").await;

    let cal = anas.get(&format!("/api/collectives/{cid}/calendar")).await;
    let cal = cal.expect_ok();
    let kinds: Vec<&str> = cal
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"event"));
    assert!(
        kinds.contains(&"candidate_date"),
        "les dates en arbitrage apparaissent, en pointilles : {kinds:?}"
    );

    let tentative = cal
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "candidate_date")
        .unwrap();
    assert_eq!(tentative["status"], "tentative");
    assert!(!tentative["opportunity_id"].is_null());
}

#[tokio::test]
async fn le_filtre_mes_evenements_ne_montre_que_les_miens() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;

    // Un membre qui ne joue nulle part et ne tient aucun poste.
    let spectateur = app.make_user("Spectateur").await;
    app.join(cid, spectateur, "member").await;
    let client = app.login_as(spectateur).await;

    let cal = client
        .get(&format!(
            "/api/collectives/{cid}/calendar?mine=true&candidates=false"
        ))
        .await;
    assert!(cal.expect_ok().as_array().unwrap().is_empty());

    // Il prend un poste : l'evenement apparait.
    let antoine = app.login_named("Antoine").await;
    let (event_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT e.id FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.collective_id = $1 AND t.key = 'concert' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    let slot = antoine
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/logistics"),
            json!({ "label": "merch", "quantity": 1 }),
        )
        .await;
    let sid = slot.expect_ok()["id"].as_str().unwrap().to_string();
    client
        .post(
            &format!("/api/collectives/{cid}/events/{event_id}/logistics/{sid}/take"),
            json!({}),
        )
        .await
        .expect_ok();

    let cal = client
        .get(&format!(
            "/api/collectives/{cid}/calendar?mine=true&candidates=false"
        ))
        .await;
    assert_eq!(cal.expect_ok().as_array().unwrap().len(), 1);
}

//! §18 : « Une date est arretee avec un lieu **sans aucun message manuel** :
//! sondage ouvert → matrice → date retenue. »
//!
//! Et l'invariant qui donne sa valeur a la matrice : « Une disponibilite ne
//! peut etre ecrite que par la personne concernee, y compris par un admin. »

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

/// Monte une opportunite a trois dates, sondage ouvert.
async fn opportunite(app: &TestApp) -> (Uuid, Uuid, Vec<Uuid>) {
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
async fn du_sondage_a_l_evenement_sans_un_seul_message_manuel() {
    let app = TestApp::seeded().await;
    let (cid, oid, dates) = opportunite(&app).await;

    // 1. Ouvrir le sondage a prevenu tout le monde, tout seul.
    let (notifs,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE kind = 'poll_open'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(notifs, 5, "un message par membre du collectif");

    // 2. Chacun repond pour lui-meme, web ou bot, peu importe.
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

    // 3. La matrice designe la date : celle ou un groupe est **complet**.
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

    assert_eq!(d1["yes"], 4, "le 14 : tout le monde est dispo");
    let complets: Vec<String> = d1["complete_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap().to_string())
        .collect();
    assert!(
        complets.contains(&ramas.to_string()),
        "Ramas doit etre complet le 14"
    );

    // Le 7, Anas dit non : Ramas est incomplet, et l'app dit **qui** manque.
    let partiels = d0["partial_groups"].as_array().unwrap();
    let ramas_partiel = partiels
        .iter()
        .find(|p| p["group_id"] == json!(ramas.to_string()))
        .expect("Ramas doit apparaitre comme incomplet le 7");
    let anas_id = app.user_id("Anas").await;
    assert_eq!(ramas_partiel["missing"][0], json!(anas_id.to_string()));

    // Le 21, Mathieu manque : Dante3p est incomplet, Ramas non.
    let complets_21: Vec<String> = d2["complete_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap().to_string())
        .collect();
    assert!(complets_21.contains(&ramas.to_string()));

    // Les volontaires sont identifies : c'est le line-up possible.
    assert_eq!(d1["volunteers"].as_array().unwrap().len(), 3);

    // 4. Conversion : la date est arretee.
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

    // 5. Tout ce que la confirmation doit declencher (§5.3) est la.
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

    // Retenus **et** non-retenus sont prevenus.
    let (retenus,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE kind = 'lineup_retained'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    let (non_retenus,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE kind = 'lineup_not_retained'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(retenus, 2, "Anas et Romain");
    assert_eq!(non_retenus, 1, "Mathieu s'etait porte volontaire");

    // Le sondage se ferme, l'opportunite est confirmee.
    let opp = antoine
        .get(&format!("/api/collectives/{cid}/opportunities/{oid}"))
        .await;
    let opp = opp.expect_ok();
    assert_eq!(opp["status"], "confirmed");
    assert_eq!(opp["poll_open"], false);
}

#[tokio::test]
async fn une_disponibilite_n_est_ecrite_que_par_son_auteur_admin_compris() {
    let app = TestApp::seeded().await;
    let (cid, oid, dates) = opportunite(&app).await;

    let anas = app.user_id("Anas").await;
    let antoine = app.login_named("Antoine").await; // admin du collectif

    // L'admin tente de repondre a la place d'Anas.
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

    // Rien n'a ete ecrit sur CE sondage (les donnees d'amorcage en contiennent
    // d'autres, deja remplies par leurs auteurs).
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

    // Anas repond pour lui : accepte.
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
async fn les_disponibilites_sont_visibles_par_tous_les_membres() {
    let app = TestApp::seeded().await;
    let (cid, oid, dates) = opportunite(&app).await;

    let romain = app.login_named("Romain").await;
    romain
        .put(
            &format!("/api/collectives/{cid}/opportunities/{oid}/availabilities"),
            json!({ "answers": [ { "candidate_date_id": dates[0], "status": "yes" } ] }),
        )
        .await
        .expect_ok();

    // Mathieu, simple membre, voit la reponse de Romain : chacun voit que la
    // date est en train de se jouer (§5.2).
    let mathieu = app.login_named("Mathieu").await;
    let matrix = mathieu
        .get(&format!(
            "/api/collectives/{cid}/opportunities/{oid}/matrix"
        ))
        .await;
    let matrix = matrix.expect_ok();
    let romain_id = app.user_id("Romain").await;
    let ligne = matrix["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["user_id"] == json!(romain_id.to_string()))
        .unwrap();
    assert_eq!(ligne["answers"][dates[0].to_string()]["status"], "yes");
}

#[tokio::test]
async fn on_ne_peut_pas_ecrire_dans_le_sondage_d_une_autre_opportunite() {
    let app = TestApp::seeded().await;
    let (cid, oid, _) = opportunite(&app).await;

    // Date candidate appartenant a l'opportunite d'amorcage, pas a celle-ci.
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
async fn un_sondage_sans_date_candidate_ne_s_ouvre_pas() {
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

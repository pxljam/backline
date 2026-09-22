//! §18 : « Un membre d'un collectif ne peut acceder a aucune donnee d'un autre
//! collectif — verifie par des tests. »
//!
//! Le cloisonnement est structurel (§15) : il est porte par le type
//! `CollectiveScope`, pas par la discipline d'ecriture des requetes. Ces tests
//! attaquent la frontiere par toutes les portes ouvertes.

use crate::harness::TestApp;
use serde_json::json;

#[tokio::test]
async fn un_etranger_ne_voit_pas_le_collectif() {
    let app = TestApp::seeded().await;
    let bonsoir = app.collective_id("bonsoir-techno").await;

    let autre = app
        .make_collective("autre-collectif", "Autre collectif")
        .await;
    let intrus = app.make_user("Intrus").await;
    app.join(autre, intrus, "admin").await;
    let client = app.login_as(intrus).await;

    // 404 et non 403 : de l'exterieur, un collectif dont on n'est pas membre
    // n'existe pas.
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
async fn un_identifiant_vole_ne_donne_pas_acces_a_la_ressource() {
    let app = TestApp::seeded().await;
    let bonsoir = app.collective_id("bonsoir-techno").await;

    // L'evenement existe bel et bien ; seul le chemin change de collectif.
    let (event_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM events WHERE collective_id = $1 LIMIT 1")
            .bind(bonsoir)
            .fetch_one(&app.db)
            .await
            .unwrap();

    let autre = app.make_collective("voisins", "Voisins").await;
    let intrus = app.make_user("Voisin curieux").await;
    app.join(autre, intrus, "admin").await;
    let client = app.login_as(intrus).await;

    let res = client
        .get(&format!("/api/collectives/{autre}/events/{event_id}"))
        .await;
    res.expect_status(404);

    let res = client
        .get(&format!(
            "/api/collectives/{autre}/events/{event_id}/run-sheet"
        ))
        .await;
    res.expect_status(404);

    let res = client
        .post(
            &format!("/api/collectives/{autre}/events/{event_id}/logistics"),
            json!({ "label": "x" }),
        )
        .await;
    res.expect_status(404);
}

#[tokio::test]
async fn un_groupe_d_un_autre_collectif_ne_peut_pas_etre_rattache() {
    let app = TestApp::seeded().await;
    let ramas = app.group_id("ramas").await;

    let autre = app.make_collective("les-autres", "Les autres").await;
    let admin = app.make_user("Admin autre").await;
    app.join(autre, admin, "admin").await;
    let client = app.login_as(admin).await;

    // Creer une opportunite portee par un groupe qu'on ne possede pas.
    let res = client
        .post(
            &format!("/api/collectives/{autre}/opportunities"),
            json!({ "title": "Tentative", "host_group_ids": [ramas] }),
        )
        .await;
    res.expect_status(404);
}

#[tokio::test]
async fn un_membre_simple_ne_peut_pas_agir_en_admin() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    let client = app.login_as(anas).await;

    // Il lit tout — les dispos sont visibles par tous (§5.2)…
    client
        .get(&format!("/api/collectives/{cid}/opportunities"))
        .await
        .expect_ok();

    // …mais il n'arbitre pas.
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
async fn l_admin_d_instance_ne_se_confond_pas_avec_l_admin_de_collectif() {
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

    // L'admin d'instance, lui, passe.
    let root = app.user_id("Admin instance").await;
    let root_client = app.login_as(root).await;
    root_client
        .get("/api/instance/collectives")
        .await
        .expect_ok();
}

#[tokio::test]
async fn une_session_absente_ou_invalide_est_refusee() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;

    let (status, _) =
        crate::harness::anonymous_get(&app.base, &format!("/api/collectives/{cid}")).await;
    assert_eq!(status, 401);

    let (status, _) = crate::harness::anonymous_get(&app.base, "/api/me").await;
    assert_eq!(status, 401);
}

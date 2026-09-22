//! §18, mode assiste (§11.2) :
//! - « Aucune tache de publication ne peut etre marquee publiee sans action
//!   humaine explicite ; toute tache non confirmee alerte un admin. »
//! - « Une tache de publication ne peut etre assignee qu'a un membre ayant
//!   acces au compte vise. »

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

async fn premiere_tache(app: &TestApp, cid: Uuid) -> Uuid {
    sqlx::query_as::<_, (Uuid,)>(
        "SELECT p.id FROM publication_tasks p JOIN events e ON e.id = p.event_id
         WHERE e.collective_id = $1 ORDER BY p.scheduled_at LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap()
    .0
}

#[tokio::test]
async fn la_timeline_s_instancie_seule_et_tout_nait_en_brouillon() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let created = antoine
        .post(
            &format!("/api/collectives/{cid}/events"),
            json!({
                "event_type_key": "concert",
                "title": "Concert de mars",
                "starts_at": "2027-03-14T20:00:00Z"
            }),
        )
        .await;
    let eid = created.expect_ok()["id"].as_str().unwrap().to_string();

    let plan = antoine
        .get(&format!("/api/collectives/{cid}/events/{eid}/comms"))
        .await;
    let plan = plan.expect_ok();
    let jalons: Vec<String> = plan
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["milestone_key"].as_str().unwrap().to_string())
        .collect();
    // La timeline du §11.1, integralement.
    for attendu in ["j-30", "j-21", "j-14", "j-7", "j-3", "j-1", "j0", "j+2"] {
        assert!(
            jalons.contains(&attendu.to_string()),
            "{attendu} absent de {jalons:?}"
        );
    }
    assert!(
        plan.as_array()
            .unwrap()
            .iter()
            .all(|t| t["status"] == "draft"),
        "toute tache nait au statut brouillon : un humain valide toujours"
    );

    // J-30 tombe bien trente jours avant.
    let j30 = plan
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["milestone_key"] == "j-30")
        .unwrap();
    let at: chrono::DateTime<chrono::Utc> = j30["scheduled_at"].as_str().unwrap().parse().unwrap();
    let event: chrono::DateTime<chrono::Utc> = "2027-03-14T20:00:00Z".parse().unwrap();
    assert_eq!((event - at).num_days(), 30);
}

#[tokio::test]
async fn une_tache_ne_s_assigne_qu_a_quelqu_un_qui_a_acces_au_compte() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let task = premiere_tache(&app, cid).await;

    // Les donnees d'amorcage rattachent les taches au compte Instagram du
    // collectif, detenu par Antoine seul.
    let anas = app.user_id("Anas").await;
    let res = antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": anas }),
        )
        .await;
    res.expect_status(403);
    assert!(res.text.contains("acces au compte"), "{}", res.text);

    // Antoine, lui, a l'acces.
    let antoine_id = app.user_id("Antoine").await;
    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": antoine_id }),
        )
        .await
        .expect_ok();

    // On donne l'acces a Anas : il devient assignable.
    let (account,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM social_accounts WHERE collective_id = $1 AND group_id IS NULL LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();
    antoine
        .put(
            &format!("/api/collectives/{cid}/social-accounts/{account}/access"),
            json!({ "access": [antoine_id, anas] }),
        )
        .await
        .expect_ok();
    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": anas }),
        )
        .await
        .expect_ok();
}

#[tokio::test]
async fn retirer_l_acces_libere_les_taches_assignees() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let antoine_id = app.user_id("Antoine").await;
    let task = premiere_tache(&app, cid).await;

    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": antoine_id }),
        )
        .await
        .expect_ok();

    let (account,): (Uuid,) =
        sqlx::query_as("SELECT social_account_id FROM publication_tasks WHERE id = $1")
            .bind(task)
            .fetch_one(&app.db)
            .await
            .unwrap();

    // Une brouille, un depart : l'acces change. L'invariant doit tenir apres
    // coup, pas seulement au moment de l'assignation.
    antoine
        .put(
            &format!("/api/collectives/{cid}/social-accounts/{account}/access"),
            json!({ "access": [] }),
        )
        .await
        .expect_ok();

    let (assignee, status): (Option<Uuid>, String) =
        sqlx::query_as("SELECT assignee_id, status FROM publication_tasks WHERE id = $1")
            .bind(task)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(assignee.is_none(), "la tache redevient sans responsable");
    assert_eq!(status, "ready");
}

#[tokio::test]
async fn publie_ne_s_obtient_que_par_une_action_humaine() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let task = premiere_tache(&app, cid).await;

    // On ne peut pas forcer le statut par la porte de service.
    let res = antoine
        .patch(
            &format!("/api/collectives/{cid}/publication-tasks/{task}"),
            json!({ "status": "published" }),
        )
        .await;
    res.expect_status(400);

    // Quelqu'un a qui la tache n'est pas assignee ne confirme pas a sa place.
    let anas = app.login_named("Anas").await;
    let res = anas
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/published"),
            json!({}),
        )
        .await;
    res.expect_status(403);

    // Le responsable appuie sur « ✅ publie ».
    let antoine_id = app.user_id("Antoine").await;
    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": antoine_id }),
        )
        .await
        .expect_ok();
    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/published"),
            json!({ "published_url": "https://instagram.com/p/abc" }),
        )
        .await
        .expect_ok();

    let (status, url): (String, Option<String>) =
        sqlx::query_as("SELECT status, published_url FROM publication_tasks WHERE id = $1")
            .bind(task)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(status, "published");
    assert_eq!(url.as_deref(), Some("https://instagram.com/p/abc"));
}

#[tokio::test]
async fn sans_confirmation_le_responsable_est_relance_puis_l_admin_alerte() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let antoine_id = app.user_id("Antoine").await;
    let task = premiere_tache(&app, cid).await;

    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": antoine_id }),
        )
        .await
        .expect_ok();

    // L'heure de publication arrive.
    app.run_due_jobs().await;
    let (dues,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'publication_due'
           AND payload->>'task_id' = $1",
    )
    .bind(task.to_string())
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(dues >= 1, "le visuel et le texte partent au responsable");

    // Une heure plus tard, puis trois : relance, puis alerte admin.
    app.run_due_jobs().await;
    app.run_due_jobs().await;

    let (status, relances): (String, i32) =
        sqlx::query_as("SELECT status, reminder_count FROM publication_tasks WHERE id = $1")
            .bind(task)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(relances >= 1, "relance envoyee");
    assert_eq!(
        status, "missed",
        "sans confirmation, la tache est marquee ratee"
    );

    let (alertes,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'admin_alert'
           AND payload->>'task_id' = $1",
    )
    .bind(task.to_string())
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(alertes >= 1, "un admin doit etre prevenu");
}

#[tokio::test]
async fn confirmer_annule_les_relances_programmees() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let antoine_id = app.user_id("Antoine").await;
    let task = premiere_tache(&app, cid).await;

    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": antoine_id }),
        )
        .await
        .expect_ok();
    app.run_due_jobs().await;

    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/published"),
            json!({}),
        )
        .await
        .expect_ok();

    let (restants,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM jobs WHERE status = 'pending' AND dedupe_key LIKE $1")
            .bind(format!("task:{task}:%"))
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(restants, 0, "plus rien ne doit sonner");
}

#[tokio::test]
async fn deplacer_un_evenement_reprogramme_toute_sa_com() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;

    let created = antoine
        .post(
            &format!("/api/collectives/{cid}/events"),
            json!({
                "event_type_key": "concert",
                "title": "Concert deplace",
                "starts_at": "2027-06-01T20:00:00Z"
            }),
        )
        .await;
    let eid = created.expect_ok()["id"].as_str().unwrap().to_string();

    antoine
        .patch(
            &format!("/api/collectives/{cid}/events/{eid}"),
            json!({ "starts_at": "2027-07-01T20:00:00Z" }),
        )
        .await
        .expect_ok();

    let plan = antoine
        .get(&format!("/api/collectives/{cid}/events/{eid}/comms"))
        .await;
    let plan = plan.expect_ok();
    let j30 = plan
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["milestone_key"] == "j-30")
        .unwrap();
    let at: chrono::DateTime<chrono::Utc> = j30["scheduled_at"].as_str().unwrap().parse().unwrap();
    let nouveau: chrono::DateTime<chrono::Utc> = "2027-07-01T20:00:00Z".parse().unwrap();
    assert_eq!(
        (nouveau - at).num_days(),
        30,
        "la com suit la date, pas l'inverse"
    );

    // Les anciennes relances logistiques ne sonneront pas a l'ancienne date.
    let (anciennes,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM jobs
         WHERE status = 'pending' AND dedupe_key LIKE $1
           AND run_at < '2027-06-02T00:00:00Z'",
    )
    .bind(format!("event:{eid}:%"))
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(anciennes, 0);
}

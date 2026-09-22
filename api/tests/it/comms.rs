//! §18, assisted mode (§11.2):
//! - "No publication task can be marked published without an explicit human
//!   action; any unconfirmed task alerts an admin."
//! - "A publication task can only be assigned to a member with access to the
//!   account in question"

use crate::harness::TestApp;
use serde_json::json;
use uuid::Uuid;

async fn first_task(app: &TestApp, cid: Uuid) -> Uuid {
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
async fn the_timeline_instantiates_itself_and_everything_is_born_a_draft() {
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
    let milestones: Vec<String> = plan
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["milestone_key"].as_str().unwrap().to_string())
        .collect();
    // The §11.1 timeline, in full.
    for expected in ["j-30", "j-21", "j-14", "j-7", "j-3", "j-1", "j0", "j+2"] {
        assert!(
            milestones.contains(&expected.to_string()),
            "{expected} missing from {milestones:?}"
        );
    }
    assert!(
        plan.as_array()
            .unwrap()
            .iter()
            .all(|t| t["status"] == "draft"),
        "toute tache nait au statut brouillon : un humain valide toujours"
    );

    // D-30 really does fall thirty days before.
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
async fn a_task_is_only_assigned_to_someone_with_access_to_the_account() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let task = first_task(&app, cid).await;

    // The seed data attaches the tasks to the collective's Instagram account,
    // held by Antoine alone.
    let anas = app.user_id("Anas").await;
    let res = antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": anas }),
        )
        .await;
    res.expect_status(403);
    assert!(res.text.contains("acces au compte"), "{}", res.text);

    // Antoine does have the access.
    let antoine_id = app.user_id("Antoine").await;
    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": antoine_id }),
        )
        .await
        .expect_ok();

    // Give Anas the access: they become assignable.
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
async fn removing_access_frees_the_assigned_tasks() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let antoine_id = app.user_id("Antoine").await;
    let task = first_task(&app, cid).await;

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

    // A falling-out, a departure: access changes. The invariant must hold
    // afterwards, not only at assignment time.
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
    assert!(assignee.is_none(), "the task becomes unowned again");
    assert_eq!(status, "ready");
}

#[tokio::test]
async fn published_is_only_reached_by_a_human_action() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let task = first_task(&app, cid).await;

    // The status cannot be forced through the back door.
    let res = antoine
        .patch(
            &format!("/api/collectives/{cid}/publication-tasks/{task}"),
            json!({ "status": "published" }),
        )
        .await;
    res.expect_status(400);

    // Someone the task is not assigned to does not confirm in their place.
    let anas = app.login_named("Anas").await;
    let res = anas
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/published"),
            json!({}),
        )
        .await;
    res.expect_status(403);

    // The owner taps "✅ publie".
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
async fn without_confirmation_the_owner_is_reminded_then_the_admin_alerted() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let antoine_id = app.user_id("Antoine").await;
    let task = first_task(&app, cid).await;

    antoine
        .post(
            &format!("/api/collectives/{cid}/publication-tasks/{task}/assign"),
            json!({ "assignee_id": antoine_id }),
        )
        .await
        .expect_ok();

    // The publication time arrives.
    app.run_due_jobs().await;
    let (due,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'publication_due'
           AND payload->>'task_id' = $1",
    )
    .bind(task.to_string())
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(due >= 1, "the visual and the text go to the owner");

    // An hour later, then three: a reminder, then an admin alert.
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

    let (alerts,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notifications WHERE kind = 'admin_alert'
           AND payload->>'task_id' = $1",
    )
    .bind(task.to_string())
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(alerts >= 1, "an admin must be warned");
}

#[tokio::test]
async fn confirming_cancels_the_scheduled_reminders() {
    let app = TestApp::seeded().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let antoine = app.login_named("Antoine").await;
    let antoine_id = app.user_id("Antoine").await;
    let task = first_task(&app, cid).await;

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

    let (remaining,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM jobs WHERE status = 'pending' AND dedupe_key LIKE $1")
            .bind(format!("task:{task}:%"))
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(remaining, 0, "nothing more must fire");
}

#[tokio::test]
async fn moving_an_event_reschedules_all_of_its_comms() {
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
    let fresh: chrono::DateTime<chrono::Utc> = "2027-07-01T20:00:00Z".parse().unwrap();
    assert_eq!(
        (fresh - at).num_days(),
        30,
        "la com suit la date, pas l'inverse"
    );

    // The old logistics reminders will not fire on the old date.
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

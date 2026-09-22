//! §13: the bot is the **members' main channel**, and a complete mirror of the
//! web. What is checked here: two taps are enough, and the bot bypasses no
//! rule — not isolation, not "an availability is only written by the person it
//! belongs to", not "nothing is published without a human action".

use crate::harness::TestApp;
use async_trait::async_trait;
use backline::services::bot;
use backline::services::telegram::{OutgoingDocument, OutgoingMessage, Telegram, Update};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Test Telegram: keeps whatever would have gone out over the network.
#[derive(Default)]
struct FakeTelegram {
    messages: Mutex<Vec<OutgoingMessage>>,
    documents: Mutex<Vec<OutgoingDocument>>,
    reponses: Mutex<Vec<String>>,
}

#[async_trait]
impl Telegram for FakeTelegram {
    async fn send(&self, msg: OutgoingMessage) -> anyhow::Result<()> {
        self.messages.lock().unwrap().push(msg);
        Ok(())
    }
    async fn send_document(&self, doc: OutgoingDocument) -> anyhow::Result<()> {
        self.documents.lock().unwrap().push(doc);
        Ok(())
    }
    async fn poll_updates(&self, _offset: i64, _timeout: u64) -> anyhow::Result<Vec<Update>> {
        Ok(Vec::new())
    }
    async fn answer_callback(&self, _id: &str, texte: &str) -> anyhow::Result<()> {
        self.reponses.lock().unwrap().push(texte.to_string());
        Ok(())
    }
}

impl FakeTelegram {
    fn texts(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .map(|m| m.text.clone())
            .collect()
    }

    fn keyboards(&self) -> Vec<Value> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter_map(|m| m.keyboard.clone())
            .collect()
    }

    fn last_answer(&self) -> String {
        self.reponses
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

/// Plugs a fake Telegram into the application and returns both.
async fn app_with_bot() -> (TestApp, Arc<FakeTelegram>) {
    let mut app = TestApp::seeded().await;
    let fake = Arc::new(FakeTelegram::default());
    app.state.telegram = fake.clone();
    (app, fake)
}

fn message(chat_id: i64, texte: &str) -> Update {
    serde_json::from_value(json!({
        "update_id": 1,
        "message": {
            "chat": { "id": chat_id },
            "from": { "id": chat_id, "username": "membre" },
            "text": texte,
        }
    }))
    .unwrap()
}

fn button(chat_id: i64, data: &str) -> Update {
    serde_json::from_value(json!({
        "update_id": 2,
        "callback_query": {
            "id": "cb1",
            "from": { "id": chat_id, "username": "membre" },
            "data": data,
            "message": { "chat": { "id": chat_id } },
        }
    }))
    .unwrap()
}

/// Links an existing user to a Telegram identifier.
async fn link_account(app: &TestApp, user_id: Uuid, telegram_id: i64) {
    sqlx::query("UPDATE users SET telegram_id = $2 WHERE id = $1")
        .bind(user_id)
        .bind(telegram_id)
        .execute(&app.db)
        .await
        .unwrap();
}

async fn open_poll(app: &TestApp, cid: Uuid) -> (Uuid, Uuid) {
    let admin = app.login_named("Antoine").await;
    let opp = admin
        .post(
            &format!("/api/collectives/{cid}/opportunities"),
            json!({ "title": "Date a caler par le bot" }),
        )
        .await;
    let oid: Uuid = serde_json::from_value(opp.expect_ok()["id"].clone()).unwrap();

    let date = admin
        .post(
            &format!("/api/collectives/{cid}/opportunities/{oid}/dates"),
            json!({ "day": "2027-03-12", "start_time": "21:00" }),
        )
        .await;
    let date_id: Uuid = serde_json::from_value(date.expect_ok()["id"].clone()).unwrap();

    admin
        .post(
            &format!("/api/collectives/{cid}/opportunities/{oid}/poll"),
            json!({}),
        )
        .await
        .expect_ok();

    (oid, date_id)
}

#[tokio::test]
async fn start_links_the_account_and_the_invitation_serves_only_once() {
    let (app, fake) = app_with_bot().await;
    let user_id = app.user_id("Anas").await;

    sqlx::query(
        "INSERT INTO invitations (user_id, code, expires_at)
         VALUES ($1, 'CODE-BOT', now() + interval '7 days')",
    )
    .bind(user_id)
    .execute(&app.db)
    .await
    .unwrap();

    bot::handle_update(&app.state, message(4242, "/start CODE-BOT"))
        .await
        .unwrap();

    let (tg,): (Option<i64>,) = sqlx::query_as("SELECT telegram_id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(tg, Some(4242));
    assert!(fake.texts().last().unwrap().contains("Anas"));

    // Replaying the same code links nobody else.
    bot::handle_update(&app.state, message(9999, "/start CODE-BOT"))
        .await
        .unwrap();
    let (other,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE telegram_id = 9999")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(other, 0, "a spent invitation links nothing");
}

#[tokio::test]
async fn without_a_linked_account_the_bot_explains_how_to_link_one() {
    let (app, fake) = app_with_bot().await;

    bot::handle_update(&app.state, message(1, "/agenda"))
        .await
        .unwrap();

    assert!(fake.texts().last().unwrap().contains("/start"));
}

#[tokio::test]
async fn an_availability_takes_two_taps_and_binds_only_its_author() {
    let (app, fake) = app_with_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let (_oid, date_id) = open_poll(&app, cid).await;

    let anas = app.user_id("Anas").await;
    let romain = app.user_id("Romain").await;
    link_account(&app, anas, 111).await;
    link_account(&app, romain, 222).await;

    // First tap: the command lists the dates with their buttons.
    bot::handle_update(&app.state, message(111, "/dispos"))
        .await
        .unwrap();
    let keyboards = fake.keyboards();
    assert!(
        keyboards
            .iter()
            .any(|k| k.to_string().contains(&format!("dispo:{date_id}:yes"))),
        "chaque date porte ses trois reponses"
    );

    // Second tap: the answer is written, in the name of whoever tapped.
    bot::handle_update(&app.state, button(111, &format!("dispo:{date_id}:yes")))
        .await
        .unwrap();

    let lines: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT user_id, status, wants_to_play FROM availabilities WHERE candidate_date_id = $1",
    )
    .bind(date_id)
    .fetch_all(&app.db)
    .await
    .unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, anas);
    assert_eq!(lines[0].1, "yes");

    // The "je veux jouer" button toggles, without touching the other answer.
    bot::handle_update(&app.state, button(111, &format!("jouer:{date_id}")))
        .await
        .unwrap();
    bot::handle_update(&app.state, button(222, &format!("dispo:{date_id}:no")))
        .await
        .unwrap();

    let anas_line: (String, bool) = sqlx::query_as(
        "SELECT status, wants_to_play FROM availabilities
         WHERE candidate_date_id = $1 AND user_id = $2",
    )
    .bind(date_id)
    .bind(anas)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(anas_line, ("yes".into(), true));

    let romain_line: (String,) = sqlx::query_as(
        "SELECT status FROM availabilities WHERE candidate_date_id = $1 AND user_id = $2",
    )
    .bind(date_id)
    .bind(romain)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(romain_line.0, "no");
}

#[tokio::test]
async fn the_bot_does_not_cross_the_boundary_between_collectives() {
    let (app, fake) = app_with_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let (_oid, date_id) = open_poll(&app, cid).await;

    // A member of another collective, linked to the bot.
    let other_collective = app.make_collective("ailleurs", "Ailleurs").await;
    let intrus = app.make_user("Intrus").await;
    app.join(other_collective, intrus, "admin").await;
    link_account(&app, intrus, 777).await;

    bot::handle_update(&app.state, button(777, &format!("dispo:{date_id}:yes")))
        .await
        .unwrap();

    let (lines,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM availabilities WHERE candidate_date_id = $1 AND user_id = $2",
    )
    .bind(date_id)
    .bind(intrus)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(lines, 0, "no write outside their own collectives");
    assert_eq!(fake.last_answer(), "Sondage clos");
}

#[tokio::test]
async fn a_vacant_slot_can_be_taken_from_the_chat() {
    let (app, fake) = app_with_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    link_account(&app, anas, 111).await;

    let (slot_id, quantity): (Uuid, i32) = sqlx::query_as(
        "SELECT s.id, s.quantity FROM logistics_slots s JOIN events e ON e.id = s.event_id
         WHERE e.collective_id = $1 AND e.starts_at > now()
           AND (SELECT count(*) FROM logistics_assignments a WHERE a.slot_id = s.id) < s.quantity
         LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    bot::handle_update(&app.state, message(111, "/postes"))
        .await
        .unwrap();
    assert!(fake
        .keyboards()
        .iter()
        .any(|k| k.to_string().contains(&format!("poste:{slot_id}"))));

    bot::handle_update(&app.state, button(111, &format!("poste:{slot_id}")))
        .await
        .unwrap();

    let (pris,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM logistics_assignments WHERE slot_id = $1 AND user_id = $2",
    )
    .bind(slot_id)
    .bind(anas)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(pris, 1);

    // A full slot refuses, and says so.
    sqlx::query("UPDATE logistics_slots SET quantity = 1 WHERE id = $1")
        .bind(slot_id)
        .execute(&app.db)
        .await
        .unwrap();
    let romain = app.user_id("Romain").await;
    link_account(&app, romain, 222).await;
    bot::handle_update(&app.state, button(222, &format!("poste:{slot_id}")))
        .await
        .unwrap();
    assert_eq!(fake.last_answer(), "Ce poste est complet");
    let _ = quantity;
}

#[tokio::test]
async fn nothing_is_published_without_a_tap_from_the_assigned_person() {
    let (app, fake) = app_with_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    let romain = app.user_id("Romain").await;
    link_account(&app, anas, 111).await;
    link_account(&app, romain, 222).await;

    let (task_id,): (Uuid,) = sqlx::query_as(
        "SELECT p.id FROM publication_tasks p JOIN events e ON e.id = p.event_id
         WHERE e.collective_id = $1 ORDER BY p.scheduled_at LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&app.db)
    .await
    .unwrap();

    sqlx::query("UPDATE publication_tasks SET assignee_id = $2, status = 'assigned' WHERE id = $1")
        .bind(task_id)
        .bind(anas)
        .execute(&app.db)
        .await
        .unwrap();

    // Someone else cannot confirm in their place.
    bot::handle_update(&app.state, button(222, &format!("publie:{task_id}")))
        .await
        .unwrap();
    let (statut,): (String,) = sqlx::query_as("SELECT status FROM publication_tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(statut, "assigned");
    assert_eq!(fake.last_answer(), "Cette tache ne t'est pas assignee");

    // The assigned person, though, confirms in one tap.
    bot::handle_update(&app.state, button(111, &format!("publie:{task_id}")))
        .await
        .unwrap();
    let (statut, publie): (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT status, published_at FROM publication_tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(statut, "published");
    assert!(publie.is_some());

    // The scheduled reminders are cancelled.
    let (relances,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM jobs WHERE dedupe_key LIKE $1 AND status = 'pending'")
            .bind(format!("task:{task_id}:%"))
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(relances, 0);
}

#[tokio::test]
async fn an_actionable_notification_goes_out_with_its_button() {
    let (app, fake) = app_with_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    link_account(&app, anas, 111).await;

    let task_id = Uuid::new_v4();
    backline::services::notify::push(
        &app.db,
        backline::services::notify::Notice {
            user_id: anas,
            collective_id: Some(cid),
            kind: "publication_due",
            title: "A publier : annonce".into(),
            body: "legende".into(),
            payload: json!({ "task_id": task_id }),
        },
    )
    .await
    .unwrap();

    let sent = backline::services::notify::deliver_pending(&app.state, 10)
        .await
        .unwrap();
    assert!(sent >= 1);

    let keyboard = fake
        .keyboards()
        .into_iter()
        .find(|k| k.to_string().contains("publie:"))
        .expect("the task arrives with its \"publie\" button");
    assert_eq!(keyboard[0][0]["callback_data"], format!("publie:{task_id}"));
}

#[tokio::test]
async fn the_collectif_command_switches_the_chat_and_the_agenda_follows() {
    let (app, fake) = app_with_bot().await;
    let anas = app.user_id("Anas").await;
    link_account(&app, anas, 111).await;

    let ailleurs = app.make_collective("ailleurs", "Ailleurs").await;
    app.join(ailleurs, anas, "member").await;

    bot::handle_update(&app.state, message(111, "/collectif"))
        .await
        .unwrap();
    let keyboard = fake
        .keyboards()
        .into_iter()
        .find(|k| k.to_string().contains("collectif:"))
        .expect("the list of collectives");
    assert!(keyboard.to_string().contains(&ailleurs.to_string()));

    bot::handle_update(&app.state, button(111, &format!("collectif:{ailleurs}")))
        .await
        .unwrap();
    assert_eq!(fake.last_answer(), "Collectif courant change");

    bot::handle_update(&app.state, message(111, "/agenda"))
        .await
        .unwrap();
    assert!(
        fake.texts().last().unwrap().contains("Ailleurs"),
        "l'agenda suit le collectif courant"
    );
}

#[tokio::test]
async fn the_tech_rider_goes_out_as_a_pdf_from_the_chat() {
    let (app, fake) = app_with_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let gid = app.group_id("ramas").await;
    let member: (Uuid,) =
        sqlx::query_as("SELECT user_id FROM group_members WHERE group_id = $1 LIMIT 1")
            .bind(gid)
            .fetch_one(&app.db)
            .await
            .unwrap();
    link_account(&app, member.0, 111).await;

    let admin = app.login_named("Antoine").await;
    let rider = admin
        .post(
            &format!("/api/collectives/{cid}/groups/{gid}/tech-riders"),
            json!({ "data": { "stage": "2 retours", "contact": "Ramas" } }),
        )
        .await;
    let rider_id: Uuid = serde_json::from_value(rider.expect_ok()["id"].clone()).unwrap();
    admin
        .post(
            &format!("/api/collectives/{cid}/groups/{gid}/tech-riders/{rider_id}/publish"),
            json!({}),
        )
        .await
        .expect_ok();

    bot::handle_update(&app.state, message(111, "/fiche"))
        .await
        .unwrap();
    assert!(fake
        .keyboards()
        .iter()
        .any(|k| k.to_string().contains(&format!("fiche:{gid}"))));

    bot::handle_update(&app.state, button(111, &format!("fiche:{gid}")))
        .await
        .unwrap();

    let documents = fake.documents.lock().unwrap();
    assert_eq!(documents.len(), 1, "a PDF goes out in the chat");
    assert!(documents[0].filename.ends_with(".pdf"));
}

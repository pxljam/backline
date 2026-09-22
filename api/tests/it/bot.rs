//! §13 : le bot est le **canal principal des membres**, et un doublon complet
//! du web. Ce qu'on verifie ici : deux appuis suffisent, et le bot ne
//! contourne aucune regle — ni le cloisonnement, ni « une dispo n'est ecrite
//! que par la personne concernee », ni « rien n'est publie sans action
//! humaine ».

use crate::harness::TestApp;
use async_trait::async_trait;
use backline::services::bot;
use backline::services::telegram::{OutgoingDocument, OutgoingMessage, Telegram, Update};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Telegram de test : on garde ce qui serait parti sur le reseau.
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
    fn textes(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .map(|m| m.text.clone())
            .collect()
    }

    fn claviers(&self) -> Vec<Value> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter_map(|m| m.keyboard.clone())
            .collect()
    }

    fn derniere_reponse(&self) -> String {
        self.reponses
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

/// Branche un faux Telegram sur l'application et renvoie les deux.
async fn avec_bot() -> (TestApp, Arc<FakeTelegram>) {
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

fn bouton(chat_id: i64, data: &str) -> Update {
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

/// Lie un utilisateur existant a un identifiant Telegram.
async fn lier(app: &TestApp, user_id: Uuid, telegram_id: i64) {
    sqlx::query("UPDATE users SET telegram_id = $2 WHERE id = $1")
        .bind(user_id)
        .bind(telegram_id)
        .execute(&app.db)
        .await
        .unwrap();
}

async fn ouvrir_sondage(app: &TestApp, cid: Uuid) -> (Uuid, Uuid) {
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
async fn start_lie_le_compte_et_l_invitation_ne_sert_qu_une_fois() {
    let (app, fake) = avec_bot().await;
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
    assert!(fake.textes().last().unwrap().contains("Anas"));

    // Rejouer le meme code ne lie personne d'autre.
    bot::handle_update(&app.state, message(9999, "/start CODE-BOT"))
        .await
        .unwrap();
    let (autre,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE telegram_id = 9999")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(autre, 0, "une invitation perimee ne lie rien");
}

#[tokio::test]
async fn sans_compte_lie_le_bot_explique_comment_se_lier() {
    let (app, fake) = avec_bot().await;

    bot::handle_update(&app.state, message(1, "/agenda"))
        .await
        .unwrap();

    assert!(fake.textes().last().unwrap().contains("/start"));
}

#[tokio::test]
async fn une_dispo_se_donne_en_deux_appuis_et_n_engage_que_son_auteur() {
    let (app, fake) = avec_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let (_oid, date_id) = ouvrir_sondage(&app, cid).await;

    let anas = app.user_id("Anas").await;
    let romain = app.user_id("Romain").await;
    lier(&app, anas, 111).await;
    lier(&app, romain, 222).await;

    // Premier appui : la commande liste les dates avec leurs boutons.
    bot::handle_update(&app.state, message(111, "/dispos"))
        .await
        .unwrap();
    let claviers = fake.claviers();
    assert!(
        claviers
            .iter()
            .any(|k| k.to_string().contains(&format!("dispo:{date_id}:yes"))),
        "chaque date porte ses trois reponses"
    );

    // Second appui : la reponse est ecrite, au nom de l'auteur du clic.
    bot::handle_update(&app.state, bouton(111, &format!("dispo:{date_id}:yes")))
        .await
        .unwrap();

    let lignes: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT user_id, status, wants_to_play FROM availabilities WHERE candidate_date_id = $1",
    )
    .bind(date_id)
    .fetch_all(&app.db)
    .await
    .unwrap();
    assert_eq!(lignes.len(), 1);
    assert_eq!(lignes[0].0, anas);
    assert_eq!(lignes[0].1, "yes");

    // Le bouton « je veux jouer » bascule, sans toucher a la reponse de l'autre.
    bot::handle_update(&app.state, bouton(111, &format!("jouer:{date_id}")))
        .await
        .unwrap();
    bot::handle_update(&app.state, bouton(222, &format!("dispo:{date_id}:no")))
        .await
        .unwrap();

    let anas_ligne: (String, bool) = sqlx::query_as(
        "SELECT status, wants_to_play FROM availabilities
         WHERE candidate_date_id = $1 AND user_id = $2",
    )
    .bind(date_id)
    .bind(anas)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(anas_ligne, ("yes".into(), true));

    let romain_ligne: (String,) = sqlx::query_as(
        "SELECT status FROM availabilities WHERE candidate_date_id = $1 AND user_id = $2",
    )
    .bind(date_id)
    .bind(romain)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(romain_ligne.0, "no");
}

#[tokio::test]
async fn le_bot_ne_franchit_pas_la_frontiere_entre_collectifs() {
    let (app, fake) = avec_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let (_oid, date_id) = ouvrir_sondage(&app, cid).await;

    // Un membre d'un autre collectif, lie au bot.
    let autre_collectif = app.make_collective("ailleurs", "Ailleurs").await;
    let intrus = app.make_user("Intrus").await;
    app.join(autre_collectif, intrus, "admin").await;
    lier(&app, intrus, 777).await;

    bot::handle_update(&app.state, bouton(777, &format!("dispo:{date_id}:yes")))
        .await
        .unwrap();

    let (lignes,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM availabilities WHERE candidate_date_id = $1 AND user_id = $2",
    )
    .bind(date_id)
    .bind(intrus)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(lignes, 0, "aucune ecriture hors de ses collectifs");
    assert_eq!(fake.derniere_reponse(), "Sondage clos");
}

#[tokio::test]
async fn un_poste_vacant_se_prend_depuis_la_conversation() {
    let (app, fake) = avec_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    lier(&app, anas, 111).await;

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
        .claviers()
        .iter()
        .any(|k| k.to_string().contains(&format!("poste:{slot_id}"))));

    bot::handle_update(&app.state, bouton(111, &format!("poste:{slot_id}")))
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

    // Un poste complet se refuse en le disant.
    sqlx::query("UPDATE logistics_slots SET quantity = 1 WHERE id = $1")
        .bind(slot_id)
        .execute(&app.db)
        .await
        .unwrap();
    let romain = app.user_id("Romain").await;
    lier(&app, romain, 222).await;
    bot::handle_update(&app.state, bouton(222, &format!("poste:{slot_id}")))
        .await
        .unwrap();
    assert_eq!(fake.derniere_reponse(), "Ce poste est complet");
    let _ = quantity;
}

#[tokio::test]
async fn rien_n_est_publie_sans_appui_de_la_personne_assignee() {
    let (app, fake) = avec_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    let romain = app.user_id("Romain").await;
    lier(&app, anas, 111).await;
    lier(&app, romain, 222).await;

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

    // Quelqu'un d'autre ne peut pas confirmer a sa place.
    bot::handle_update(&app.state, bouton(222, &format!("publie:{task_id}")))
        .await
        .unwrap();
    let (statut,): (String,) = sqlx::query_as("SELECT status FROM publication_tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(statut, "assigned");
    assert_eq!(fake.derniere_reponse(), "Cette tache ne t'est pas assignee");

    // La personne assignee, elle, confirme en un appui.
    bot::handle_update(&app.state, bouton(111, &format!("publie:{task_id}")))
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

    // Les relances programmees sont annulees.
    let (relances,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM jobs WHERE dedupe_key LIKE $1 AND status = 'pending'")
            .bind(format!("task:{task_id}:%"))
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(relances, 0);
}

#[tokio::test]
async fn une_notification_a_action_part_avec_son_bouton() {
    let (app, fake) = avec_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let anas = app.user_id("Anas").await;
    lier(&app, anas, 111).await;

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

    let envoyees = backline::services::notify::deliver_pending(&app.state, 10)
        .await
        .unwrap();
    assert!(envoyees >= 1);

    let clavier = fake
        .claviers()
        .into_iter()
        .find(|k| k.to_string().contains("publie:"))
        .expect("la tache arrive avec son bouton « publie »");
    assert_eq!(clavier[0][0]["callback_data"], format!("publie:{task_id}"));
}

#[tokio::test]
async fn collectif_bascule_la_conversation_et_l_agenda_suit() {
    let (app, fake) = avec_bot().await;
    let anas = app.user_id("Anas").await;
    lier(&app, anas, 111).await;

    let ailleurs = app.make_collective("ailleurs", "Ailleurs").await;
    app.join(ailleurs, anas, "member").await;

    bot::handle_update(&app.state, message(111, "/collectif"))
        .await
        .unwrap();
    let clavier = fake
        .claviers()
        .into_iter()
        .find(|k| k.to_string().contains("collectif:"))
        .expect("la liste des collectifs");
    assert!(clavier.to_string().contains(&ailleurs.to_string()));

    bot::handle_update(&app.state, bouton(111, &format!("collectif:{ailleurs}")))
        .await
        .unwrap();
    assert_eq!(fake.derniere_reponse(), "Collectif courant change");

    bot::handle_update(&app.state, message(111, "/agenda"))
        .await
        .unwrap();
    assert!(
        fake.textes().last().unwrap().contains("Ailleurs"),
        "l'agenda suit le collectif courant"
    );
}

#[tokio::test]
async fn la_fiche_technique_part_en_pdf_depuis_la_conversation() {
    let (app, fake) = avec_bot().await;
    let cid = app.collective_id("bonsoir-techno").await;
    let gid = app.group_id("ramas").await;
    let membre: (Uuid,) =
        sqlx::query_as("SELECT user_id FROM group_members WHERE group_id = $1 LIMIT 1")
            .bind(gid)
            .fetch_one(&app.db)
            .await
            .unwrap();
    lier(&app, membre.0, 111).await;

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
        .claviers()
        .iter()
        .any(|k| k.to_string().contains(&format!("fiche:{gid}"))));

    bot::handle_update(&app.state, bouton(111, &format!("fiche:{gid}")))
        .await
        .unwrap();

    let documents = fake.documents.lock().unwrap();
    assert_eq!(documents.len(), 1, "un PDF part dans la conversation");
    assert!(documents[0].filename.ends_with(".pdf"));
}

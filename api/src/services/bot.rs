//! Telegram bot (§13): the **members' main channel**, a complete mirror of the
//! web. Every decision takes two taps — a command, a button.
//!
//! The bot bypasses no domain rule: an availability is only written by the
//! person it belongs to, a publication task is only confirmed by its owner, and
//! membership of the collective is checked on every action.

use crate::error::AppResult;
use crate::services::pdf;
use crate::services::telegram::{
    CallbackQuery, Message, OutgoingDocument, OutgoingMessage, Update,
};
use crate::state::AppState;
use chrono::Utc;
use chrono_tz::Europe::Paris;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const HELP: &str = "Commandes : /dispos · /agenda · /postes · /publier · /fiche · /collectif";

/// Long polling loop. A single process holds it (the `bot` service).
pub async fn run_forever(state: AppState) {
    if !state.telegram.enabled() {
        // Without a token there is nothing to read. The service stays alive
        // rather than exiting in a loop: the web is a complete mirror of the
        // bot (§19).
        tracing::warn!("bot: no Telegram token — service idle");
        futures::future::pending::<()>().await;
        return;
    }

    let mut offset = 0i64;
    tracing::info!("bot: listening");

    loop {
        match state.telegram.poll_updates(offset, 30).await {
            Ok(updates) => {
                for update in updates {
                    offset = offset.max(update.update_id + 1);
                    if let Err(e) = handle_update(&state, update).await {
                        tracing::warn!(error = %e, "bot: update not handled");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "bot: reading updates");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

pub async fn handle_update(state: &AppState, update: Update) -> AppResult<()> {
    if let Some(cb) = update.callback_query {
        return handle_callback(state, cb).await;
    }
    if let Some(msg) = update.message {
        return handle_message(state, msg).await;
    }
    Ok(())
}

// --- Parsing -----------------------------------------------------------------

/// `/dispos@backline_bot 2` -> `("dispos", "2")`. Returns `None` if the message
/// is not a command.
pub fn parse_command(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    let rest = text.strip_prefix('/')?;
    let (head, argument) = match rest.split_once(char::is_whitespace) {
        Some((t, a)) => (t, a.trim()),
        None => (rest, ""),
    };
    let command = head.split('@').next().unwrap_or(head).to_lowercase();
    if command.is_empty() {
        return None;
    }
    Some((command, argument.to_string()))
}

/// A button's data: `action:argument[:value]`, 64 bytes at most.
pub fn parse_callback(data: &str) -> (String, Vec<String>) {
    let mut parts = data.split(':');
    let action = parts.next().unwrap_or_default().to_string();
    (action, parts.map(|s| s.to_string()).collect())
}

/// Keyboard attached to a notification delivered through Telegram: this is
/// what makes the bot a channel for action rather than a log (§13).
pub fn keyboard_for(kind: &str, payload: &Value) -> Option<Value> {
    let id = |key: &str| payload.get(key).and_then(Value::as_str).map(str::to_string);

    match kind {
        "poll_open" => id("opportunity_id")
            .map(|oid| json!([[{ "text": "Repondre au sondage", "callback_data": format!("dispos:{oid}") }]])),
        "publication_due" | "publication_assigned" => id("task_id")
            .map(|tid| json!([[{ "text": "✅ publie", "callback_data": format!("publie:{tid}") }]])),
        "logistics_vacant" => id("event_id")
            .map(|eid| json!([[{ "text": "Voir les postes", "callback_data": format!("postes:{eid}") }]])),
        "lineup_retained" => id("event_id")
            .map(|eid| json!([[{ "text": "✅ bien recu", "callback_data": format!("ack:{eid}") }]])),
        _ => None,
    }
}

// --- Messages ----------------------------------------------------------------

async fn handle_message(state: &AppState, msg: Message) -> AppResult<()> {
    let chat_id = msg.chat.id;
    let text = msg.text.clone().unwrap_or_default();
    let Some((command, argument)) = parse_command(&text) else {
        return reply(
            state,
            chat_id,
            &format!("Je ne comprends que des commandes.\n{HELP}"),
            None,
        )
        .await;
    };

    // `/start <code>` is the only open command: it is what links the
    // account.
    if command == "start" {
        return link_account(state, &msg, &argument).await;
    }

    let Some(user_id) = user_for_telegram_id(&state.db, msg.from.as_ref().map(|u| u.id)).await?
    else {
        return reply(
            state,
            chat_id,
            "Ce compte Telegram n'est lie a aucun membre. Envoie <code>/start &lt;code&gt;</code> \
             avec le code de ton invitation.",
            None,
        )
        .await;
    };

    match command.as_str() {
        "dispos" => availabilities(state, chat_id, user_id).await,
        "agenda" => agenda(state, chat_id, user_id).await,
        "postes" => slots(state, chat_id, user_id).await,
        "publier" => publish(state, chat_id, user_id).await,
        "fiche" => riders(state, chat_id, user_id).await,
        "collectif" => collectives(state, chat_id, user_id).await,
        "aide" | "help" => reply(state, chat_id, HELP, None).await,
        _ => reply(state, chat_id, &format!("Commande inconnue.\n{HELP}"), None).await,
    }
}

async fn link_account(state: &AppState, msg: &Message, code: &str) -> AppResult<()> {
    let chat_id = msg.chat.id;
    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };

    if code.is_empty() {
        let existing = user_for_telegram_id(&state.db, Some(from.id)).await?;
        let text = if existing.is_some() {
            format!("Ton compte est deja lie.\n{HELP}")
        } else {
            "Envoie <code>/start &lt;code&gt;</code> avec le code de ton invitation.".into()
        };
        return reply(state, chat_id, &text, None).await;
    }

    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT i.id, i.user_id, u.display_name FROM invitations i
         JOIN users u ON u.id = i.user_id
         WHERE i.code = $1 AND i.used_at IS NULL AND i.expires_at > now()
         FOR UPDATE OF i",
    )
    .bind(code)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((invitation_id, user_id, name)) = row else {
        tx.rollback().await?;
        return reply(state, chat_id, "Invitation inconnue ou perimee.", None).await;
    };

    sqlx::query(
        "UPDATE users SET telegram_id = $2, telegram_username = $3, updated_at = now()
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(from.id)
    .bind(&from.username)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE invitations SET used_at = now() WHERE id = $1")
        .bind(invitation_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    reply(
        state,
        chat_id,
        &format!("Compte lie : <b>{name}</b>.\n{HELP}"),
        None,
    )
    .await
}

// --- Commands -----------------------------------------------------------------

async fn availabilities(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, _)) = current_collective(&state.db, user_id).await? else {
        return reply(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
    };

    let polls: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT o.id, o.title FROM opportunities o
         JOIN availability_polls p ON p.opportunity_id = o.id
         WHERE o.collective_id = $1 AND p.closed_at IS NULL
         ORDER BY p.opened_at",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    if polls.is_empty() {
        return reply(state, chat_id, "Aucun sondage ouvert.", None).await;
    }

    for (oid, title) in polls {
        send_poll(state, chat_id, user_id, oid, &title).await?;
    }
    Ok(())
}

/// One message per date: three answer buttons and one for "je veux jouer".
/// Two taps are enough — that is the channel's rule (§13).
async fn send_poll(
    state: &AppState,
    chat_id: i64,
    user_id: Uuid,
    oid: Uuid,
    title: &str,
) -> AppResult<()> {
    let dates: Vec<(Uuid, chrono::NaiveDate, Option<chrono::NaiveTime>)> = sqlx::query_as(
        "SELECT id, day, start_time FROM candidate_dates
         WHERE opportunity_id = $1 ORDER BY position, day",
    )
    .bind(oid)
    .fetch_all(&state.db)
    .await?;

    reply(state, chat_id, &format!("<b>{title}</b>"), None).await?;

    for (date_id, jour, heure) in dates {
        let answer: Option<(String, bool)> = sqlx::query_as(
            "SELECT status, wants_to_play FROM availabilities
             WHERE candidate_date_id = $1 AND user_id = $2",
        )
        .bind(date_id)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;

        let mark = |status: &str, icon: &str| -> String {
            match &answer {
                Some((s, _)) if s == status => format!("• {icon}"),
                _ => icon.to_string(),
            }
        };
        let play_mark = match &answer {
            Some((_, true)) => "• 🎸 je joue",
            _ => "🎸 je veux jouer",
        };

        let when = match heure {
            Some(h) => format!("{} a {}", jour.format("%d/%m/%Y"), h.format("%H:%M")),
            None => jour.format("%d/%m/%Y").to_string(),
        };

        let keyboard = json!([
            [
                { "text": mark("yes", "✅"), "callback_data": format!("dispo:{date_id}:yes") },
                { "text": mark("maybe", "❔"), "callback_data": format!("dispo:{date_id}:maybe") },
                { "text": mark("no", "❌"), "callback_data": format!("dispo:{date_id}:no") },
            ],
            [{ "text": play_mark, "callback_data": format!("jouer:{date_id}") }],
        ]);

        reply(state, chat_id, &when, Some(keyboard)).await?;
    }
    Ok(())
}

async fn agenda(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, name)) = current_collective(&state.db, user_id).await? else {
        return reply(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
    };

    let events: Vec<(String, chrono::DateTime<Utc>, Option<String>, String)> = sqlx::query_as(
        "SELECT e.title, e.starts_at, v.name, e.status FROM events e
         LEFT JOIN venues v ON v.id = e.venue_id
         WHERE e.collective_id = $1 AND e.starts_at > now() AND e.status <> 'cancelled'
         ORDER BY e.starts_at LIMIT 10",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    if events.is_empty() {
        return reply(state, chat_id, &format!("{name} : rien de prevu."), None).await;
    }

    let lines: Vec<String> = events
        .into_iter()
        .map(|(title, when, venue, status)| {
            let local = when.with_timezone(&Paris);
            let venue = venue.unwrap_or_else(|| "lieu a caler".into());
            let mention = if status == "draft" {
                " (brouillon)"
            } else {
                ""
            };
            format!(
                "• {} — <b>{title}</b>{mention}\n  {venue}",
                local.format("%d/%m %H:%M")
            )
        })
        .collect();

    reply(
        state,
        chat_id,
        &format!("<b>{name}</b>\n{}", lines.join("\n")),
        None,
    )
    .await
}

async fn slots(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, _)) = current_collective(&state.db, user_id).await? else {
        return reply(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
    };

    let slots: Vec<(Uuid, String, i32, i64, String, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT s.id, s.label, s.quantity,
                (SELECT count(*) FROM logistics_assignments a WHERE a.slot_id = s.id),
                e.title, e.starts_at
         FROM logistics_slots s
         JOIN events e ON e.id = s.event_id
         WHERE e.collective_id = $1 AND e.starts_at > now() AND e.status <> 'cancelled'
         ORDER BY e.starts_at, s.position",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let vacant: Vec<_> = slots
        .into_iter()
        .filter(|(_, _, quantity, pris, _, _)| *pris < *quantity as i64)
        .collect();

    if vacant.is_empty() {
        return reply(state, chat_id, "Aucun poste vacant. 🎉", None).await;
    }

    for (slot_id, label, quantity, pris, title, when) in vacant {
        let local = when.with_timezone(&Paris);
        let text = format!(
            "<b>{label}</b> — {title}\n{} · encore {} place(s)",
            local.format("%d/%m %H:%M"),
            quantity as i64 - pris
        );
        let keyboard =
            json!([[{ "text": "Je le prends", "callback_data": format!("poste:{slot_id}") }]]);
        reply(state, chat_id, &text, Some(keyboard)).await?;
    }
    Ok(())
}

async fn publish(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let tasks: Vec<(Uuid, String, String, String, chrono::DateTime<Utc>, String)> = sqlx::query_as(
        "SELECT p.id, p.label, p.caption, p.hashtags, p.scheduled_at, e.title
             FROM publication_tasks p JOIN events e ON e.id = p.event_id
             WHERE (p.assignee_id = $1 OR p.backup_assignee_id = $1)
               AND p.status <> 'published'
             ORDER BY p.scheduled_at LIMIT 10",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    if tasks.is_empty() {
        return reply(state, chat_id, "Aucune publication a ta charge.", None).await;
    }

    for (tid, label, caption, hashtags, when, evenement) in tasks {
        let local = when.with_timezone(&Paris);
        let text = format!(
            "<b>{label}</b> — {evenement}\n{}\n\n{caption}\n{hashtags}",
            local.format("%d/%m %H:%M")
        );
        let keyboard = json!([[{ "text": "✅ publie", "callback_data": format!("publie:{tid}") }]]);
        // The visual goes out with the text: everything is ready to paste (§11.2).
        let photo = task_visual(state, tid).await;
        send(state, chat_id, &text, Some(keyboard), photo).await?;
    }
    Ok(())
}

async fn riders(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, _)) = current_collective(&state.db, user_id).await? else {
        return reply(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
    };

    let groups: Vec<(Uuid, String, Option<i32>)> = sqlx::query_as(
        "SELECT g.id, g.name,
                (SELECT max(r.version) FROM tech_riders r
                  WHERE r.group_id = g.id AND r.status = 'published')
         FROM groups g
         JOIN group_members gm ON gm.group_id = g.id
         WHERE g.collective_id = $1 AND gm.user_id = $2
         ORDER BY g.name",
    )
    .bind(cid)
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    if groups.is_empty() {
        return reply(
            state,
            chat_id,
            "Tu n'es dans aucun groupe de ce collectif.",
            None,
        )
        .await;
    }

    for (gid, name, version) in groups {
        match version {
            Some(v) => {
                let keyboard = json!([[{ "text": format!("Envoyer la v{v}"), "callback_data": format!("fiche:{gid}") }]]);
                reply(
                    state,
                    chat_id,
                    &format!("<b>{name}</b> — fiche v{v}"),
                    Some(keyboard),
                )
                .await?;
            }
            // A missing rider is reported, never blocking (§12).
            None => {
                reply(
                    state,
                    chat_id,
                    &format!("<b>{name}</b> — aucune fiche publiee."),
                    None,
                )
                .await?
            }
        }
    }
    Ok(())
}

async fn collectives(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT c.id, c.name FROM memberships m JOIN collectives c ON c.id = m.collective_id
         WHERE m.user_id = $1 ORDER BY c.name",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    if rows.len() <= 1 {
        let name = rows.first().map(|(_, n)| n.clone()).unwrap_or_default();
        return reply(
            state,
            chat_id,
            &format!("Tu n'es que dans <b>{name}</b> — rien a basculer."),
            None,
        )
        .await;
    }

    let current = current_collective(&state.db, user_id)
        .await?
        .map(|(id, _)| id);
    let buttons: Vec<Value> = rows
        .iter()
        .map(|(id, name)| {
            let mark = if Some(*id) == current { "• " } else { "" };
            json!([{ "text": format!("{mark}{name}"), "callback_data": format!("collectif:{id}") }])
        })
        .collect();

    reply(state, chat_id, "Sur quel collectif ?", Some(json!(buttons))).await
}

// --- Buttons -------------------------------------------------------------------

async fn handle_callback(state: &AppState, cb: CallbackQuery) -> AppResult<()> {
    let chat_id = cb.message.as_ref().map(|m| m.chat.id).unwrap_or(cb.from.id);
    let data = cb.data.clone().unwrap_or_default();
    let (action, args) = parse_callback(&data);

    let Some(user_id) = user_for_telegram_id(&state.db, Some(cb.from.id)).await? else {
        return acknowledge_callback(state, &cb.id, "Compte non lie").await;
    };

    let uuid = |i: usize| -> Option<Uuid> { args.get(i).and_then(|s| Uuid::parse_str(s).ok()) };

    match action.as_str() {
        "dispo" => {
            let (Some(date_id), Some(status)) = (uuid(0), args.get(1)) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            match answer_availability(state, user_id, date_id, status).await? {
                true => acknowledge_callback(state, &cb.id, "Reponse enregistree").await,
                false => acknowledge_callback(state, &cb.id, "Sondage clos").await,
            }
        }
        "jouer" => {
            let Some(date_id) = uuid(0) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            let wants = toggle_wants_to_play(state, user_id, date_id).await?;
            let text = if wants {
                "Note : tu veux jouer"
            } else {
                "Retire"
            };
            acknowledge_callback(state, &cb.id, text).await
        }
        "dispos" => {
            let Some(oid) = uuid(0) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            let title: Option<(String,)> = sqlx::query_as(
                "SELECT o.title FROM opportunities o
                 JOIN memberships m ON m.collective_id = o.collective_id AND m.user_id = $2
                 WHERE o.id = $1",
            )
            .bind(oid)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
            match title {
                Some((title,)) => {
                    acknowledge_callback(state, &cb.id, "").await?;
                    send_poll(state, chat_id, user_id, oid, &title).await
                }
                None => acknowledge_callback(state, &cb.id, "Hors de tes collectifs").await,
            }
        }
        "postes" => {
            acknowledge_callback(state, &cb.id, "").await?;
            slots(state, chat_id, user_id).await
        }
        "poste" => {
            let Some(slot_id) = uuid(0) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            match take_slot(state, user_id, slot_id).await? {
                true => acknowledge_callback(state, &cb.id, "C'est note, le poste est a toi").await,
                false => acknowledge_callback(state, &cb.id, "Ce poste est complet").await,
            }
        }
        "publie" => {
            let Some(tid) = uuid(0) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            match mark_published(state, user_id, tid).await? {
                true => acknowledge_callback(state, &cb.id, "Publication confirmee, merci").await,
                false => {
                    acknowledge_callback(state, &cb.id, "Cette tache ne t'est pas assignee").await
                }
            }
        }
        "ack" => {
            let Some(eid) = uuid(0) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            acknowledge_line_up(state, user_id, eid).await?;
            acknowledge_callback(state, &cb.id, "Bien recu").await
        }
        "fiche" => {
            let Some(gid) = uuid(0) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            acknowledge_callback(state, &cb.id, "Envoi en cours").await?;
            send_rider(state, chat_id, user_id, gid).await
        }
        "collectif" => {
            let Some(cid) = uuid(0) else {
                return acknowledge_callback(state, &cb.id, "Bouton illisible").await;
            };
            let change = sqlx::query(
                "UPDATE users SET telegram_collective_id = $2 WHERE id = $1
                 AND EXISTS (SELECT 1 FROM memberships m
                             WHERE m.user_id = $1 AND m.collective_id = $2)",
            )
            .bind(user_id)
            .bind(cid)
            .execute(&state.db)
            .await?;
            match change.rows_affected() {
                0 => acknowledge_callback(state, &cb.id, "Tu n'es pas dans ce collectif").await,
                _ => acknowledge_callback(state, &cb.id, "Collectif courant change").await,
            }
        }
        _ => acknowledge_callback(state, &cb.id, "Bouton inconnu").await,
    }
}

// --- Domain writes -------------------------------------------------------------

/// An availability is only written by the person it belongs to (§5.2): here it
/// is structural — whoever taps is whoever answers.
async fn answer_availability(
    state: &AppState,
    user_id: Uuid,
    date_id: Uuid,
    status: &str,
) -> AppResult<bool> {
    if !matches!(status, "yes" | "maybe" | "no") {
        return Ok(false);
    }

    let poll: Option<(Uuid,)> = sqlx::query_as(
        "SELECT p.id FROM availability_polls p
         JOIN candidate_dates d ON d.opportunity_id = p.opportunity_id
         JOIN opportunities o ON o.id = p.opportunity_id
         JOIN memberships m ON m.collective_id = o.collective_id AND m.user_id = $2
         WHERE d.id = $1 AND p.closed_at IS NULL",
    )
    .bind(date_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((poll_id,)) = poll else {
        return Ok(false);
    };

    sqlx::query(
        "INSERT INTO availabilities (poll_id, candidate_date_id, user_id, status, source)
         VALUES ($1, $2, $3, $4, 'telegram')
         ON CONFLICT (candidate_date_id, user_id)
         DO UPDATE SET status = EXCLUDED.status, source = 'telegram', updated_at = now()",
    )
    .bind(poll_id)
    .bind(date_id)
    .bind(user_id)
    .bind(status)
    .execute(&state.db)
    .await?;
    Ok(true)
}

async fn toggle_wants_to_play(state: &AppState, user_id: Uuid, date_id: Uuid) -> AppResult<bool> {
    let line: Option<(bool,)> = sqlx::query_as(
        "UPDATE availabilities SET wants_to_play = NOT wants_to_play, updated_at = now()
         WHERE candidate_date_id = $1 AND user_id = $2 RETURNING wants_to_play",
    )
    .bind(date_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    match line {
        Some((wants,)) => Ok(wants),
        // Wanting to play without having said "dispo" makes no sense: record
        // the answer at the same time.
        None => {
            answer_availability(state, user_id, date_id, "yes").await?;
            let line: Option<(bool,)> = sqlx::query_as(
                "UPDATE availabilities SET wants_to_play = TRUE, updated_at = now()
                 WHERE candidate_date_id = $1 AND user_id = $2 RETURNING wants_to_play",
            )
            .bind(date_id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
            Ok(line.map(|(v,)| v).unwrap_or(false))
        }
    }
}

async fn take_slot(state: &AppState, user_id: Uuid, slot_id: Uuid) -> AppResult<bool> {
    let slot: Option<(i32, i64)> = sqlx::query_as(
        "SELECT s.quantity,
                (SELECT count(*) FROM logistics_assignments a WHERE a.slot_id = s.id)
         FROM logistics_slots s
         JOIN events e ON e.id = s.event_id
         JOIN memberships m ON m.collective_id = e.collective_id AND m.user_id = $2
         WHERE s.id = $1",
    )
    .bind(slot_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    let Some((quantity, pris)) = slot else {
        return Ok(false);
    };
    if pris >= quantity as i64 {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO logistics_assignments (slot_id, user_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(slot_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;
    Ok(true)
}

/// No task reaches "publie" without an explicit human action (§18).
async fn mark_published(state: &AppState, user_id: Uuid, task_id: Uuid) -> AppResult<bool> {
    let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT assignee_id, backup_assignee_id FROM publication_tasks WHERE id = $1",
    )
    .bind(task_id)
    .fetch_optional(&state.db)
    .await?;

    let Some((assignee, backup)) = row else {
        return Ok(false);
    };
    if assignee != Some(user_id) && backup != Some(user_id) {
        return Ok(false);
    }

    sqlx::query(
        "UPDATE publication_tasks SET status = 'published', published_at = now(),
                                      updated_at = now()
         WHERE id = $1",
    )
    .bind(task_id)
    .execute(&state.db)
    .await?;

    // The scheduled reminders no longer have a reason to exist.
    crate::services::jobs::cancel_by_prefix(&state.db, &format!("task:{task_id}:")).await?;
    Ok(true)
}

async fn acknowledge_line_up(state: &AppState, user_id: Uuid, event_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE participations p SET acknowledged_at = now()
         WHERE p.event_id = $1 AND p.acknowledged_at IS NULL
           AND (p.user_id = $2
                OR EXISTS (SELECT 1 FROM group_members gm
                           WHERE gm.group_id = p.group_id AND gm.user_id = $2))",
    )
    .bind(event_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// The tech rider goes out as a PDF straight from the chat, in under a minute
/// (§18).
async fn send_rider(
    state: &AppState,
    chat_id: i64,
    user_id: Uuid,
    group_id: Uuid,
) -> AppResult<()> {
    let row: Option<(Uuid, i32, Value, String)> = sqlx::query_as(
        "SELECT r.id, r.version, r.data, g.name FROM tech_riders r
         JOIN groups g ON g.id = r.group_id
         JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = $2
         WHERE r.group_id = $1 AND r.status = 'published'
         ORDER BY r.version DESC LIMIT 1",
    )
    .bind(group_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    let Some((rider_id, version, data, name)) = row else {
        return reply(state, chat_id, "Aucune fiche publiee pour ce groupe.", None).await;
    };

    let payload = json!({
        "group_name": name,
        "version": version,
        "rider": data,
        "generated_on": Utc::now().format("%d/%m/%Y").to_string(),
    });

    let bytes = match pdf::compile("tech-rider.typ", &payload).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(error = %e, "bot: PDF compilation");
            return reply(state, chat_id, "La fiche n'a pas pu etre produite.", None).await;
        }
    };

    let key = format!("riders/{rider_id}-v{version}.pdf");
    state
        .storage
        .put(&key, bytes, "application/pdf")
        .await
        .map_err(crate::error::AppError::Internal)?;
    let url = state
        .storage
        .signed_url(&key, 3600)
        .await
        .map_err(crate::error::AppError::Internal)?;

    state
        .telegram
        .send_document(OutgoingDocument {
            chat_id,
            url,
            filename: format!("fiche-technique-{name}-v{version}.pdf"),
            caption: format!("<b>{name}</b> — fiche technique v{version}"),
        })
        .await
        .map_err(crate::error::AppError::Internal)?;
    Ok(())
}

// --- Helpers -------------------------------------------------------------------

async fn user_for_telegram_id(db: &PgPool, telegram_id: Option<i64>) -> AppResult<Option<Uuid>> {
    let Some(tg) = telegram_id else {
        return Ok(None);
    };
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE telegram_id = $1")
        .bind(tg)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|(id,)| id))
}

/// The collective the conversation is about: the one chosen with `/collectif`,
/// otherwise the first — a member of a single collective has nothing to pick.
async fn current_collective(db: &PgPool, user_id: Uuid) -> AppResult<Option<(Uuid, String)>> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT c.id, c.name FROM memberships m
         JOIN collectives c ON c.id = m.collective_id
         JOIN users u ON u.id = m.user_id
         WHERE m.user_id = $1
         ORDER BY (c.id = u.telegram_collective_id) DESC, c.name
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

async fn task_visual(state: &AppState, task_id: Uuid) -> Option<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT a.storage_key FROM publication_assets pa
         JOIN assets a ON a.id = pa.asset_id
         WHERE pa.publication_task_id = $1 AND pa.status = 'ready'
         ORDER BY pa.created_at LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (key,) = row?;
    state.storage.signed_url(&key, 3600).await.ok()
}

async fn reply(
    state: &AppState,
    chat_id: i64,
    text: &str,
    keyboard: Option<Value>,
) -> AppResult<()> {
    send(state, chat_id, text, keyboard, None).await
}

async fn send(
    state: &AppState,
    chat_id: i64,
    text: &str,
    keyboard: Option<Value>,
    photo_url: Option<String>,
) -> AppResult<()> {
    state
        .telegram
        .send(OutgoingMessage {
            chat_id,
            text: text.to_string(),
            keyboard,
            photo_url,
        })
        .await
        .map_err(crate::error::AppError::Internal)?;
    Ok(())
}

async fn acknowledge_callback(state: &AppState, callback_id: &str, text: &str) -> AppResult<()> {
    state
        .telegram
        .answer_callback(callback_id, text)
        .await
        .map_err(crate::error::AppError::Internal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_command_with_or_without_the_bot_mention() {
        assert_eq!(
            parse_command("/dispos"),
            Some(("dispos".into(), String::new()))
        );
        assert_eq!(
            parse_command("/start@backline_bot AB12"),
            Some(("start".into(), "AB12".into()))
        );
        assert_eq!(parse_command("bonjour"), None);
    }

    #[test]
    fn reads_a_button_payload() {
        let (action, args) = parse_callback("dispo:11111111-1111-1111-1111-111111111111:yes");
        assert_eq!(action, "dispo");
        assert_eq!(args.len(), 2);
        assert_eq!(args[1], "yes");
    }

    #[test]
    fn a_publication_task_arrives_with_its_button() {
        let kb = keyboard_for("publication_due", &json!({ "task_id": "abc" })).unwrap();
        assert_eq!(kb[0][0]["callback_data"], "publie:abc");
    }

    #[test]
    fn a_notification_without_an_action_has_no_keyboard() {
        assert!(keyboard_for("stream_live", &json!({ "event_id": "abc" })).is_none());
    }

    /// A Telegram button's data cannot exceed 64 bytes.
    #[test]
    fn buttons_fit_within_telegram_s_limit() {
        let id = Uuid::new_v4();
        for data in [
            format!("dispo:{id}:maybe"),
            format!("jouer:{id}"),
            format!("poste:{id}"),
            format!("publie:{id}"),
            format!("fiche:{id}"),
            format!("collectif:{id}"),
            format!("dispos:{id}"),
            format!("postes:{id}"),
            format!("ack:{id}"),
        ] {
            assert!(data.len() <= 64, "{data} depasse 64 octets");
        }
    }
}

//! Bot Telegram (§13) : **canal principal des membres**, doublon complet du
//! web. Toute decision tient en deux appuis — une commande, un bouton.
//!
//! Le bot ne contourne aucune regle metier : une disponibilite n'est ecrite
//! que par la personne concernee, une tache de publication n'est confirmee que
//! par son responsable, et l'appartenance au collectif est verifiee a chaque
//! action.

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

const AIDE: &str = "Commandes : /dispos · /agenda · /postes · /publier · /fiche · /collectif";

/// Boucle de long polling. Un seul processus la tient (service `bot`).
pub async fn run_forever(state: AppState) {
    if !state.telegram.enabled() {
        // Sans jeton, il n'y a rien a lire. Le service reste en vie plutot que
        // de sortir en boucle : le web est un doublon complet du bot (§19).
        tracing::warn!("bot : aucun jeton Telegram — service au repos");
        futures::future::pending::<()>().await;
        return;
    }

    let mut offset = 0i64;
    tracing::info!("bot : a l'ecoute");

    loop {
        match state.telegram.poll_updates(offset, 30).await {
            Ok(updates) => {
                for update in updates {
                    offset = offset.max(update.update_id + 1);
                    if let Err(e) = handle_update(&state, update).await {
                        tracing::warn!(error = %e, "bot : mise a jour non traitee");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "bot : lecture des mises a jour");
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

// --- Analyse -----------------------------------------------------------------

/// `/dispos@backline_bot 2` -> `("dispos", "2")`. Renvoie `None` si le message
/// n'est pas une commande.
pub fn parse_command(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    let reste = text.strip_prefix('/')?;
    let (tete, argument) = match reste.split_once(char::is_whitespace) {
        Some((t, a)) => (t, a.trim()),
        None => (reste, ""),
    };
    let commande = tete.split('@').next().unwrap_or(tete).to_lowercase();
    if commande.is_empty() {
        return None;
    }
    Some((commande, argument.to_string()))
}

/// Donnee d'un bouton : `action:argument[:valeur]`, 64 octets au plus.
pub fn parse_callback(data: &str) -> (String, Vec<String>) {
    let mut parts = data.split(':');
    let action = parts.next().unwrap_or_default().to_string();
    (action, parts.map(|s| s.to_string()).collect())
}

/// Clavier attache a une notification delivree par Telegram : c'est ce qui
/// fait du bot un canal d'action et pas un journal (§13).
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
    let texte = msg.text.clone().unwrap_or_default();
    let Some((commande, argument)) = parse_command(&texte) else {
        return repondre(
            state,
            chat_id,
            &format!("Je ne comprends que des commandes.\n{AIDE}"),
            None,
        )
        .await;
    };

    // `/start <code>` est la seule commande ouverte : c'est elle qui lie le
    // compte.
    if commande == "start" {
        return lier_compte(state, &msg, &argument).await;
    }

    let Some(user_id) = utilisateur(&state.db, msg.from.as_ref().map(|u| u.id)).await? else {
        return repondre(
            state,
            chat_id,
            "Ce compte Telegram n'est lie a aucun membre. Envoie <code>/start &lt;code&gt;</code> \
             avec le code de ton invitation.",
            None,
        )
        .await;
    };

    match commande.as_str() {
        "dispos" => dispos(state, chat_id, user_id).await,
        "agenda" => agenda(state, chat_id, user_id).await,
        "postes" => postes(state, chat_id, user_id).await,
        "publier" => publier(state, chat_id, user_id).await,
        "fiche" => fiches(state, chat_id, user_id).await,
        "collectif" => collectifs(state, chat_id, user_id).await,
        "aide" | "help" => repondre(state, chat_id, AIDE, None).await,
        _ => repondre(state, chat_id, &format!("Commande inconnue.\n{AIDE}"), None).await,
    }
}

async fn lier_compte(state: &AppState, msg: &Message, code: &str) -> AppResult<()> {
    let chat_id = msg.chat.id;
    let Some(from) = msg.from.as_ref() else {
        return Ok(());
    };

    if code.is_empty() {
        let deja = utilisateur(&state.db, Some(from.id)).await?;
        let texte = if deja.is_some() {
            format!("Ton compte est deja lie.\n{AIDE}")
        } else {
            "Envoie <code>/start &lt;code&gt;</code> avec le code de ton invitation.".into()
        };
        return repondre(state, chat_id, &texte, None).await;
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

    let Some((invitation_id, user_id, nom)) = row else {
        tx.rollback().await?;
        return repondre(state, chat_id, "Invitation inconnue ou perimee.", None).await;
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

    repondre(
        state,
        chat_id,
        &format!("Compte lie : <b>{nom}</b>.\n{AIDE}"),
        None,
    )
    .await
}

// --- Commandes ----------------------------------------------------------------

async fn dispos(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, _)) = collectif_courant(&state.db, user_id).await? else {
        return repondre(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
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
        return repondre(state, chat_id, "Aucun sondage ouvert.", None).await;
    }

    for (oid, titre) in polls {
        envoyer_sondage(state, chat_id, user_id, oid, &titre).await?;
    }
    Ok(())
}

/// Un message par date : trois boutons de reponse et un pour « je veux jouer ».
/// Deux appuis suffisent, c'est la regle du canal (§13).
async fn envoyer_sondage(
    state: &AppState,
    chat_id: i64,
    user_id: Uuid,
    oid: Uuid,
    titre: &str,
) -> AppResult<()> {
    let dates: Vec<(Uuid, chrono::NaiveDate, Option<chrono::NaiveTime>)> = sqlx::query_as(
        "SELECT id, day, start_time FROM candidate_dates
         WHERE opportunity_id = $1 ORDER BY position, day",
    )
    .bind(oid)
    .fetch_all(&state.db)
    .await?;

    repondre(state, chat_id, &format!("<b>{titre}</b>"), None).await?;

    for (date_id, jour, heure) in dates {
        let reponse: Option<(String, bool)> = sqlx::query_as(
            "SELECT status, wants_to_play FROM availabilities
             WHERE candidate_date_id = $1 AND user_id = $2",
        )
        .bind(date_id)
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;

        let marque = |statut: &str, icone: &str| -> String {
            match &reponse {
                Some((s, _)) if s == statut => format!("• {icone}"),
                _ => icone.to_string(),
            }
        };
        let jouer = match &reponse {
            Some((_, true)) => "• 🎸 je joue",
            _ => "🎸 je veux jouer",
        };

        let quand = match heure {
            Some(h) => format!("{} a {}", jour.format("%d/%m/%Y"), h.format("%H:%M")),
            None => jour.format("%d/%m/%Y").to_string(),
        };

        let clavier = json!([
            [
                { "text": marque("yes", "✅"), "callback_data": format!("dispo:{date_id}:yes") },
                { "text": marque("maybe", "❔"), "callback_data": format!("dispo:{date_id}:maybe") },
                { "text": marque("no", "❌"), "callback_data": format!("dispo:{date_id}:no") },
            ],
            [{ "text": jouer, "callback_data": format!("jouer:{date_id}") }],
        ]);

        repondre(state, chat_id, &quand, Some(clavier)).await?;
    }
    Ok(())
}

async fn agenda(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, nom)) = collectif_courant(&state.db, user_id).await? else {
        return repondre(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
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
        return repondre(state, chat_id, &format!("{nom} : rien de prevu."), None).await;
    }

    let lignes: Vec<String> = events
        .into_iter()
        .map(|(titre, quand, lieu, statut)| {
            let local = quand.with_timezone(&Paris);
            let lieu = lieu.unwrap_or_else(|| "lieu a caler".into());
            let mention = if statut == "draft" {
                " (brouillon)"
            } else {
                ""
            };
            format!(
                "• {} — <b>{titre}</b>{mention}\n  {lieu}",
                local.format("%d/%m %H:%M")
            )
        })
        .collect();

    repondre(
        state,
        chat_id,
        &format!("<b>{nom}</b>\n{}", lignes.join("\n")),
        None,
    )
    .await
}

async fn postes(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, _)) = collectif_courant(&state.db, user_id).await? else {
        return repondre(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
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

    let vacants: Vec<_> = slots
        .into_iter()
        .filter(|(_, _, quantity, pris, _, _)| *pris < *quantity as i64)
        .collect();

    if vacants.is_empty() {
        return repondre(state, chat_id, "Aucun poste vacant. 🎉", None).await;
    }

    for (slot_id, label, quantity, pris, titre, quand) in vacants {
        let local = quand.with_timezone(&Paris);
        let texte = format!(
            "<b>{label}</b> — {titre}\n{} · encore {} place(s)",
            local.format("%d/%m %H:%M"),
            quantity as i64 - pris
        );
        let clavier =
            json!([[{ "text": "Je le prends", "callback_data": format!("poste:{slot_id}") }]]);
        repondre(state, chat_id, &texte, Some(clavier)).await?;
    }
    Ok(())
}

async fn publier(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let taches: Vec<(Uuid, String, String, String, chrono::DateTime<Utc>, String)> =
        sqlx::query_as(
            "SELECT p.id, p.label, p.caption, p.hashtags, p.scheduled_at, e.title
             FROM publication_tasks p JOIN events e ON e.id = p.event_id
             WHERE (p.assignee_id = $1 OR p.backup_assignee_id = $1)
               AND p.status <> 'published'
             ORDER BY p.scheduled_at LIMIT 10",
        )
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;

    if taches.is_empty() {
        return repondre(state, chat_id, "Aucune publication a ta charge.", None).await;
    }

    for (tid, label, caption, hashtags, quand, evenement) in taches {
        let local = quand.with_timezone(&Paris);
        let texte = format!(
            "<b>{label}</b> — {evenement}\n{}\n\n{caption}\n{hashtags}",
            local.format("%d/%m %H:%M")
        );
        let clavier = json!([[{ "text": "✅ publie", "callback_data": format!("publie:{tid}") }]]);
        // Le visuel part avec le texte : tout est pret a coller (§11.2).
        let photo = visuel_de_tache(state, tid).await;
        envoyer(state, chat_id, &texte, Some(clavier), photo).await?;
    }
    Ok(())
}

async fn fiches(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let Some((cid, _)) = collectif_courant(&state.db, user_id).await? else {
        return repondre(state, chat_id, "Tu n'appartiens a aucun collectif.", None).await;
    };

    let groupes: Vec<(Uuid, String, Option<i32>)> = sqlx::query_as(
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

    if groupes.is_empty() {
        return repondre(
            state,
            chat_id,
            "Tu n'es dans aucun groupe de ce collectif.",
            None,
        )
        .await;
    }

    for (gid, nom, version) in groupes {
        match version {
            Some(v) => {
                let clavier = json!([[{ "text": format!("Envoyer la v{v}"), "callback_data": format!("fiche:{gid}") }]]);
                repondre(
                    state,
                    chat_id,
                    &format!("<b>{nom}</b> — fiche v{v}"),
                    Some(clavier),
                )
                .await?;
            }
            // L'absence de fiche est signalee, jamais bloquante (§12).
            None => {
                repondre(
                    state,
                    chat_id,
                    &format!("<b>{nom}</b> — aucune fiche publiee."),
                    None,
                )
                .await?
            }
        }
    }
    Ok(())
}

async fn collectifs(state: &AppState, chat_id: i64, user_id: Uuid) -> AppResult<()> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT c.id, c.name FROM memberships m JOIN collectives c ON c.id = m.collective_id
         WHERE m.user_id = $1 ORDER BY c.name",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    if rows.len() <= 1 {
        let nom = rows.first().map(|(_, n)| n.clone()).unwrap_or_default();
        return repondre(
            state,
            chat_id,
            &format!("Tu n'es que dans <b>{nom}</b> — rien a basculer."),
            None,
        )
        .await;
    }

    let courant = collectif_courant(&state.db, user_id)
        .await?
        .map(|(id, _)| id);
    let boutons: Vec<Value> = rows
        .iter()
        .map(|(id, nom)| {
            let marque = if Some(*id) == courant { "• " } else { "" };
            json!([{ "text": format!("{marque}{nom}"), "callback_data": format!("collectif:{id}") }])
        })
        .collect();

    repondre(state, chat_id, "Sur quel collectif ?", Some(json!(boutons))).await
}

// --- Boutons -------------------------------------------------------------------

async fn handle_callback(state: &AppState, cb: CallbackQuery) -> AppResult<()> {
    let chat_id = cb.message.as_ref().map(|m| m.chat.id).unwrap_or(cb.from.id);
    let data = cb.data.clone().unwrap_or_default();
    let (action, args) = parse_callback(&data);

    let Some(user_id) = utilisateur(&state.db, Some(cb.from.id)).await? else {
        return acquitter(state, &cb.id, "Compte non lie").await;
    };

    let uuid = |i: usize| -> Option<Uuid> { args.get(i).and_then(|s| Uuid::parse_str(s).ok()) };

    match action.as_str() {
        "dispo" => {
            let (Some(date_id), Some(statut)) = (uuid(0), args.get(1)) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
            };
            match repondre_dispo(state, user_id, date_id, statut).await? {
                true => acquitter(state, &cb.id, "Reponse enregistree").await,
                false => acquitter(state, &cb.id, "Sondage clos").await,
            }
        }
        "jouer" => {
            let Some(date_id) = uuid(0) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
            };
            let veut = basculer_jouer(state, user_id, date_id).await?;
            let texte = if veut {
                "Note : tu veux jouer"
            } else {
                "Retire"
            };
            acquitter(state, &cb.id, texte).await
        }
        "dispos" => {
            let Some(oid) = uuid(0) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
            };
            let titre: Option<(String,)> = sqlx::query_as(
                "SELECT o.title FROM opportunities o
                 JOIN memberships m ON m.collective_id = o.collective_id AND m.user_id = $2
                 WHERE o.id = $1",
            )
            .bind(oid)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
            match titre {
                Some((titre,)) => {
                    acquitter(state, &cb.id, "").await?;
                    envoyer_sondage(state, chat_id, user_id, oid, &titre).await
                }
                None => acquitter(state, &cb.id, "Hors de tes collectifs").await,
            }
        }
        "postes" => {
            acquitter(state, &cb.id, "").await?;
            postes(state, chat_id, user_id).await
        }
        "poste" => {
            let Some(slot_id) = uuid(0) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
            };
            match prendre_poste(state, user_id, slot_id).await? {
                true => acquitter(state, &cb.id, "C'est note, le poste est a toi").await,
                false => acquitter(state, &cb.id, "Ce poste est complet").await,
            }
        }
        "publie" => {
            let Some(tid) = uuid(0) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
            };
            match marquer_publie(state, user_id, tid).await? {
                true => acquitter(state, &cb.id, "Publication confirmee, merci").await,
                false => acquitter(state, &cb.id, "Cette tache ne t'est pas assignee").await,
            }
        }
        "ack" => {
            let Some(eid) = uuid(0) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
            };
            accuser_reception(state, user_id, eid).await?;
            acquitter(state, &cb.id, "Bien recu").await
        }
        "fiche" => {
            let Some(gid) = uuid(0) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
            };
            acquitter(state, &cb.id, "Envoi en cours").await?;
            envoyer_fiche(state, chat_id, user_id, gid).await
        }
        "collectif" => {
            let Some(cid) = uuid(0) else {
                return acquitter(state, &cb.id, "Bouton illisible").await;
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
                0 => acquitter(state, &cb.id, "Tu n'es pas dans ce collectif").await,
                _ => acquitter(state, &cb.id, "Collectif courant change").await,
            }
        }
        _ => acquitter(state, &cb.id, "Bouton inconnu").await,
    }
}

// --- Ecritures metier ----------------------------------------------------------

/// Une disponibilite n'est ecrite que par la personne concernee (§5.2) : ici,
/// c'est structurel, l'auteur du clic est l'auteur de la reponse.
async fn repondre_dispo(
    state: &AppState,
    user_id: Uuid,
    date_id: Uuid,
    statut: &str,
) -> AppResult<bool> {
    if !matches!(statut, "yes" | "maybe" | "no") {
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
    .bind(statut)
    .execute(&state.db)
    .await?;
    Ok(true)
}

async fn basculer_jouer(state: &AppState, user_id: Uuid, date_id: Uuid) -> AppResult<bool> {
    let ligne: Option<(bool,)> = sqlx::query_as(
        "UPDATE availabilities SET wants_to_play = NOT wants_to_play, updated_at = now()
         WHERE candidate_date_id = $1 AND user_id = $2 RETURNING wants_to_play",
    )
    .bind(date_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    match ligne {
        Some((veut,)) => Ok(veut),
        // Vouloir jouer sans avoir dit « dispo » n'a pas de sens : on pose la
        // reponse en meme temps.
        None => {
            repondre_dispo(state, user_id, date_id, "yes").await?;
            let ligne: Option<(bool,)> = sqlx::query_as(
                "UPDATE availabilities SET wants_to_play = TRUE, updated_at = now()
                 WHERE candidate_date_id = $1 AND user_id = $2 RETURNING wants_to_play",
            )
            .bind(date_id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
            Ok(ligne.map(|(v,)| v).unwrap_or(false))
        }
    }
}

async fn prendre_poste(state: &AppState, user_id: Uuid, slot_id: Uuid) -> AppResult<bool> {
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

/// Aucune tache ne passe a « publie » sans action humaine explicite (§18).
async fn marquer_publie(state: &AppState, user_id: Uuid, task_id: Uuid) -> AppResult<bool> {
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

    // Les relances programmees n'ont plus lieu d'etre.
    crate::services::jobs::cancel_by_prefix(&state.db, &format!("task:{task_id}:")).await?;
    Ok(true)
}

async fn accuser_reception(state: &AppState, user_id: Uuid, event_id: Uuid) -> AppResult<()> {
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

/// La fiche technique part en PDF depuis la conversation, en moins d'une
/// minute (§18).
async fn envoyer_fiche(
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

    let Some((rider_id, version, data, nom)) = row else {
        return repondre(state, chat_id, "Aucune fiche publiee pour ce groupe.", None).await;
    };

    let payload = json!({
        "group_name": nom,
        "version": version,
        "rider": data,
        "generated_on": Utc::now().format("%d/%m/%Y").to_string(),
    });

    let bytes = match pdf::compile("tech-rider.typ", &payload).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(error = %e, "bot : compilation PDF");
            return repondre(state, chat_id, "La fiche n'a pas pu etre produite.", None).await;
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
            filename: format!("fiche-technique-{nom}-v{version}.pdf"),
            caption: format!("<b>{nom}</b> — fiche technique v{version}"),
        })
        .await
        .map_err(crate::error::AppError::Internal)?;
    Ok(())
}

// --- Outils --------------------------------------------------------------------

async fn utilisateur(db: &PgPool, telegram_id: Option<i64>) -> AppResult<Option<Uuid>> {
    let Some(tg) = telegram_id else {
        return Ok(None);
    };
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE telegram_id = $1")
        .bind(tg)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|(id,)| id))
}

/// Collectif sur lequel porte la conversation : celui choisi par `/collectif`,
/// sinon le premier — un membre d'un seul collectif n'a rien a choisir.
async fn collectif_courant(db: &PgPool, user_id: Uuid) -> AppResult<Option<(Uuid, String)>> {
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

async fn visuel_de_tache(state: &AppState, task_id: Uuid) -> Option<String> {
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

async fn repondre(
    state: &AppState,
    chat_id: i64,
    texte: &str,
    clavier: Option<Value>,
) -> AppResult<()> {
    envoyer(state, chat_id, texte, clavier, None).await
}

async fn envoyer(
    state: &AppState,
    chat_id: i64,
    texte: &str,
    clavier: Option<Value>,
    photo_url: Option<String>,
) -> AppResult<()> {
    state
        .telegram
        .send(OutgoingMessage {
            chat_id,
            text: texte.to_string(),
            keyboard: clavier,
            photo_url,
        })
        .await
        .map_err(crate::error::AppError::Internal)?;
    Ok(())
}

async fn acquitter(state: &AppState, callback_id: &str, texte: &str) -> AppResult<()> {
    state
        .telegram
        .answer_callback(callback_id, texte)
        .await
        .map_err(crate::error::AppError::Internal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_une_commande_avec_ou_sans_mention_du_bot() {
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
    fn lit_la_donnee_d_un_bouton() {
        let (action, args) = parse_callback("dispo:11111111-1111-1111-1111-111111111111:yes");
        assert_eq!(action, "dispo");
        assert_eq!(args.len(), 2);
        assert_eq!(args[1], "yes");
    }

    #[test]
    fn une_tache_de_publication_arrive_avec_son_bouton() {
        let kb = keyboard_for("publication_due", &json!({ "task_id": "abc" })).unwrap();
        assert_eq!(kb[0][0]["callback_data"], "publie:abc");
    }

    #[test]
    fn une_notification_sans_action_n_a_pas_de_clavier() {
        assert!(keyboard_for("stream_live", &json!({ "event_id": "abc" })).is_none());
    }

    /// La donnee d'un bouton Telegram ne peut pas depasser 64 octets.
    #[test]
    fn les_boutons_tiennent_dans_la_limite_de_telegram() {
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

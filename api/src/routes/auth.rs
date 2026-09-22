//! Connexion et invitations (§3). Aucune inscription publique.

use crate::auth::{self, session};
use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login/password", post(login_password))
        .route("/login/telegram", post(login_telegram))
        .route("/logout", post(logout))
        .route(
            "/invitation/:code",
            get(peek_invitation).post(accept_invitation),
        )
}

#[derive(Deserialize)]
struct PasswordLogin {
    email: String,
    password: String,
}

/// Secours : e-mail + mot de passe. Obligatoire pour au moins un admin
/// d'instance, afin de ne jamais dependre de Telegram pour l'administration.
async fn login_password(
    State(state): State<AppState>,
    Json(body): Json<PasswordLogin>,
) -> AppResult<impl IntoResponse> {
    let row: Option<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE lower(email) = lower($1)")
            .bind(&body.email)
            .fetch_optional(&state.db)
            .await?;

    // Meme message dans tous les cas : on ne dit pas si l'adresse existe.
    let invalid = || AppError::forbidden("identifiants invalides");
    let (user_id, hash) = row.ok_or_else(invalid)?;
    let hash = hash.ok_or_else(invalid)?;
    if !auth::verify_password(&body.password, &hash) {
        return Err(invalid());
    }

    let token = session::create_session(&state.db, user_id).await?;
    let secure = state.config.public_base_url.starts_with("https");
    Ok((
        [(SET_COOKIE, session::cookie_header(&token, secure))],
        Json(json!({ "token": token, "user_id": user_id })),
    ))
}

/// Telegram Login Widget. La signature est verifiee cote Rust (§15) ; sans
/// cela n'importe qui pourrait se declarer n'importe quel compte.
async fn login_telegram(
    State(state): State<AppState>,
    Json(body): Json<auth::TelegramLoginData>,
) -> AppResult<impl IntoResponse> {
    let token_secret = state.config.telegram_bot_token.as_ref().ok_or_else(|| {
        AppError::forbidden("connexion Telegram non configuree sur cette instance")
    })?;

    if !auth::verify_telegram_login(&body, token_secret, chrono::Utc::now().timestamp()) {
        return Err(AppError::forbidden("signature Telegram invalide"));
    }

    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE telegram_id = $1")
        .bind(body.id)
        .fetch_optional(&state.db)
        .await?;

    // Pas de creation implicite : un compte existe parce qu'un admin l'a cree.
    let (user_id,) = row.ok_or_else(|| {
        AppError::forbidden("ce compte Telegram n'est lie a aucun membre — demande une invitation")
    })?;

    let token = session::create_session(&state.db, user_id).await?;
    let secure = state.config.public_base_url.starts_with("https");
    Ok((
        [(SET_COOKIE, session::cookie_header(&token, secure))],
        Json(json!({ "token": token, "user_id": user_id })),
    ))
}

async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> AppResult<impl IntoResponse> {
    if let Some(raw) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = raw
            .split(';')
            .filter_map(|kv| kv.trim().split_once('='))
            .find(|(k, _)| *k == session::SESSION_COOKIE)
            .map(|(_, v)| v)
        {
            session::destroy_session(&state.db, token).await?;
        }
    }
    Ok((
        [(SET_COOKIE, session::clear_cookie_header())],
        Json(json!({ "ok": true })),
    ))
}

#[derive(Serialize)]
struct InvitationPeek {
    display_name: String,
    collectives: Vec<String>,
    bot_username: Option<String>,
}

async fn peek_invitation(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> AppResult<Json<InvitationPeek>> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT u.id, u.display_name FROM invitations i JOIN users u ON u.id = i.user_id
         WHERE i.code = $1 AND i.used_at IS NULL AND i.expires_at > now()",
    )
    .bind(&code)
    .fetch_optional(&state.db)
    .await?;
    let (user_id, display_name) =
        row.ok_or_else(|| AppError::not_found("invitation inconnue ou perimee"))?;

    let collectives: Vec<(String,)> = sqlx::query_as(
        "SELECT c.name FROM memberships m JOIN collectives c ON c.id = m.collective_id
         WHERE m.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(InvitationPeek {
        display_name,
        collectives: collectives.into_iter().map(|(n,)| n).collect(),
        bot_username: state.config.telegram_bot_username.clone(),
    }))
}

#[derive(Deserialize)]
struct AcceptInvitation {
    #[serde(default)]
    telegram: Option<auth::TelegramLoginData>,
    /// Secours hors Telegram : un mot de passe, refuse si un compte Telegram
    /// est deja lie.
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

/// Consomme le lien a usage unique et ouvre une session.
async fn accept_invitation(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Json(body): Json<AcceptInvitation>,
) -> AppResult<impl IntoResponse> {
    let mut tx = state.db.begin().await?;

    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT i.id, i.user_id FROM invitations i
         WHERE i.code = $1 AND i.used_at IS NULL AND i.expires_at > now()
         FOR UPDATE",
    )
    .bind(&code)
    .fetch_optional(&mut *tx)
    .await?;
    let (invitation_id, user_id) =
        row.ok_or_else(|| AppError::not_found("invitation inconnue ou perimee"))?;

    match (&body.telegram, &body.password) {
        (Some(tg), _) => {
            let secret = state.config.telegram_bot_token.as_ref().ok_or_else(|| {
                AppError::forbidden("connexion Telegram non configuree sur cette instance")
            })?;
            if !auth::verify_telegram_login(tg, secret, chrono::Utc::now().timestamp()) {
                return Err(AppError::forbidden("signature Telegram invalide"));
            }
            sqlx::query(
                "UPDATE users SET telegram_id = $2, telegram_username = $3, updated_at = now()
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(tg.id)
            .bind(&tg.username)
            .execute(&mut *tx)
            .await?;
        }
        (None, Some(pw)) => {
            if pw.chars().count() < 10 {
                return Err(AppError::bad_request(
                    "mot de passe trop court (10 caracteres)",
                ));
            }
            let hash = auth::hash_password(pw).map_err(AppError::Internal)?;
            sqlx::query(
                "UPDATE users SET password_hash = $2, email = COALESCE($3, email), updated_at = now()
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(hash)
            .bind(&body.email)
            .execute(&mut *tx)
            .await?;
        }
        (None, None) => {
            return Err(AppError::bad_request(
                "fournir une identification Telegram ou un mot de passe",
            ))
        }
    }

    sqlx::query("UPDATE invitations SET used_at = now() WHERE id = $1")
        .bind(invitation_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let token = session::create_session(&state.db, user_id).await?;
    let secure = state.config.public_base_url.starts_with("https");
    Ok((
        [(SET_COOKIE, session::cookie_header(&token, secure))],
        Json(json!({ "token": token, "user_id": user_id })),
    ))
}

#[derive(Serialize)]
pub struct Me {
    pub id: Uuid,
    pub display_name: String,
    pub stage_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub telegram_linked: bool,
    pub is_instance_admin: bool,
    pub collectives: Vec<MyCollective>,
    pub groups: Vec<MyGroup>,
}

#[derive(Serialize)]
pub struct MyCollective {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct MyGroup {
    pub id: Uuid,
    pub collective_id: Uuid,
    pub name: String,
    pub is_admin: bool,
}

pub async fn me(State(state): State<AppState>, Auth(actor): Auth) -> AppResult<Json<Me>> {
    let u: (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        bool,
    ) = sqlx::query_as(
        "SELECT display_name, stage_name, phone, email, telegram_id, is_instance_admin
             FROM users WHERE id = $1",
    )
    .bind(actor.user_id)
    .fetch_one(&state.db)
    .await?;

    let collectives: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT c.id, c.slug, c.name, m.role FROM memberships m
         JOIN collectives c ON c.id = m.collective_id
         WHERE m.user_id = $1 ORDER BY c.name",
    )
    .bind(actor.user_id)
    .fetch_all(&state.db)
    .await?;

    let groups: Vec<(Uuid, Uuid, String, bool)> = sqlx::query_as(
        "SELECT g.id, g.collective_id, g.name, gm.is_admin FROM group_members gm
         JOIN groups g ON g.id = gm.group_id WHERE gm.user_id = $1 ORDER BY g.name",
    )
    .bind(actor.user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(Me {
        id: actor.user_id,
        display_name: u.0,
        stage_name: u.1,
        phone: u.2,
        email: u.3,
        telegram_linked: u.4.is_some(),
        is_instance_admin: u.5,
        collectives: collectives
            .into_iter()
            .map(|(id, slug, name, role)| MyCollective {
                id,
                slug,
                name,
                role,
            })
            .collect(),
        groups: groups
            .into_iter()
            .map(|(id, collective_id, name, is_admin)| MyGroup {
                id,
                collective_id,
                name,
                is_admin,
            })
            .collect(),
    }))
}

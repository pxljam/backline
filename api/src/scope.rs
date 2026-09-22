//! Cloisonnement multi-collectif (§4, §15).
//!
//! Regle : **aucune requete metier ne prend un `collective_id` brut**. Elle prend
//! un [`CollectiveScope`], qui ne peut etre construit qu'en prouvant l'appartenance
//! de l'utilisateur au collectif. Le filtre n'est donc pas une discipline
//! d'ecriture des requetes, c'est une contrainte de type.

use crate::error::{AppError, AppResult};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Member,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }
    pub fn parse(s: &str) -> Role {
        match s {
            "admin" => Role::Admin,
            _ => Role::Member,
        }
    }
}

/// L'utilisateur authentifie, sans aucun droit encore prouve.
#[derive(Clone, Copy, Debug)]
pub struct Actor {
    pub user_id: Uuid,
    pub is_instance_admin: bool,
}

impl Actor {
    pub fn require_instance_admin(&self) -> AppResult<()> {
        if self.is_instance_admin {
            Ok(())
        } else {
            Err(AppError::forbidden("reserve a l'administrateur d'instance"))
        }
    }
}

/// Preuve d'acces a un collectif. Sa seule voie de construction est
/// [`CollectiveScope::resolve`], qui interroge `memberships`.
#[derive(Clone, Copy, Debug)]
pub struct CollectiveScope {
    collective_id: Uuid,
    pub actor: Actor,
    pub role: Role,
}

impl CollectiveScope {
    pub async fn resolve(db: &PgPool, actor: Actor, collective_id: Uuid) -> AppResult<Self> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT role FROM memberships WHERE collective_id = $1 AND user_id = $2",
        )
        .bind(collective_id)
        .bind(actor.user_id)
        .fetch_optional(db)
        .await?;

        match row {
            Some((role,)) => Ok(Self {
                collective_id,
                actor,
                role: Role::parse(&role),
            }),
            // L'admin d'instance traverse les collectifs pour l'exploitation
            // technique, mais il doit exister.
            None if actor.is_instance_admin => {
                let exists: Option<(Uuid,)> =
                    sqlx::query_as("SELECT id FROM collectives WHERE id = $1")
                        .bind(collective_id)
                        .fetch_optional(db)
                        .await?;
                exists
                    .map(|_| Self {
                        collective_id,
                        actor,
                        role: Role::Admin,
                    })
                    .ok_or_else(|| AppError::not_found("collectif introuvable"))
            }
            // Ni 403 ni message : de l'exterieur, un collectif dont on n'est pas
            // membre n'existe pas.
            None => Err(AppError::not_found("collectif introuvable")),
        }
    }

    pub fn collective_id(&self) -> Uuid {
        self.collective_id
    }

    pub fn user_id(&self) -> Uuid {
        self.actor.user_id
    }

    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }

    pub fn require_admin(&self) -> AppResult<()> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(AppError::forbidden("reserve aux admins du collectif"))
        }
    }

    /// Une disponibilite n'est modifiable que par la personne concernee,
    /// **admins compris** (§5.2, §20). Le droit d'administration ne franchit
    /// jamais cette frontiere.
    pub fn require_self(&self, user_id: Uuid) -> AppResult<()> {
        if self.actor.user_id == user_id {
            Ok(())
        } else {
            Err(AppError::forbidden(
                "une disponibilite ne peut etre ecrite que par la personne concernee",
            ))
        }
    }

    /// Admin du collectif, ou admin du groupe vise.
    pub async fn require_group_admin(&self, db: &PgPool, group_id: Uuid) -> AppResult<()> {
        if self.is_admin() {
            return Ok(());
        }
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT gm.is_admin FROM group_members gm
             JOIN groups g ON g.id = gm.group_id
             WHERE gm.group_id = $1 AND gm.user_id = $2 AND g.collective_id = $3",
        )
        .bind(group_id)
        .bind(self.actor.user_id)
        .bind(self.collective_id)
        .fetch_optional(db)
        .await?;
        match row {
            Some((true,)) => Ok(()),
            _ => Err(AppError::forbidden("reserve aux admins du groupe")),
        }
    }

    /// Verifie qu'un groupe appartient bien a ce collectif avant tout usage.
    pub async fn check_group(&self, db: &PgPool, group_id: Uuid) -> AppResult<()> {
        let row: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM groups WHERE id = $1 AND collective_id = $2")
                .bind(group_id)
                .bind(self.collective_id)
                .fetch_optional(db)
                .await?;
        row.map(|_| ())
            .ok_or_else(|| AppError::not_found("groupe introuvable"))
    }
}

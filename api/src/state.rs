use crate::config::Config;
use crate::error::AppResult;
use crate::scope::{Actor, CollectiveScope};
use crate::services::storage::Storage;
use crate::services::telegram::Telegram;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub storage: Arc<Storage>,
    pub telegram: Arc<dyn Telegram>,
}

impl AppState {
    /// Unique porte d'entree vers les donnees d'un collectif.
    pub async fn scope(&self, actor: Actor, collective_id: Uuid) -> AppResult<CollectiveScope> {
        CollectiveScope::resolve(&self.db, actor, collective_id).await
    }
}

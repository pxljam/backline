//! Telegram channel (§13). The API never writes straight to Telegram from an
//! HTTP request: it drops off a notification, and a job delivers it.
//!
//! Without `TELEGRAM_BOT_TOKEN` the implementation is a log: the application
//! stays fully usable through the web, which is a complete mirror (§19).

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

/// An update received from the bot: a message, or a button press.
#[derive(Debug, Clone, Deserialize)]
pub struct Update {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default)]
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub chat: Chat,
    #[serde(default)]
    pub from: Option<User>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chat {
    pub id: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub first_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub message: Option<Message>,
}

#[derive(Debug, Clone)]
pub struct OutgoingDocument {
    pub chat_id: i64,
    pub url: String,
    pub filename: String,
    pub caption: String,
}

#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    pub chat_id: i64,
    pub text: String,
    /// Inline keyboard: `[[{ text, callback_data }]]`
    pub keyboard: Option<Value>,
    pub photo_url: Option<String>,
}

#[async_trait]
pub trait Telegram: Send + Sync {
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;

    /// Sends a file — a tech rider goes out as a PDF straight from the chat,
    /// without going through the web (§13 `/fiche`).
    async fn send_document(&self, _doc: OutgoingDocument) -> Result<()> {
        Ok(())
    }

    /// Long polling. Without a token, the bot loop has nothing to read.
    async fn poll_updates(&self, _offset: i64, _timeout_s: u64) -> Result<Vec<Update>> {
        Ok(Vec::new())
    }

    /// Acknowledges a button press: without it, Telegram leaves the loading
    /// spinner running on the member's phone.
    async fn answer_callback(&self, _callback_id: &str, _text: &str) -> Result<()> {
        Ok(())
    }

    fn enabled(&self) -> bool {
        true
    }
}

/// Fallback when no token is configured.
pub struct LoggingTelegram;

#[async_trait]
impl Telegram for LoggingTelegram {
    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        tracing::info!(chat_id = msg.chat_id, text = %msg.text, "telegram disabled — message logged");
        Ok(())
    }
    fn enabled(&self) -> bool {
        false
    }
}

pub struct HttpTelegram {
    token: String,
    client: reqwest::Client,
}

impl HttpTelegram {
    pub fn new(token: String) -> Self {
        Self {
            token,
            // Long polling holds a request open: the client must allow more
            // time than the timeout asked of Telegram.
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(90))
                .build()
                .unwrap_or_default(),
        }
    }

    fn url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
    }
}

#[async_trait]
impl Telegram for HttpTelegram {
    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        let (method, mut body) = match &msg.photo_url {
            Some(url) => (
                "sendPhoto",
                serde_json::json!({
                    "chat_id": msg.chat_id,
                    "photo": url,
                    "caption": msg.text,
                    "parse_mode": "HTML",
                }),
            ),
            None => (
                "sendMessage",
                serde_json::json!({
                    "chat_id": msg.chat_id,
                    "text": msg.text,
                    "parse_mode": "HTML",
                    "disable_web_page_preview": true,
                }),
            ),
        };
        if let Some(kb) = msg.keyboard {
            body["reply_markup"] = serde_json::json!({ "inline_keyboard": kb });
        }
        let resp = self
            .client
            .post(self.url(method))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("telegram {method} a repondu {status}: {text}");
        }
        Ok(())
    }

    async fn send_document(&self, doc: OutgoingDocument) -> Result<()> {
        let body = serde_json::json!({
            "chat_id": doc.chat_id,
            "document": doc.url,
            "caption": doc.caption,
            "parse_mode": "HTML",
        });
        let resp = self
            .client
            .post(self.url("sendDocument"))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("telegram sendDocument a repondu {status}: {text}");
        }
        Ok(())
    }

    async fn poll_updates(&self, offset: i64, timeout_s: u64) -> Result<Vec<Update>> {
        #[derive(Deserialize)]
        struct ApiResponse {
            ok: bool,
            #[serde(default)]
            result: Vec<Update>,
            #[serde(default)]
            description: Option<String>,
        }

        let resp = self
            .client
            .post(self.url("getUpdates"))
            .json(&serde_json::json!({
                "offset": offset,
                "timeout": timeout_s,
                "allowed_updates": ["message", "callback_query"],
            }))
            .send()
            .await?
            .json::<ApiResponse>()
            .await?;

        if !resp.ok {
            anyhow::bail!(
                "telegram getUpdates : {}",
                resp.description.unwrap_or_default()
            );
        }
        Ok(resp.result)
    }

    async fn answer_callback(&self, callback_id: &str, text: &str) -> Result<()> {
        self.client
            .post(self.url("answerCallbackQuery"))
            .json(&serde_json::json!({ "callback_query_id": callback_id, "text": text }))
            .send()
            .await?;
        Ok(())
    }
}

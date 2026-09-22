//! Authentication (§3). No public sign-up: an admin creates the user and
//! generates a single-use invitation link.
//!
//! - Web sign-in: Telegram Login Widget, HMAC signature verified here.
//! - Fallback: email + Argon2 password, mandatory for at least one instance
//!   admin, so administration never depends on Telegram.

pub mod password;
pub mod session;
pub mod telegram_login;

pub use password::{hash_password, verify_password};
pub use session::{create_session, destroy_session, SESSION_COOKIE};
pub use telegram_login::{verify_telegram_login, TelegramLoginData};

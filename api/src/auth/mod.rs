//! Authentification (§3). Aucune inscription publique : un admin cree
//! l'utilisateur et genere un lien d'invitation a usage unique.
//!
//! - Connexion web : Telegram Login Widget, signature HMAC verifiee ici.
//! - Secours : e-mail + mot de passe Argon2, obligatoire pour au moins un
//!   admin d'instance, pour ne jamais dependre de Telegram cote administration.

pub mod password;
pub mod session;
pub mod telegram_login;

pub use password::{hash_password, verify_password};
pub use session::{create_session, destroy_session, SESSION_COOKIE};
pub use telegram_login::{verify_telegram_login, TelegramLoginData};

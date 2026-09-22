//! Verification du Telegram Login Widget cote Rust (§15).
//!
//! Telegram signe les donnees du widget avec `HMAC-SHA256`, dont la cle est
//! `SHA256(bot_token)`. Sans cette verification, n'importe qui pourrait se
//! declarer n'importe quel `telegram_id`.

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramLoginData {
    pub id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub photo_url: Option<String>,
    pub auth_date: i64,
    pub hash: String,
}

/// `true` si la signature est valide et la connexion pas trop ancienne.
pub fn verify_telegram_login(data: &TelegramLoginData, bot_token: &str, now: i64) -> bool {
    if now - data.auth_date > 86_400 {
        return false;
    }

    // Le champ `hash` est exclu ; le reste est trie par cle, `k=v`, joint par \n.
    let mut fields: BTreeMap<&str, String> = BTreeMap::new();
    fields.insert("id", data.id.to_string());
    fields.insert("auth_date", data.auth_date.to_string());
    if let Some(v) = &data.first_name {
        fields.insert("first_name", v.clone());
    }
    if let Some(v) = &data.last_name {
        fields.insert("last_name", v.clone());
    }
    if let Some(v) = &data.username {
        fields.insert("username", v.clone());
    }
    if let Some(v) = &data.photo_url {
        fields.insert("photo_url", v.clone());
    }

    let check_string = fields
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");

    let secret = Sha256::digest(bot_token.as_bytes());
    let mut mac = match Hmac::<Sha256>::new_from_slice(&secret) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(check_string.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    // Comparaison a temps constant.
    constant_time_eq(expected.as_bytes(), data.hash.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::Mac;

    fn signed(token: &str, id: i64, auth_date: i64) -> TelegramLoginData {
        let check = format!("auth_date={auth_date}\nid={id}\nusername=romain");
        let secret = Sha256::digest(token.as_bytes());
        let mut mac = Hmac::<Sha256>::new_from_slice(&secret).unwrap();
        mac.update(check.as_bytes());
        TelegramLoginData {
            id,
            first_name: None,
            last_name: None,
            username: Some("romain".into()),
            photo_url: None,
            auth_date,
            hash: hex::encode(mac.finalize().into_bytes()),
        }
    }

    #[test]
    fn accepte_une_signature_valide() {
        let d = signed("123:abc", 42, 1_700_000_000);
        assert!(verify_telegram_login(&d, "123:abc", 1_700_000_100));
    }

    #[test]
    fn refuse_un_autre_jeton() {
        let d = signed("123:abc", 42, 1_700_000_000);
        assert!(!verify_telegram_login(&d, "999:xyz", 1_700_000_100));
    }

    #[test]
    fn refuse_une_connexion_perimee() {
        let d = signed("123:abc", 42, 1_700_000_000);
        assert!(!verify_telegram_login(
            &d,
            "123:abc",
            1_700_000_000 + 86_401
        ));
    }

    #[test]
    fn refuse_un_hash_bricole() {
        let mut d = signed("123:abc", 42, 1_700_000_000);
        d.id = 43;
        assert!(!verify_telegram_login(&d, "123:abc", 1_700_000_100));
    }
}

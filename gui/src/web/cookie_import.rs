//! Linux browser cookie import for cookie-authenticated providers.
//!
//! Chromium-family (Chrome/Chromium/Brave/Edge) store cookies in a SQLite
//! `Cookies` DB. Values are encrypted with AES-128-CBC; the key is PBKDF2(
//! password, "saltysalt", iterations=1, 16 bytes). The password ("Safe Storage"
//! key) comes from the desktop secret service (GNOME Keyring/KWallet) for v11
//! values, or the well-known fallback "peanuts" for v10. Firefox stores cookies
//! unencrypted in `cookies.sqlite`.
//!
//! We extract the cookies a provider needs and join them into an HTTP `Cookie:`
//! header string, which the engine accepts via the `cookieHeader` config field.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

const CHROMIUM_SALT: &[u8] = b"saltysalt";
const CHROMIUM_IV: [u8; 16] = [b' '; 16]; // 16 spaces
const FALLBACK_PASSWORD: &[u8] = b"peanuts";

/// A Chromium-family browser and where its profile lives.
pub struct ChromiumBrowser {
    pub name: &'static str,
    pub config_subdir: &'static str,
    /// Secret-service label for the Safe Storage key, e.g. "Chrome Safe Storage".
    pub keyring_label: &'static str,
}

pub const CHROMIUM_BROWSERS: &[ChromiumBrowser] = &[
    ChromiumBrowser { name: "Google Chrome", config_subdir: "google-chrome", keyring_label: "Chrome Safe Storage" },
    ChromiumBrowser { name: "Chromium", config_subdir: "chromium", keyring_label: "Chromium Safe Storage" },
    ChromiumBrowser { name: "Brave", config_subdir: "BraveSoftware/Brave-Browser", keyring_label: "Brave Safe Storage" },
    ChromiumBrowser { name: "Microsoft Edge", config_subdir: "microsoft-edge", keyring_label: "Microsoft Edge Safe Storage" },
];

#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub host: String,
}

/// Build a `name=value; name2=value2` Cookie header from decrypted cookies that
/// match any of `domains` (suffix match) and whose name is in `wanted` (or all
/// if `wanted` is empty).
pub fn cookie_header_for(
    domains: &[&str],
    wanted: &[&str],
    cookies: &[Cookie],
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for c in cookies {
        let host_ok = domains.iter().any(|d| {
            let d = d.trim_start_matches('.');
            c.host.trim_start_matches('.').ends_with(d)
        });
        if !host_ok {
            continue;
        }
        if !wanted.is_empty() && !wanted.iter().any(|w| *w == c.name) {
            continue;
        }
        if !c.value.is_empty() {
            parts.push(format!("{}={}", c.name, c.value));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// Read + decrypt all cookies from one Chromium-family browser's default profile.
pub fn import_chromium(browser: &ChromiumBrowser) -> Result<Vec<Cookie>> {
    let base = config_dir().join(browser.config_subdir);
    let cookies_db = locate_cookies_db(&base)
        .ok_or_else(|| anyhow!("no Cookies DB under {}", base.display()))?;

    let password = safe_storage_password(browser.keyring_label);
    let key = derive_key(&password);

    // Copy the DB to a temp path: the browser may hold a lock.
    let tmp = std::env::temp_dir().join(format!(
        "codexbar-cookies-{}.sqlite",
        std::process::id()
    ));
    std::fs::copy(&cookies_db, &tmp).context("copying Cookies DB")?;
    let result = read_chromium_db(&tmp, &key);
    let _ = std::fs::remove_file(&tmp);
    result
}

fn read_chromium_db(path: &Path, key: &[u8; 16]) -> Result<Vec<Cookie>> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .context("opening Cookies DB")?;

    let mut stmt = conn
        .prepare("SELECT host_key, name, value, encrypted_value FROM cookies")
        .context("querying cookies")?;

    let rows = stmt.query_map([], |row| {
        let host: String = row.get(0)?;
        let name: String = row.get(1)?;
        let plain: String = row.get(2)?;
        let enc: Vec<u8> = row.get(3).unwrap_or_default();
        Ok((host, name, plain, enc))
    })?;

    let mut cookies = Vec::new();
    for r in rows {
        let (host, name, plain, enc) = r?;
        let value = if !plain.is_empty() {
            plain
        } else {
            decrypt_value(&enc, key).unwrap_or_default()
        };
        cookies.push(Cookie { name, value, host });
    }
    Ok(cookies)
}

/// Decrypt a Chromium `encrypted_value` (v10/v11 = AES-128-CBC).
fn decrypt_value(enc: &[u8], key: &[u8; 16]) -> Result<String> {
    if enc.len() < 3 {
        return Err(anyhow!("encrypted value too short"));
    }
    let prefix = &enc[0..3];
    if prefix != b"v10" && prefix != b"v11" {
        // Unknown/plaintext scheme; return as-is.
        return Ok(String::from_utf8_lossy(enc).into_owned());
    }
    let ciphertext = &enc[3..];
    let mut buf = ciphertext.to_vec();
    let pt = Aes128CbcDec::new(key.into(), &CHROMIUM_IV.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| anyhow!("AES-CBC decrypt failed: {e}"))?;
    // Newer Chrome (v10 on some builds) prepends a 32-byte SHA256 domain hash.
    // Heuristic: if the result isn't valid UTF-8, retry skipping 32 bytes.
    match std::str::from_utf8(pt) {
        Ok(s) => Ok(s.to_string()),
        Err(_) if pt.len() > 32 => {
            Ok(String::from_utf8_lossy(&pt[32..]).into_owned())
        }
        Err(_) => Ok(String::from_utf8_lossy(pt).into_owned()),
    }
}

fn derive_key(password: &[u8]) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2::<hmac::Hmac<sha1::Sha1>>(password, CHROMIUM_SALT, 1, &mut key)
        .expect("pbkdf2 length valid");
    key
}

/// Fetch the browser's "Safe Storage" key from the secret service; fall back to
/// the well-known "peanuts" password used when no keyring is available.
fn safe_storage_password(label: &str) -> Vec<u8> {
    match read_secret_service(label) {
        Some(pw) if !pw.is_empty() => pw,
        _ => FALLBACK_PASSWORD.to_vec(),
    }
}

fn read_secret_service(label: &str) -> Option<Vec<u8>> {
    use secret_service::blocking::SecretService;
    use secret_service::EncryptionType;

    let ss = SecretService::connect(EncryptionType::Dh).ok()?;
    let collection = ss.get_default_collection().ok()?;
    let _ = collection.unlock();
    let items = collection.get_all_items().ok()?;
    for item in items {
        if let Ok(item_label) = item.get_label() {
            if item_label == label {
                if let Ok(secret) = item.get_secret() {
                    return Some(secret);
                }
            }
        }
    }
    None
}

fn locate_cookies_db(base: &Path) -> Option<PathBuf> {
    // Prefer the Default profile, else the first profile that has a Cookies DB.
    let default = base.join("Default/Cookies");
    if default.exists() {
        return Some(default);
    }
    let direct = base.join("Cookies");
    if direct.exists() {
        return Some(direct);
    }
    if let Ok(entries) = std::fs::read_dir(base) {
        for e in entries.flatten() {
            let c = e.path().join("Cookies");
            if c.exists() {
                return Some(c);
            }
        }
    }
    None
}

/// Firefox: cookies live unencrypted in cookies.sqlite.
pub fn import_firefox() -> Result<Vec<Cookie>> {
    let base = home().join(".mozilla/firefox");
    let profile = firefox_default_profile(&base)
        .ok_or_else(|| anyhow!("no Firefox default profile under {}", base.display()))?;
    let db = profile.join("cookies.sqlite");
    if !db.exists() {
        return Err(anyhow!("no cookies.sqlite in {}", profile.display()));
    }
    let tmp = std::env::temp_dir().join(format!("codexbar-ff-{}.sqlite", std::process::id()));
    std::fs::copy(&db, &tmp)?;
    let conn = rusqlite::Connection::open_with_flags(
        &tmp,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut stmt = conn.prepare("SELECT host, name, value FROM moz_cookies")?;
    let rows = stmt.query_map([], |row| {
        Ok(Cookie {
            host: row.get(0)?,
            name: row.get(1)?,
            value: row.get(2)?,
        })
    })?;
    let cookies: Vec<Cookie> = rows.filter_map(|r| r.ok()).collect();
    let _ = std::fs::remove_file(&tmp);
    Ok(cookies)
}

fn firefox_default_profile(base: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(base).ok()?;
    let mut fallback = None;
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".default-release") {
            return Some(e.path());
        }
        if name.ends_with(".default") {
            fallback = Some(e.path());
        }
    }
    fallback
}

fn config_dir() -> PathBuf {
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(x);
    }
    home().join(".config")
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_header_filters_by_domain_and_name() {
        let cookies = vec![
            Cookie { name: "WorkosCursorSessionToken".into(), value: "abc".into(), host: ".cursor.com".into() },
            Cookie { name: "other".into(), value: "x".into(), host: ".example.com".into() },
            Cookie { name: "empty".into(), value: "".into(), host: ".cursor.com".into() },
        ];
        let h = cookie_header_for(&["cursor.com"], &["WorkosCursorSessionToken"], &cookies).unwrap();
        assert_eq!(h, "WorkosCursorSessionToken=abc");
        // empty value skipped, wrong-domain skipped
        let all = cookie_header_for(&["cursor.com"], &[], &cookies).unwrap();
        assert_eq!(all, "WorkosCursorSessionToken=abc");
    }

    #[test]
    fn unmatched_domain_yields_none() {
        let cookies = vec![Cookie { name: "a".into(), value: "b".into(), host: ".foo.com".into() }];
        assert!(cookie_header_for(&["bar.com"], &[], &cookies).is_none());
    }

    #[test]
    fn key_derivation_is_stable() {
        let k1 = derive_key(b"peanuts");
        let k2 = derive_key(b"peanuts");
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 16);
    }
}

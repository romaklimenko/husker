//! Access tokens for the online server.
//!
//! A token is signed on the user's computer with an Ed25519 private key that
//! stays in `online.toml` in the profile. The online server holds only public
//! keys, so nothing in the cloud can create a token.

use std::{collections::HashSet, io::Write, path::Path};

use anyhow::{Context, Result, anyhow, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize};

/// Every token starts with this text. It names the token format.
pub const TOKEN_PREFIX: &str = "hsk1_";

/// The name of the key file in the profile directory.
pub const KEY_FILE: &str = "online.toml";

/// The longest lifetime a token may have.
pub const MAX_LIFETIME_DAYS: i64 = 400;

/// The lifetime `husker token` gives a token by default.
pub const DEFAULT_LIFETIME_DAYS: i64 = 365;

const SECONDS_PER_DAY: i64 = 86_400;

/// A token signed this far in the future is still accepted, to allow for clock differences.
const CLOCK_SKEW_SECONDS: i64 = 300;

/// The payload of a token.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    /// The token ID. A server setting can revoke it.
    pub id: String,
    pub name: String,
    /// The address of the online server the token is for.
    pub aud: String,
    /// The `uid` of the only accessible list, or `None` for every list.
    // Without `deserialize_with`, serde would accept a payload that omits the member.
    #[serde(deserialize_with = "Option::deserialize")]
    pub list: Option<String>,
    pub write: bool,
    /// Seconds since 1970-01-01 UTC.
    pub iat: i64,
    pub exp: i64,
}

/// Why the server refused a token. The client only ever sees status 401.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Denied {
    Malformed,
    Signature,
    Payload,
    Audience,
    Expired,
    NotYetValid,
    TooLong,
    Revoked,
}

/// Returns `bytes` random bytes as lowercase hexadecimal text.
pub fn random_hex(bytes: usize) -> Result<String> {
    let mut buffer = vec![0_u8; bytes];
    SystemRandom::new()
        .fill(&mut buffer)
        .map_err(|_| anyhow!("The system has no random numbers."))?;
    Ok(buffer.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Checks the public address of an online server and returns it in one form.
///
/// The address is a scheme, a host, and an optional port. `http` is accepted
/// only for this computer, because a token is sent with every request.
pub fn normalize_origin(address: &str) -> Result<String> {
    let address = address.trim();
    let address = address.strip_suffix('/').unwrap_or(address);
    let (scheme, authority) = address
        .split_once("://")
        .context("The address must start with https://")?;
    let scheme = scheme.to_lowercase();
    ensure!(
        !authority.is_empty()
            && !authority.contains(['/', '?', '#', '@'])
            && !authority.contains(char::is_whitespace),
        "The address must be a scheme and a host, without a path."
    );
    let authority = authority.to_lowercase();
    // An IPv6 host stands in brackets. A port, if any, follows the host after a colon.
    let (host, port) = match authority.strip_prefix('[') {
        Some(bracketed) => {
            let (host, rest) = bracketed
                .split_once(']')
                .context("The address has an unclosed bracket.")?;
            ensure!(
                rest.is_empty() || rest.starts_with(':'),
                "The address must be a scheme and a host, without a path."
            );
            (host, rest.strip_prefix(':'))
        }
        None => match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority.as_str(), None),
        },
    };
    ensure!(!host.is_empty(), "The address has no host name.");
    ensure!(
        port.is_none_or(|port| port.parse::<u16>().is_ok()),
        "The port in the address is not a number."
    );
    match scheme.as_str() {
        "https" => {}
        "http" => ensure!(
            matches!(host, "localhost" | "127.0.0.1" | "::1"),
            "The address must use https. http is accepted only for localhost."
        ),
        _ => bail!("The address must start with https://"),
    }
    Ok(format!("{scheme}://{authority}"))
}

/// The host and optional port of a normalized address.
pub fn origin_host(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, host)| host)
}

/// Signs tokens with the private key of a profile.
pub struct Signer {
    pair: Ed25519KeyPair,
}

impl Signer {
    /// Creates a new key. Returns the signer and the text to store in `online.toml`.
    pub fn generate() -> Result<(Self, String)> {
        let document = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
            .map_err(|_| anyhow!("Cannot create a key."))?;
        let encoded = URL_SAFE_NO_PAD.encode(document.as_ref());
        Ok((Self::from_encoded(&encoded)?, encoded))
    }

    /// Reads a key stored by `generate`. The error never repeats the key.
    pub fn from_encoded(encoded: &str) -> Result<Self> {
        let invalid = || anyhow!("key is not a valid private key.");
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .map_err(|_| invalid())?;
        let pair = Ed25519KeyPair::from_pkcs8(&bytes).map_err(|_| invalid())?;
        Ok(Self { pair })
    }

    /// The public key in the form the server setting `HUSKER_ONLINE_KEYS` holds.
    pub fn public_key(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.pair.public_key().as_ref())
    }

    pub fn sign(&self, claims: &Claims) -> String {
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("claims serialize"));
        let signed = format!("{TOKEN_PREFIX}{payload}");
        let signature = URL_SAFE_NO_PAD.encode(self.pair.sign(signed.as_bytes()).as_ref());
        format!("{signed}.{signature}")
    }

    /// Builds and signs a token that is valid from `now` for `days` days.
    pub fn mint(
        &self,
        origin: &str,
        name: &str,
        list: Option<String>,
        write: bool,
        days: i64,
        now: i64,
    ) -> Result<(Claims, String)> {
        let name = name.trim();
        ensure!(
            (1..=64).contains(&name.chars().count()),
            "The token name must have 1 to 64 characters."
        );
        ensure!(
            (1..=MAX_LIFETIME_DAYS).contains(&days),
            "The lifetime must be from 1 through {MAX_LIFETIME_DAYS} days."
        );
        let claims = Claims {
            id: random_hex(6)?,
            name: name.to_owned(),
            aud: origin.to_owned(),
            list,
            write,
            iat: now,
            exp: now + days * SECONDS_PER_DAY,
        };
        let token = self.sign(&claims);
        Ok((claims, token))
    }
}

/// Reports whether `id` has the form of a token ID: 12 lowercase hexadecimal characters.
fn is_token_id(id: &str) -> bool {
    id.len() == 12
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Reads the revoked token IDs of the server settings, separated by commas.
///
/// An entry that cannot be a token ID is an error. A misspelled ID would
/// otherwise revoke nothing, and nobody would notice.
pub fn parse_revoked(text: &str) -> Result<HashSet<String>> {
    text.split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| {
            ensure!(
                is_token_id(id),
                "A revoked token ID must be 12 lowercase hexadecimal characters. Separate IDs with commas."
            );
            Ok(id.to_owned())
        })
        .collect()
}

/// Checks tokens against the public keys of the server settings.
pub struct Verifier {
    keys: Vec<Vec<u8>>,
    origin: String,
    revoked: HashSet<String>,
}

impl Verifier {
    /// `keys` holds public keys separated by commas. `revoked` comes from `parse_revoked`.
    pub fn new(keys: &str, origin: &str, revoked: HashSet<String>) -> Result<Self> {
        let keys = keys
            .split(',')
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(|key| {
                let bytes = URL_SAFE_NO_PAD
                    .decode(key)
                    .ok()
                    .filter(|bytes| bytes.len() == 32)
                    .context("A public key must be 32 bytes in base64url without padding.")?;
                Ok(bytes)
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(!keys.is_empty(), "At least one public key is needed.");
        Ok(Self {
            keys,
            origin: origin.to_owned(),
            revoked,
        })
    }

    /// Checks a token at the time `now`, in seconds since 1970-01-01 UTC.
    ///
    /// The signature is checked before the payload is read as JSON.
    pub fn verify(&self, token: &str, now: i64) -> Result<Claims, Denied> {
        let rest = token.strip_prefix(TOKEN_PREFIX).ok_or(Denied::Malformed)?;
        let (payload, signature) = rest.split_once('.').ok_or(Denied::Malformed)?;
        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| Denied::Malformed)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| Denied::Malformed)?;
        let signed = &token.as_bytes()[..TOKEN_PREFIX.len() + payload.len()];
        let trusted = self.keys.iter().any(|key| {
            UnparsedPublicKey::new(&ED25519, key)
                .verify(signed, &signature)
                .is_ok()
        });
        if !trusted {
            return Err(Denied::Signature);
        }
        let claims: Claims = serde_json::from_slice(&payload_bytes).map_err(|_| Denied::Payload)?;
        if !is_token_id(&claims.id) || !(1..=64).contains(&claims.name.chars().count()) {
            return Err(Denied::Payload);
        }
        if claims.aud != self.origin {
            return Err(Denied::Audience);
        }
        if claims.exp <= now {
            return Err(Denied::Expired);
        }
        if claims.iat > now.saturating_add(CLOCK_SKEW_SECONDS) {
            return Err(Denied::NotYetValid);
        }
        if claims.exp.saturating_sub(claims.iat) > MAX_LIFETIME_DAYS * SECONDS_PER_DAY {
            return Err(Denied::TooLong);
        }
        if self.revoked.contains(&claims.id) {
            return Err(Denied::Revoked);
        }
        Ok(claims)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KeyFile {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

/// The private key and the server address of a profile, from `online.toml`.
pub struct OnlineProfile {
    signer: Signer,
    encoded_key: String,
    /// The public address of the online server, once the user gave it.
    pub url: Option<String>,
}

impl OnlineProfile {
    /// Reads `online.toml` from the profile, or creates it with a new key.
    ///
    /// Errors name the file and never repeat the key.
    pub fn open(profile: &Path) -> Result<Self> {
        let path = profile.join(KEY_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let (signer, encoded_key) = Signer::generate()?;
                let created = Self {
                    signer,
                    encoded_key,
                    url: None,
                };
                created.save(profile)?;
                return Ok(created);
            }
            Err(error) => {
                return Err(error).with_context(|| format!("Cannot read {}", path.display()));
            }
        };
        Self::parse(&text).with_context(|| format!("Invalid settings in {}", path.display()))
    }

    fn parse(text: &str) -> Result<Self> {
        // The TOML error can quote the offending line, which may hold the key.
        let file: KeyFile = toml::from_str(text)
            .map_err(|error| anyhow!("{}. Use the keys key and url.", error.message()))?;
        let url = file
            .url
            .as_deref()
            .map(normalize_origin)
            .transpose()
            .context("url")?;
        Ok(Self {
            signer: Signer::from_encoded(&file.key)?,
            encoded_key: file.key.trim().to_owned(),
            url,
        })
    }

    /// Stores the public address of the online server for later tokens.
    pub fn set_url(&mut self, profile: &Path, address: &str) -> Result<()> {
        let url = normalize_origin(address)?;
        if self.url.as_deref() != Some(url.as_str()) {
            self.url = Some(url);
            self.save(profile)?;
        }
        Ok(())
    }

    pub fn signer(&self) -> &Signer {
        &self.signer
    }

    /// Writes the file through a temporary file, so a crash leaves no half-written key.
    fn save(&self, profile: &Path) -> Result<()> {
        let path = profile.join(KEY_FILE);
        let text = toml::to_string(&KeyFile {
            key: self.encoded_key.clone(),
            url: self.url.clone(),
        })?;
        let mut file = tempfile::NamedTempFile::new_in(profile)
            .with_context(|| format!("Cannot write {}", path.display()))?;
        file.write_all(text.as_bytes())?;
        file.persist(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("Cannot write {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "https://tasks.example.net";
    const NOW: i64 = 1_800_000_000;

    fn signer() -> Signer {
        Signer::generate().unwrap().0
    }

    fn verifier(signer: &Signer) -> Verifier {
        Verifier::new(&signer.public_key(), ORIGIN, HashSet::new()).unwrap()
    }

    fn claims() -> Claims {
        Claims {
            id: "0123456789ab".into(),
            name: "test".into(),
            aud: ORIGIN.into(),
            list: None,
            write: true,
            iat: NOW,
            exp: NOW + 3600,
        }
    }

    #[test]
    fn a_signed_token_is_accepted_with_its_claims() {
        let signer = signer();
        let (minted, token) = signer
            .mint(ORIGIN, " phone ", Some("a".repeat(32)), false, 365, NOW)
            .unwrap();
        assert!(token.starts_with(TOKEN_PREFIX));
        assert_eq!(token.matches('.').count(), 1);
        let accepted = verifier(&signer).verify(&token, NOW + 10).unwrap();
        assert_eq!(accepted, minted);
        assert_eq!(accepted.name, "phone");
        assert_eq!(accepted.list.as_deref(), Some("a".repeat(32).as_str()));
        assert!(!accepted.write);
        assert_eq!(accepted.exp - accepted.iat, 365 * SECONDS_PER_DAY);
        assert_eq!(accepted.id.len(), 12);
    }

    #[test]
    fn each_refused_token_case_is_refused() {
        let signer = signer();
        let verifier = verifier(&signer);
        let good = signer.sign(&claims());
        assert!(verifier.verify(&good, NOW).is_ok());

        // Malformed text.
        for token in [
            "",
            "hsk2_abc.def",
            "hsk1_abc",
            "hsk1_!!!.abc",
            "hsk1_abc.!!!",
        ] {
            assert_eq!(
                verifier.verify(token, NOW),
                Err(Denied::Malformed),
                "{token}"
            );
        }

        // A signature from another key.
        let other = super::Signer::generate().unwrap().0.sign(&claims());
        assert_eq!(verifier.verify(&other, NOW), Err(Denied::Signature));

        // A changed payload with the old signature.
        let (_, signature) = good.split_once('.').unwrap();
        let mut changed = claims();
        changed.write = false;
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&changed).unwrap());
        let forged = format!("{TOKEN_PREFIX}{payload}.{signature}");
        assert_eq!(verifier.verify(&forged, NOW), Err(Denied::Signature));

        // Payloads that are signed but do not have exactly the seven members.
        let sign_raw = |payload: serde_json::Value| {
            let payload = URL_SAFE_NO_PAD.encode(payload.to_string());
            let signed = format!("{TOKEN_PREFIX}{payload}");
            let signature = URL_SAFE_NO_PAD.encode(signer.pair.sign(signed.as_bytes()).as_ref());
            format!("{signed}.{signature}")
        };
        let mut extra = serde_json::to_value(claims()).unwrap();
        extra["admin"] = true.into();
        assert_eq!(verifier.verify(&sign_raw(extra), NOW), Err(Denied::Payload));
        let mut missing = serde_json::to_value(claims()).unwrap();
        missing.as_object_mut().unwrap().remove("list");
        assert_eq!(
            verifier.verify(&sign_raw(missing), NOW),
            Err(Denied::Payload)
        );
        let mut bad_id = claims();
        bad_id.id = "XYZ".into();
        assert_eq!(
            verifier.verify(&signer.sign(&bad_id), NOW),
            Err(Denied::Payload)
        );
        assert_eq!(
            verifier.verify(&sign_raw(serde_json::json!("text")), NOW),
            Err(Denied::Payload)
        );

        let mut elsewhere = claims();
        elsewhere.aud = "https://other.example.net".into();
        assert_eq!(
            verifier.verify(&signer.sign(&elsewhere), NOW),
            Err(Denied::Audience)
        );

        assert_eq!(verifier.verify(&good, NOW + 3600), Err(Denied::Expired));

        let mut early = claims();
        early.iat = NOW + 301;
        early.exp = NOW + 4000;
        assert_eq!(
            verifier.verify(&signer.sign(&early), NOW),
            Err(Denied::NotYetValid)
        );
        early.iat = NOW + 300;
        assert!(verifier.verify(&signer.sign(&early), NOW).is_ok());

        let mut long = claims();
        long.exp = NOW + 400 * SECONDS_PER_DAY + 1;
        assert_eq!(
            verifier.verify(&signer.sign(&long), NOW),
            Err(Denied::TooLong)
        );
        long.exp = NOW + 400 * SECONDS_PER_DAY;
        assert!(verifier.verify(&signer.sign(&long), NOW).is_ok());

        let revoked = parse_revoked("ffffffffffff, 0123456789ab").unwrap();
        let revoking = Verifier::new(&signer.public_key(), ORIGIN, revoked).unwrap();
        assert_eq!(revoking.verify(&good, NOW), Err(Denied::Revoked));
    }

    #[test]
    fn a_misspelled_revocation_list_is_an_error() {
        assert!(parse_revoked("").unwrap().is_empty());
        assert_eq!(parse_revoked(" 0123456789ab ,, ").unwrap().len(), 1);
        // Each of these would revoke nothing if it were accepted.
        for text in [
            "0123456789ab ba9876543210",
            "0123456789AB",
            "0123456789ab;ba9876543210",
            "\"0123456789ab\"",
            "0123456789a",
            "0123456789abc",
        ] {
            assert!(parse_revoked(text).is_err(), "{text}");
        }
    }

    #[test]
    fn any_configured_key_accepts_its_tokens() {
        let first = signer();
        let second = signer();
        let keys = format!("{}, {}", first.public_key(), second.public_key());
        let verifier = Verifier::new(&keys, ORIGIN, HashSet::new()).unwrap();
        assert!(verifier.verify(&first.sign(&claims()), NOW).is_ok());
        assert!(verifier.verify(&second.sign(&claims()), NOW).is_ok());
        assert_eq!(
            verifier.verify(&signer().sign(&claims()), NOW),
            Err(Denied::Signature)
        );
    }

    #[test]
    fn keys_and_lifetimes_are_validated() {
        assert!(Verifier::new("", ORIGIN, HashSet::new()).is_err());
        assert!(Verifier::new("not-a-key", ORIGIN, HashSet::new()).is_err());
        let signer = signer();
        assert_eq!(signer.public_key().len(), 43);
        for days in [0, 401] {
            assert!(signer.mint(ORIGIN, "x", None, true, days, NOW).is_err());
        }
        assert!(signer.mint(ORIGIN, " ", None, true, 1, NOW).is_err());
        assert!(
            signer
                .mint(ORIGIN, &"n".repeat(65), None, true, 1, NOW)
                .is_err()
        );
        assert!(signer.mint(ORIGIN, "x", None, true, 400, NOW).is_ok());
    }

    #[test]
    fn addresses_are_normalized_and_checked() {
        assert_eq!(
            normalize_origin(" HTTPS://Tasks.Example.net/ ").unwrap(),
            "https://tasks.example.net"
        );
        assert_eq!(
            normalize_origin("http://127.0.0.1:8787").unwrap(),
            "http://127.0.0.1:8787"
        );
        assert_eq!(
            origin_host("https://tasks.example.net:444"),
            "tasks.example.net:444"
        );
        assert_eq!(
            normalize_origin("http://[::1]:8787").unwrap(),
            "http://[::1]:8787"
        );
        for address in [
            "https://tasks.example.net:abc",
            "https://tasks.example.net:99999",
            "https://[::1",
            "https://[::1]x",
            "tasks.example.net",
            "ftp://tasks.example.net",
            "https://",
            "https://tasks.example.net/path",
            "https://tasks.example.net?x=1",
            "https://user@tasks.example.net",
            "http://tasks.example.net",
        ] {
            assert!(normalize_origin(address).is_err(), "{address}");
        }
    }

    #[test]
    fn the_key_file_is_created_once_and_keeps_the_address() {
        let profile = tempfile::tempdir().unwrap();
        let mut created = OnlineProfile::open(profile.path()).unwrap();
        let public_key = created.signer().public_key();
        assert!(created.url.is_none());
        created
            .set_url(profile.path(), "https://Tasks.example.net/")
            .unwrap();

        let reopened = OnlineProfile::open(profile.path()).unwrap();
        assert_eq!(reopened.signer().public_key(), public_key);
        assert_eq!(reopened.url.as_deref(), Some("https://tasks.example.net"));
    }

    #[test]
    fn key_file_errors_name_the_file_and_hide_the_key() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join(KEY_FILE);
        for text in [
            "key = \"S3CRETKEY\"\n",
            "key = \"S3CRETKEY\"\nother = 1\n",
            "key = \"S3CRETKEY\nurl = 1",
        ] {
            std::fs::write(&path, text).unwrap();
            let error = format!("{:#}", OnlineProfile::open(profile.path()).err().unwrap());
            assert!(error.contains(KEY_FILE), "{error}");
            assert!(!error.contains("S3CRETKEY"), "{error}");
        }
    }
}

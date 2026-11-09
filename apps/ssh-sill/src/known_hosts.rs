//! Parsing and serialising OpenSSH `known_hosts` files.
//!
//! Entry format (see `sshd(8)`, "SSH_KNOWN_HOSTS FILE FORMAT"):
//!
//! ```text
//! [@cert-authority|@revoked] host1,[host2]:2222 key-type base64-key [comment]
//! ```

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    CertAuthority,
    Revoked,
}

impl Marker {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "@cert-authority" => Some(Self::CertAuthority),
            "@revoked" => Some(Self::Revoked),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CertAuthority => "@cert-authority",
            Self::Revoked => "@revoked",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub marker: Option<Marker>,
    /// Host patterns, e.g. `github.com`, `[10.0.0.1]:2222` or a hashed `|1|salt|hash`.
    pub hosts: Vec<String>,
    pub key_type: String,
    /// Base64-encoded public key blob.
    pub key: String,
    pub comment: String,
    /// The original line, written back unchanged on save.
    raw: String,
}

impl Entry {
    /// Build a new entry, validating that the key is base64 and its embedded
    /// type matches `key_type`.
    pub fn new(
        hosts: &str,
        key_type: &str,
        key: &str,
        comment: &str,
        marker: Option<Marker>,
    ) -> Result<Self, String> {
        let hosts: Vec<String> = hosts
            .split(',')
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map(str::to_owned)
            .collect();
        if hosts.is_empty() {
            return Err("At least one host is required".into());
        }
        if hosts.iter().any(|h| h.contains(char::is_whitespace)) {
            return Err("Hosts must not contain spaces (separate them with commas)".into());
        }
        let key_type = key_type.trim();
        if key_type.is_empty() || key_type.contains(char::is_whitespace) {
            return Err("Key type is required".into());
        }
        let key = key.trim();
        let blob = STANDARD
            .decode(key)
            .map_err(|_| "Key is not valid base64".to_owned())?;
        match blob_key_type(&blob) {
            Some(t) if t == key_type => {}
            Some(t) => return Err(format!("Key is a {t} key, not {key_type}")),
            None => return Err("Key is not an SSH public key".into()),
        }
        let comment = comment.trim().to_owned();

        let mut parts: Vec<&str> = Vec::new();
        if let Some(m) = marker {
            parts.push(m.as_str());
        }
        let joined = hosts.join(",");
        parts.extend([joined.as_str(), key_type, key]);
        if !comment.is_empty() {
            parts.push(&comment);
        }
        let raw = parts.join(" ");

        Ok(Self {
            marker,
            hosts,
            key_type: key_type.to_owned(),
            key: key.to_owned(),
            comment,
            raw,
        })
    }

    /// Build an entry from a full `ssh-keyscan` / `*.pub` style line:
    /// `[marker] hosts key-type key [comment]`.
    pub fn parse(line: &str) -> Option<Self> {
        let mut tokens = line.split_whitespace();
        let mut first = tokens.next()?;
        let marker = if first.starts_with('@') {
            let m = Marker::parse(first)?;
            first = tokens.next()?;
            Some(m)
        } else {
            None
        };
        let key_type = tokens.next()?;
        let key = tokens.next()?;
        let comment = tokens.collect::<Vec<_>>().join(" ");
        Some(Self {
            marker,
            hosts: first.split(',').map(str::to_owned).collect(),
            key_type: key_type.to_owned(),
            key: key.to_owned(),
            comment,
            raw: line.to_owned(),
        })
    }

    pub fn is_hashed(&self) -> bool {
        self.hosts.iter().any(|h| h.starts_with("|1|"))
    }

    /// Human-readable host list; hashed hosts can't be reversed, so say so.
    pub fn hosts_display(&self) -> String {
        self.hosts
            .iter()
            .map(|h| {
                if h.starts_with("|1|") {
                    "(hashed)"
                } else {
                    h.as_str()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// `SHA256:…` fingerprint, matching `ssh-keygen -lf`.
    pub fn fingerprint(&self) -> Option<String> {
        let blob = STANDARD.decode(&self.key).ok()?;
        let digest = Sha256::digest(&blob);
        Some(format!("SHA256:{}", STANDARD_NO_PAD.encode(digest)))
    }
}

/// Read the key type string embedded at the start of an SSH public key blob
/// (a big-endian u32 length followed by that many bytes).
fn blob_key_type(blob: &[u8]) -> Option<&str> {
    let len = u32::from_be_bytes(blob.get(..4)?.try_into().ok()?) as usize;
    std::str::from_utf8(blob.get(4..4 + len)?).ok()
}

#[derive(Clone, Debug)]
pub enum Line {
    Entry(Entry),
    /// Blank lines, comments and unparseable lines, kept verbatim.
    Other(String),
}

#[derive(Clone, Debug, Default)]
pub struct KnownHosts {
    pub lines: Vec<Line>,
    crlf: bool,
}

impl KnownHosts {
    pub fn parse(text: &str) -> Self {
        let lines = text
            .lines()
            .map(|l| {
                let trimmed = l.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return Line::Other(l.to_owned());
                }
                match Entry::parse(l) {
                    Some(e) => Line::Entry(e),
                    None => Line::Other(l.to_owned()),
                }
            })
            .collect();
        Self {
            lines,
            crlf: text.contains("\r\n"),
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("Could not read file: {e}"))?;
        Ok(Self::parse(&text))
    }

    pub fn serialize(&self) -> String {
        let eol = if self.crlf { "\r\n" } else { "\n" };
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(match line {
                Line::Entry(e) => &e.raw,
                Line::Other(s) => s,
            });
            out.push_str(eol);
        }
        out
    }

    /// Write via a sibling temp file and rename, so a failed write never leaves a truncated known_hosts behind.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".ssh-sill.tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::write(&tmp, self.serialize()).map_err(|e| format!("Could not write file: {e}"))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Could not replace file: {e}")
        })
    }

    pub fn entries(&self) -> impl Iterator<Item = (usize, &Entry)> {
        self.lines.iter().enumerate().filter_map(|(i, l)| match l {
            Line::Entry(e) => Some((i, e)),
            Line::Other(_) => None,
        })
    }

    pub fn push(&mut self, entry: Entry) {
        self.lines.push(Line::Entry(entry));
    }

    /// Remove the line at `index` (an index from [`entries`](Self::entries)).
    pub fn remove(&mut self, index: usize) {
        if index < self.lines.len() {
            self.lines.remove(index);
        }
    }
}

/// `~/.ssh/known_hosts`, if a home directory can be found.
pub fn default_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".ssh").join("known_hosts"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // github.com's published ed25519 host key.
    const GH_KEY: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";

    fn sample() -> String {
        format!(
            "# my hosts\n\
             github.com ssh-ed25519 {GH_KEY}\n\
             \n\
             @cert-authority *.example.com,[10.0.0.1]:2222 ssh-ed25519 {GH_KEY} ca key\n\
             |1|abc=|def= ssh-ed25519 {GH_KEY}\n\
             garbage\n"
        )
    }

    #[test]
    fn round_trips_unchanged() {
        let text = sample();
        assert_eq!(KnownHosts::parse(&text).serialize(), text);
        let crlf = text.replace('\n', "\r\n");
        assert_eq!(KnownHosts::parse(&crlf).serialize(), crlf);
    }

    #[test]
    fn parses_entries() {
        let kh = KnownHosts::parse(&sample());
        let entries: Vec<_> = kh.entries().map(|(_, e)| e).collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].hosts, ["github.com"]);
        assert_eq!(entries[1].marker, Some(Marker::CertAuthority));
        assert_eq!(entries[1].hosts, ["*.example.com", "[10.0.0.1]:2222"]);
        assert_eq!(entries[1].comment, "ca key");
        assert!(entries[2].is_hashed());
        assert_eq!(entries[2].hosts_display(), "(hashed)");
    }

    #[test]
    fn fingerprint_matches_ssh_keygen() {
        let kh = KnownHosts::parse(&sample());
        let (_, e) = kh.entries().next().unwrap();
        assert_eq!(
            e.fingerprint().unwrap(),
            "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU"
        );
    }

    #[test]
    fn add_and_remove() {
        let mut kh = KnownHosts::parse(&sample());
        let e = Entry::new("a.test, b.test", "ssh-ed25519", GH_KEY, "new", None).unwrap();
        kh.push(e);
        assert!(
            kh.serialize()
                .ends_with(&format!("a.test,b.test ssh-ed25519 {GH_KEY} new\n"))
        );

        let (first, _) = kh.entries().next().unwrap();
        kh.remove(first);
        assert!(!kh.serialize().contains("github.com"));
        assert!(kh.serialize().starts_with("# my hosts\n"));
    }

    #[test]
    fn rejects_bad_new_entries() {
        assert!(Entry::new("", "ssh-ed25519", GH_KEY, "", None).is_err());
        assert!(Entry::new("h", "ssh-ed25519", "not base64!", "", None).is_err());
        assert!(Entry::new("h", "ssh-rsa", GH_KEY, "", None).is_err());
    }
}

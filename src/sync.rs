//! Encrypted sync, the part that needs no UI and no network.
//!
//! The data worth carrying between machines (history, workspace names, pinned
//! tabs, ...) is gathered into one [`Bundle`], padded, and sealed with a
//! passphrase using `age` (scrypt + ChaCha20-Poly1305). The result is a plain
//! `age` file: anything that can run `age -d` can open it, with or without rail.
//! Cookies and other sessions are deliberately never part of it.
//!
//! Merging is done on plaintext after decryption: history is a union of visits,
//! everything else is last-writer-wins by the bundle's timestamp.

use age::secrecy::SecretString;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// The files in the data directory that sync carries. Order is irrelevant.
pub const SYNCED: &[&str] = &["history.tsv", "workspaces.txt", "pinned.txt", "nokeys.txt"];
const HISTORY: &str = "history.tsv";
const MAGIC: &str = "rail-sync 1";
/// Ciphertext length hides the exact amount of data: plaintext is padded to a
/// multiple of this (so a repo only reveals roughly how much history exists).
const PAD_TO: usize = 16 * 1024;
const WORK_FACTOR: u8 = 18; // ~0.5 s per unlock; age's own default for passphrases

#[derive(Debug)]
pub enum Error {
    /// Wrong passphrase, or the file was altered.
    Decrypt,
    /// Decrypted fine but is not a rail bundle.
    Format(&'static str),
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Decrypt => f.write_str("wrong passphrase, or the data was tampered with"),
            Error::Format(why) => write!(f, "not a rail bundle ({why})"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// A snapshot of the synced files at one moment.
#[derive(Debug, Default, PartialEq)]
pub struct Bundle {
    /// Unix seconds when this snapshot was taken; the newer bundle wins ties.
    pub stamp: u64,
    pub files: BTreeMap<String, String>,
}

impl Bundle {
    /// Read the synced files out of the data directory (missing ones are skipped).
    /// The stamp is when the user last edited anything but history (which merges as a
    /// union and needs no stamp): the newest file modification time.
    pub fn collect(dir: &Path) -> Bundle {
        let mut files = BTreeMap::new();
        let mut stamp = 0;
        for name in SYNCED {
            let path = dir.join(name);
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            files.insert((*name).to_string(), text);
            if *name != HISTORY {
                let m = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                let secs = m.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                stamp = stamp.max(secs);
            }
        }
        Bundle { stamp, files }
    }

    /// Write the bundle's files into the data directory. Files that already match
    /// are left alone; the rest get the bundle's stamp as their modification time,
    /// so applying someone else's edit does not make it look like a fresh local one.
    pub fn apply(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for (name, text) in &self.files {
            // Never trust names that came out of a file: only our own list is written.
            if !SYNCED.contains(&name.as_str()) || std::fs::read_to_string(dir.join(name)).is_ok_and(|t| t == *text) {
                continue;
            }
            let tmp = dir.join(format!("{name}.tmp"));
            std::fs::write(&tmp, text)?;
            if self.stamp > 0 {
                let f = std::fs::File::options().write(true).open(&tmp)?;
                f.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(self.stamp))?;
            }
            std::fs::rename(&tmp, dir.join(name))?;
        }
        Ok(())
    }

    /// Combine two snapshots: history is the union of both, every other file
    /// comes from whichever snapshot is newer (and only exists if it does there).
    pub fn merge(&self, other: &Bundle) -> Bundle {
        let (newer, older) = if other.stamp > self.stamp { (other, self) } else { (self, other) };
        let mut files = newer.files.clone();
        let hist = merge_history(
            older.files.get(HISTORY).map_or("", String::as_str),
            newer.files.get(HISTORY).map_or("", String::as_str),
        );
        if !hist.is_empty() {
            files.insert(HISTORY.to_string(), hist);
        }
        Bundle { stamp: newer.stamp, files }
    }

    /// `rail-sync 1 <stamp>`, then per file `file <name> <len>` + bytes, then `end`,
    /// then zero padding up to the next multiple of [`PAD_TO`].
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = format!("{MAGIC} {}\n", self.stamp).into_bytes();
        for (name, text) in &self.files {
            out.extend_from_slice(format!("file {name} {}\n", text.len()).as_bytes());
            out.extend_from_slice(text.as_bytes());
            out.push(b'\n');
        }
        out.extend_from_slice(b"end\n");
        out.resize(out.len().div_ceil(PAD_TO) * PAD_TO, 0);
        out
    }

    fn from_bytes(mut data: &[u8]) -> Result<Bundle, Error> {
        fn line<'a>(data: &mut &'a [u8]) -> Result<&'a str, Error> {
            let end = data.iter().position(|&b| b == b'\n').ok_or(Error::Format("truncated"))?;
            let (l, rest) = data.split_at(end);
            *data = &rest[1..];
            std::str::from_utf8(l).map_err(|_| Error::Format("not text"))
        }
        let head = line(&mut data)?;
        let stamp = head
            .strip_prefix(MAGIC)
            .and_then(|r| r.trim().parse().ok())
            .ok_or(Error::Format("unknown version"))?;
        let mut files = BTreeMap::new();
        loop {
            let l = line(&mut data)?;
            if l == "end" {
                break;
            }
            let mut p = l.splitn(3, ' ');
            let (Some("file"), Some(name), Some(len)) = (p.next(), p.next(), p.next()) else {
                return Err(Error::Format("bad entry"));
            };
            let len: usize = len.parse().map_err(|_| Error::Format("bad length"))?;
            if data.len() < len + 1 {
                return Err(Error::Format("truncated"));
            }
            let text = std::str::from_utf8(&data[..len]).map_err(|_| Error::Format("not text"))?;
            files.insert(name.to_string(), text.to_string());
            data = &data[len + 1..];
        }
        Ok(Bundle { stamp, files })
    }
}

/// Union of two history logs. A visit is identified by `(time, url)`, so the same
/// visit seen on two machines (or synced twice) is stored once. Oldest first.
pub fn merge_history(a: &str, b: &str) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let mut rows: Vec<(u64, &str, &str)> = Vec::new();
    for line in a.lines().chain(b.lines()) {
        let mut p = line.splitn(2, '\t');
        let (Some(t), Some(rest)) = (p.next(), p.next()) else { continue };
        let Ok(t) = t.parse::<u64>() else { continue };
        let url = rest.split('\t').next().unwrap_or("");
        if seen.insert((t, url)) {
            rows.push((t, url, line));
        }
    }
    rows.sort_by_key(|r| (r.0, r.1));
    let mut out = String::new();
    for (_, _, line) in rows {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn seal(bundle: &Bundle, passphrase: &str, work_factor: u8) -> Result<Vec<u8>, Error> {
    let mut r = age::scrypt::Recipient::new(SecretString::from(passphrase.to_string()));
    r.set_work_factor(work_factor);
    let enc = age::Encryptor::with_recipients(std::iter::once(&r as &dyn age::Recipient))
        .map_err(|_| Error::Format("no recipient"))?;
    let mut out = Vec::new();
    let mut w = enc.wrap_output(&mut out)?;
    w.write_all(&bundle.to_bytes())?;
    w.finish()?;
    Ok(out)
}

/// Encrypt a bundle for storage anywhere untrusted (a git remote, a USB stick).
pub fn encrypt(bundle: &Bundle, passphrase: &str) -> Result<Vec<u8>, Error> {
    seal(bundle, passphrase, WORK_FACTOR)
}

/// Open what [`encrypt`] made. Any failure to authenticate is [`Error::Decrypt`].
pub fn decrypt(data: &[u8], passphrase: &str) -> Result<Bundle, Error> {
    let d = age::Decryptor::new(data).map_err(|_| Error::Decrypt)?;
    let id = age::scrypt::Identity::new(SecretString::from(passphrase.to_string()));
    let mut r = d.decrypt(std::iter::once(&id as &dyn age::Identity)).map_err(|_| Error::Decrypt)?;
    let mut plain = Vec::new();
    r.read_to_end(&mut plain).map_err(|_| Error::Decrypt)?;
    Bundle::from_bytes(&plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Bundle {
        let mut files = BTreeMap::new();
        files.insert("history.tsv".to_string(), "100\thttps://a.com/\tA\n".to_string());
        files.insert("workspaces.txt".to_string(), "Work\nHome".to_string());
        Bundle { stamp: 1000, files }
    }

    #[test]
    fn round_trip() {
        let b = sample();
        let blob = seal(&b, "correct horse", 10).unwrap();
        assert_eq!(decrypt(&blob, "correct horse").unwrap(), b);
    }

    #[test]
    fn wrong_passphrase_and_tampering_fail() {
        let mut blob = seal(&sample(), "right", 10).unwrap();
        assert!(matches!(decrypt(&blob, "wrong"), Err(Error::Decrypt)));
        let last = blob.len() - 1;
        blob[last] ^= 1;
        assert!(matches!(decrypt(&blob, "right"), Err(Error::Decrypt)));
    }

    #[test]
    fn size_is_bucketed_and_content_is_hidden() {
        let small = seal(&sample(), "p", 10).unwrap();
        let mut b = sample();
        b.files.insert("pinned.txt".into(), "x".repeat(5000));
        let bigger = seal(&b, "p", 10).unwrap();
        assert_eq!(small.len(), bigger.len());
        assert!(!small.windows(5).any(|w| w == b"a.com"));
    }

    #[test]
    fn history_merges_as_a_union() {
        let a = "100\thttps://a.com/\tA\n300\thttps://c.com/\tC\n";
        let b = "100\thttps://a.com/\tA\n200\thttps://b.com/\tB\n";
        assert_eq!(
            merge_history(a, b),
            "100\thttps://a.com/\tA\n200\thttps://b.com/\tB\n300\thttps://c.com/\tC\n"
        );
        assert_eq!(merge_history(a, a), merge_history(a, ""));
    }

    #[test]
    fn newer_bundle_wins_other_files() {
        let mut old = sample();
        let mut new = sample();
        new.stamp = 2000;
        new.files.insert("workspaces.txt".into(), "Only".into());
        new.files.insert("history.tsv".into(), "200\thttps://b.com/\tB\n".into());
        let m = old.merge(&new);
        assert_eq!(m.files["workspaces.txt"], "Only");
        assert_eq!(m.files["history.tsv"].lines().count(), 2);
        old.stamp = 3000; // merge is symmetric in what it keeps
        assert_eq!(old.merge(&new).files["workspaces.txt"], "Work\nHome");
    }

    #[test]
    fn collect_and_apply() {
        let dir = std::env::temp_dir().join(format!("rail-sync-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        sample().apply(&dir).unwrap();
        let got = Bundle::collect(&dir);
        assert_eq!(got.files, sample().files);
        assert_eq!(got.stamp, 1000); // the stamp survives a trip through the disk
        let _ = std::fs::remove_dir_all(&dir);
    }
}

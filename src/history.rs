//! Browsing history (for address autocomplete) and URL helpers.
//!
//! History is an append-only log, `history.tsv`: `unix_time \t url \t title`.
//! It is folded into one entry per URL when loaded.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Entry {
    pub url: String,
    pub title: String,
    pub visits: u32,
    pub last: u64,
}

pub struct History {
    entries: Vec<Entry>,
    file: PathBuf,
}

/// An inline completion: `shown` is what the entry displays (typed text plus the
/// completed tail), `target` is the URL that Enter should open.
pub struct Completion {
    pub shown: String,
    pub target: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn clean(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().to_string()
}

/// `https://www.github.com/a/b` -> `github.com/a/b`
pub fn strip(url: &str) -> &str {
    let s = url.split_once("://").map_or(url, |x| x.1);
    s.strip_prefix("www.").unwrap_or(s)
}

/// `https://user@www.github.com:8080/a` -> `www.github.com:8080`
fn authority(url: &str) -> &str {
    let s = url.split_once("://").map_or(url, |x| x.1);
    let a = s.split(['/', '?', '#']).next().unwrap_or(s);
    a.rsplit_once('@').map_or(a, |x| x.1)
}

/// The site a URL belongs to: `app.example.com`, `example.com` and
/// `example.com/gg` all share the key `example.com`. Not http(s) -> `None`.
pub fn domain_key(url: &str) -> Option<String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let auth = authority(url).to_lowercase();
    let host = if auth.starts_with('[') { auth.clone() } else { auth.split(':').next().unwrap_or(&auth).to_string() };
    let labels: Vec<&str> = host.split('.').collect();
    let is_ip = host.starts_with('[') || labels.iter().all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()));
    if is_ip || labels.len() < 3 {
        return Some(host.trim_start_matches("www.").to_string());
    }
    // Without a public-suffix list: treat `co.uk`, `com.au`, ... as a two-part suffix.
    let n = labels.len();
    let (sld, tld) = (labels[n - 2], labels[n - 1]);
    let take = if tld.len() == 2 && matches!(sld, "co" | "com" | "org" | "net" | "gov" | "edu" | "ac" | "or" | "ne") { 3 } else { 2 };
    Some(labels[n - take..].join("."))
}

impl History {
    pub fn load(file: PathBuf) -> Self {
        let mut h = History { entries: Vec::new(), file };
        let text = std::fs::read_to_string(&h.file).unwrap_or_default();
        for line in text.lines() {
            let mut p = line.splitn(3, '\t');
            let (Some(t), Some(url), title) = (p.next(), p.next(), p.next().unwrap_or("")) else { continue };
            h.fold(url, title, t.parse().unwrap_or(0));
        }
        h
    }

    fn fold(&mut self, url: &str, title: &str, t: u64) {
        match self.entries.iter_mut().find(|e| e.url == url) {
            Some(e) => {
                e.visits += 1;
                e.last = e.last.max(t);
                if !title.is_empty() {
                    e.title = title.to_string();
                }
            }
            None => self.entries.push(Entry { url: url.into(), title: title.into(), visits: 1, last: t }),
        }
    }

    pub fn visit(&mut self, url: &str, title: &str) {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return;
        }
        let url = url.split('#').next().unwrap_or(url);
        let title = clean(title);
        let t = now();
        self.fold(url, &title, t);
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.file) {
            let _ = writeln!(f, "{t}\t{url}\t{title}");
        }
    }

    /// Frequent and recent beats merely old: visits count, but fade with age.
    fn score(e: &Entry, now: u64) -> f64 {
        let days = now.saturating_sub(e.last) as f64 / 86_400.0;
        e.visits.min(50) as f64 * (0.2 + 1.0 / (1.0 + days / 14.0))
    }

    /// Best matches for what has been typed. Empty query -> the most used sites.
    pub fn suggest(&self, query: &str, n: usize) -> Vec<&Entry> {
        let q = query.trim().to_lowercase();
        let t = now();
        let tokens: Vec<&str> = q.split_whitespace().collect();
        let mut hits: Vec<(f64, &Entry)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let mut s = Self::score(e, t);
                if !tokens.is_empty() {
                    let url = strip(&e.url).to_lowercase();
                    let hay = format!("{url} {}", e.title.to_lowercase());
                    if !tokens.iter().all(|k| hay.contains(k)) {
                        return None;
                    }
                    if url.starts_with(tokens[0]) {
                        s *= 3.0;
                    }
                } else if e.url.matches('/').count() > 3 {
                    s *= 0.3; // for an empty query prefer front pages over deep links
                }
                Some((s, e))
            })
            .collect();
        hits.sort_by(|a, b| b.0.total_cmp(&a.0));
        hits.into_iter().take(n).map(|x| x.1).collect()
    }

    /// Inline completion for `typed`: `git` -> `github.com`, `github.com/rust` ->
    /// `github.com/rust-lang/rust`. Hosts are completed as a whole (summed over
    /// all their pages); once a `/` is typed it completes to a full address.
    pub fn complete(&self, typed: &str) -> Option<Completion> {
        if typed.is_empty() || typed.contains(char::is_whitespace) {
            return None;
        }
        let lower = typed.to_lowercase();
        let t = now();

        if lower.contains("://") {
            let best = self
                .entries
                .iter()
                .filter(|e| e.url.to_lowercase().starts_with(&lower))
                .max_by(|a, b| Self::score(a, t).total_cmp(&Self::score(b, t)))?;
            let tail = best.url.get(typed.len()..)?;
            return Some(Completion { shown: format!("{typed}{tail}"), target: best.url.clone() });
        }

        let q = lower.strip_prefix("www.").unwrap_or(&lower);
        if q.is_empty() {
            return None;
        }
        if !q.contains('/') {
            let mut by_host: Vec<(String, f64)> = Vec::new();
            for e in &self.entries {
                let host = strip(authority(&e.url)).to_lowercase();
                if !host.starts_with(q) {
                    continue;
                }
                let s = Self::score(e, t);
                match by_host.iter_mut().find(|h| h.0 == host) {
                    Some(h) => h.1 += s,
                    None => by_host.push((host.clone(), s)),
                }
            }
            let (host, _) = by_host.iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
            let tail = host.get(q.len()..)?;
            // Open the origin the user actually visited (it may need `www.` or `http`).
            let rep = self.entries.iter().filter(|e| strip(authority(&e.url)).to_lowercase() == *host)
                .max_by(|a, b| Self::score(a, t).total_cmp(&Self::score(b, t)))?;
            let scheme = rep.url.split_once("://").map_or("https", |x| x.0);
            return Some(Completion {
                shown: format!("{typed}{tail}"),
                target: format!("{scheme}://{}", authority(&rep.url)),
            });
        }

        let best = self
            .entries
            .iter()
            .filter(|e| strip(&e.url).to_lowercase().starts_with(q))
            .max_by(|a, b| Self::score(a, t).total_cmp(&Self::score(b, t)))?;
        let tail = strip(&best.url).get(q.len()..)?;
        Some(Completion { shown: format!("{typed}{tail}"), target: best.url.clone() })
    }
}

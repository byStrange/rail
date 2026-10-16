//! `rail sync`: the command line around [`crate::sync`], in the spirit of gopass.
//!
//! The sync repository is an ordinary git clone that lives in `<data dir>/sync`.
//! Look inside it, `rail sync git log`, or `cd` there and use git yourself. It holds
//! one encrypted file per device (`<device>.rail`) and nothing else, so two devices
//! never write the same file and a push can only ever conflict with itself.
//!
//!   rail sync init <git-url>   set up (clones/creates the repo, asks for a passphrase)
//!   rail sync                  pull, merge, push
//!   rail sync status | path | git <args...>

use super::*;
use crate::sync::{self, Bundle};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

const BRANCH: &str = "main";

fn repo_dir() -> PathBuf {
    data_dir().join("sync")
}

/// A random-looking id, made once per installation, naming this device's file.
fn device_id() -> String {
    let file = data_dir().join("device-id");
    if let Ok(id) = std::fs::read_to_string(&file) {
        if !id.trim().is_empty() {
            return id.trim().to_string();
        }
    }
    let mut bytes = [0u8; 6];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = std::io::Read::read_exact(&mut f, &mut bytes);
    }
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let _ = std::fs::write(&file, &id);
    id
}

/// Run git in the sync repo. Commits are made as "rail" so no git identity is needed.
fn git(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(["-c", "user.name=rail", "-c", "user.email=rail@localhost"])
        .args(args)
        .current_dir(repo_dir())
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("git {}: {}", args.join(" "), err.trim()))
    }
}

// ---- passphrase ----------------------------------------------------------

fn keyring(args: &[&str], input: Option<&str>) -> Option<String> {
    let mut c = Command::new("secret-tool").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    if let Some(mut stdin) = c.stdin.take() {
        if let Some(i) = input {
            let _ = stdin.write_all(i.as_bytes());
        }
    }
    let out = c.wait_with_output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

fn prompt(msg: &str) -> String {
    eprint!("{msg}");
    let _ = std::io::stderr().flush();
    let hidden = Command::new("stty").arg("-echo").status().is_ok_and(|s| s.success());
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    if hidden {
        let _ = Command::new("stty").arg("echo").status();
        eprintln!();
    }
    line.trim_end_matches(['\n', '\r']).to_string()
}

/// `RAIL_SYNC_PASSPHRASE`, then the desktop keyring (if remembered), then ask.
/// The passphrase is never written to disk by rail.
fn passphrase(confirm: bool) -> Result<String, String> {
    if let Ok(p) = std::env::var("RAIL_SYNC_PASSPHRASE") {
        return Ok(p);
    }
    if let Some(p) = keyring(&["lookup", "application", "rail-sync"], None).filter(|p| !p.is_empty()) {
        return Ok(p);
    }
    let p = prompt("Sync passphrase: ");
    if p.is_empty() {
        return Err("empty passphrase".into());
    }
    if confirm && prompt("Repeat it (there is NO way to recover it): ") != p {
        return Err("passphrases do not match".into());
    }
    Ok(p)
}

// ---- the repo ------------------------------------------------------------

fn initialised() -> bool {
    repo_dir().join(".git").exists()
}

/// Every `*.rail` file in the repo, as (device id, bytes).
fn device_files() -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = std::fs::read_dir(repo_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let id = name.strip_suffix(".rail")?.to_string();
            Some((id, std::fs::read(e.path()).ok()?))
        })
        .collect();
    v.sort();
    v
}

/// Bring the working tree to the remote's state. `Ok(false)` = the remote is empty.
/// Local sync-repo state is disposable: our own file is rebuilt from the data directory.
fn fetch() -> Result<bool, String> {
    match git(&["fetch", "origin", BRANCH]) {
        Ok(_) => {
            git(&["checkout", "-q", "-f", "-B", BRANCH, &format!("origin/{BRANCH}")])?;
            Ok(true)
        }
        Err(e) if e.contains("couldn't find remote ref") => Ok(false),
        Err(e) => Err(e),
    }
}

fn init(url: &str, remember: bool) -> Result<(), String> {
    if initialised() {
        return Err(format!("already set up in {} (use `rail sync git remote set-url origin <url>` to change it)", repo_dir().display()));
    }
    std::fs::create_dir_all(repo_dir()).map_err(|e| e.to_string())?;
    git(&["init", "-q", "-b", BRANCH])?;
    git(&["remote", "add", "origin", url])?;
    let existing = fetch()?;
    let has_data = existing && !device_files().is_empty();
    let pass = passphrase(!has_data)?;
    if has_data {
        // Joining an existing repo: prove the passphrase before we change anything.
        let (_, data) = &device_files()[0];
        sync::decrypt(data, &pass).map_err(|e| e.to_string())?;
    }
    if remember {
        match keyring(&["store", "--label=rail sync", "application", "rail-sync"], Some(&pass)) {
            Some(_) => eprintln!("passphrase stored in the desktop keyring"),
            None => eprintln!("could not store the passphrase (is `secret-tool` installed?)"),
        }
    }
    eprintln!("sync repo: {}", repo_dir().display());
    run_sync(&pass)
}

fn run_sync(pass: &str) -> Result<(), String> {
    let id = device_id();
    let dir = data_dir();
    for attempt in 1..=3 {
        fetch()?;
        // Everything the other devices (and our own last push) know.
        let mut merged = Bundle::collect(&dir);
        let mut theirs = 0;
        for (dev, data) in device_files() {
            let b = sync::decrypt(&data, pass).map_err(|e| format!("{dev}.rail: {e}"))?;
            merged = merged.merge(&b);
            theirs += usize::from(dev != id);
        }
        let before = Bundle::collect(&dir);
        merged.apply(&dir).map_err(|e| e.to_string())?;
        if merged.files != before.files {
            eprintln!("merged changes from {theirs} other device(s) into {}", dir.display());
        }

        // Re-encrypt only when something changed (age output differs on every run).
        let own = repo_dir().join(format!("{id}.rail"));
        let up_to_date = std::fs::read(&own).ok().and_then(|d| sync::decrypt(&d, pass).ok()).is_some_and(|b| b == merged);
        if !up_to_date {
            let blob = sync::encrypt(&merged, pass).map_err(|e| e.to_string())?;
            std::fs::write(&own, blob).map_err(|e| e.to_string())?;
        }
        git(&["add", "-A"])?;
        let dirty = !git(&["status", "--porcelain"])?.is_empty();
        if dirty {
            git(&["commit", "-q", "-m", &format!("sync {id}")])?;
        }
        let ahead = git(&["rev-list", "--count", &format!("origin/{BRANCH}..HEAD")]).unwrap_or_else(|_| "1".into());
        if !dirty && ahead == "0" {
            println!("up to date");
            return Ok(());
        }
        match git(&["push", "-q", "-u", "origin", BRANCH]) {
            Ok(_) => {
                println!("synced");
                return Ok(());
            }
            Err(e) if attempt == 3 => return Err(e),
            Err(_) => eprintln!("remote moved on, retrying…"),
        }
    }
    Ok(())
}

fn status() -> Result<(), String> {
    if !initialised() {
        return Err("not set up: run `rail sync init <git-url>`".into());
    }
    println!("repo:    {}", repo_dir().display());
    println!("remote:  {}", git(&["remote", "get-url", "origin"])?);
    println!("device:  {}", device_id());
    let devices: Vec<String> = device_files().into_iter().map(|d| d.0).collect();
    println!("devices: {}", if devices.is_empty() { "none yet".into() } else { devices.join(", ") });
    println!("last:    {}", git(&["log", "-1", "--format=%cr (%h)"]).unwrap_or_else(|_| "never synced".into()));
    Ok(())
}

const USAGE: &str = "usage:
  rail sync init <git-url> [--remember]   set up (--remember keeps the passphrase in the keyring)
  rail sync                               pull, merge, push
  rail sync status                        what is set up
  rail sync path                          where the repo lives
  rail sync git <args...>                 run git inside the repo

The passphrase is asked for each time, or taken from $RAIL_SYNC_PASSPHRASE.";

/// Entry point for `rail sync ...`; returns the process exit code.
pub fn cli(args: &[String]) -> i32 {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["init", rest @ ..] => match rest.iter().find(|a| !a.starts_with("--")) {
            Some(url) => init(url, rest.contains(&"--remember")),
            None => Err(USAGE.into()),
        },
        [] => {
            if !initialised() {
                Err("not set up: run `rail sync init <git-url>`".into())
            } else {
                passphrase(false).and_then(|p| run_sync(&p))
            }
        }
        ["status"] => status(),
        ["path"] => {
            println!("{}", repo_dir().display());
            Ok(())
        }
        ["git", rest @ ..] if initialised() => {
            let code = Command::new("git").args(rest).current_dir(repo_dir()).status();
            return code.ok().and_then(|s| s.code()).unwrap_or(1);
        }
        ["git", ..] => Err("not set up: run `rail sync init <git-url>`".into()),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("rail sync: {e}");
            1
        }
    }
}

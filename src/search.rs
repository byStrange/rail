//! Search shortcuts: `gh rust` in the address bar searches GitHub, and so on.
//! Add your own in `searchengines.txt` (one `alias  https://site/?q=%s` per line).

use gtk4::glib;

const BUILTIN: &[(&str, &str)] = &[
    ("g", "https://www.google.com/search?q=%s"),
    ("ddg", "https://duckduckgo.com/?q=%s"),
    ("gh", "https://github.com/search?q=%s"),
    ("yt", "https://www.youtube.com/results?search_query=%s"),
    ("w", "https://en.wikipedia.org/w/index.php?search=%s"),
    ("mdn", "https://developer.mozilla.org/en-US/search?q=%s"),
    ("so", "https://stackoverflow.com/search?q=%s"),
    ("r", "https://www.reddit.com/search/?q=%s"),
    ("crates", "https://crates.io/search?q=%s"),
    ("docs", "https://docs.rs/releases/search?query=%s"),
    ("npm", "https://www.npmjs.com/search?q=%s"),
    ("aur", "https://aur.archlinux.org/packages?K=%s"),
    ("aw", "https://wiki.archlinux.org/index.php?search=%s"),
    ("maps", "https://www.openstreetmap.org/search?query=%s"),
    ("i", "https://duckduckgo.com/?iax=images&ia=images&q=%s"),
];

pub type Engines = Vec<(String, String)>;

pub fn load(path: &std::path::Path) -> Engines {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let (alias, url) = l.trim().split_once(char::is_whitespace)?;
            let url = url.trim();
            (url.contains("%s")).then(|| (alias.to_string(), url.to_string()))
        })
        .collect()
}

/// `alias query...` -> the search URL, if `alias` is a known shortcut.
pub fn expand(input: &str, custom: &Engines) -> Option<String> {
    let (alias, rest) = input.trim().split_once(char::is_whitespace)?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let template = custom
        .iter()
        .find(|(a, _)| a == alias)
        .map(|(_, u)| u.as_str())
        .or_else(|| BUILTIN.iter().find(|(a, _)| *a == alias).map(|(_, u)| *u))?;
    Some(template.replace("%s", &glib::Uri::escape_string(rest, None, false)))
}

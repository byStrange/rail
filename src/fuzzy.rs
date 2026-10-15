//! Tiny fuzzy matcher for the tab search: every space-separated word of the
//! query has to match, in order, somewhere in the text.

/// Higher is better; `None` when a word does not match.
pub fn score(query: &str, title: &str, url: &str) -> Option<i32> {
    let (title, url) = (title.to_lowercase(), url.to_lowercase());
    let mut total = 0;
    for word in query.to_lowercase().split_whitespace() {
        let t = word_score(word, &title).map(|s| s * 3 / 2);
        let u = word_score(word, &url);
        total += t.into_iter().chain(u).max()?;
    }
    Some(total)
}

fn word_score(word: &str, hay: &str) -> Option<i32> {
    let w: Vec<char> = word.chars().collect();
    let h: Vec<char> = hay.chars().collect();
    if w.is_empty() {
        return Some(0);
    }
    let mut best = None;
    // Try every place the first letter occurs and match the rest greedily from there.
    for start in (0..h.len()).filter(|&i| h[i] == w[0]) {
        let (mut s, mut at, mut prev) = (0, start, None::<usize>);
        let mut ok = true;
        for &c in &w {
            let Some(i) = (at..h.len()).find(|&i| h[i] == c) else {
                ok = false;
                break;
            };
            s += 10;
            if prev == Some(i.wrapping_sub(1)) {
                s += 15;
            } else if let Some(p) = prev {
                s -= ((i - p - 1) as i32).min(8);
            }
            if i == 0 || !h[i - 1].is_alphanumeric() {
                s += 20;
            }
            prev = Some(i);
            at = i + 1;
        }
        if ok {
            best = best.max(Some(s));
        }
    }
    if best.is_some() && hay.contains(word) {
        best = best.map(|s| s + 50);
    }
    best
}

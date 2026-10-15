//! Moving around: fuzzy tab search, find in page, and the page's own key script.
//! Also the per-tab switches: pin, mute, ad blocking.

use super::*;
use webkit6::FindOptions;

const FIND_HINT: &str = "Enter  keep      ↑ ↓  next / previous      Esc  clear";

impl App {
    // ---- messages from the key script injected into every page ------------

    pub(crate) fn on_page_msg(self: &Rc<Self>, msg: &str) {
        match msg {
            "tabs" => self.open_palette(Mode::Tabs),
            "find" => self.open_palette(Mode::Find),
            "next" => self.find_step(1),
            "prev" => self.find_step(-1),
            "dirty" => {
                if let Some(t) = self.focused_tab() {
                    t.st.dirty.set(true);
                }
            }
            _ => {}
        }
    }

    // ---- fuzzy tab search --------------------------------------------------

    /// Fill the palette list with the open tabs that match `query`, best first.
    pub(crate) fn update_tab_results(&self, query: &str) {
        self.clear_suggestions();
        let q = query.trim();
        let cur = self.cur.get();
        let focused = self.focused_view();
        let mut hits: Vec<(i32, usize, usize)> = Vec::new();
        for (wi, ws) in self.wss.borrow().iter().enumerate() {
            for (ti, t) in ws.tabs.borrow().iter().enumerate() {
                let score = if q.is_empty() {
                    // The tab you are on is the least useful jump target.
                    Some(if focused.as_ref() == Some(&t.view) { -1 } else { i32::from(wi == cur) })
                } else {
                    let uri = t.uri().unwrap_or_default();
                    fuzzy::score(q, &Self::tab_label(t), history::strip(&uri))
                };
                if let Some(s) = score {
                    hits.push((s, wi, ti));
                }
            }
        }
        hits.sort_by(|a, b| b.0.cmp(&a.0)); // stable: ties keep strip order
        let wss = self.wss.borrow();
        let mut rows = self.sugg_tabs.borrow_mut();
        for &(_, wi, ti) in hits.iter().take(8) {
            let ws = &wss[wi];
            let tabs = ws.tabs.borrow();
            let t = &tabs[ti];
            let uri = t.uri().unwrap_or_default();
            let mut sub = format!("{}  ·  {}", ws.name.borrow(), history::strip(&uri));
            if t.st.pinned.get() {
                sub.push_str("  ·  pinned");
            }
            if t.suspended() {
                sub.push_str("  ·  asleep");
            }
            let row = gtk::Box::new(gtk::Orientation::Vertical, 1);
            row.add_css_class("sugg-row");
            for (text, class) in [(Self::tab_label(t), "sugg-title"), (sub, "sugg-url")] {
                let l = gtk::Label::new(Some(&text));
                l.add_css_class(class);
                l.set_xalign(0.0);
                l.set_ellipsize(gtk::pango::EllipsizeMode::End);
                l.set_width_chars(1);
                l.set_max_width_chars(1);
                row.append(&l);
            }
            self.sugg.append(&row);
            rows.push((wi, ti));
        }
        if rows.is_empty() {
            return;
        }
        self.sugg.set_visible(true);
        self.sugg_sel.set(Some(0));
        if let Some(r) = self.sugg.row_at_index(0) {
            self.sugg.select_row(Some(&r));
        }
    }

    /// ↑ / ↓ in the tab list: wraps around, never touches the typed text.
    pub(crate) fn tabs_move(&self, dir: isize) {
        let n = self.sugg_tabs.borrow().len() as isize;
        if n == 0 {
            return;
        }
        let next = (self.sugg_sel.get().unwrap_or(0) as isize + dir).rem_euclid(n) as usize;
        self.sugg_sel.set(Some(next));
        if let Some(r) = self.sugg.row_at_index(next as i32) {
            self.sugg.select_row(Some(&r));
        }
    }

    pub(crate) fn jump_to(self: &Rc<Self>, wi: usize, ti: usize) {
        if wi != self.cur.get() {
            self.switch_ws(wi);
        }
        if ti < self.ws().tabs.borrow().len() {
            self.focus_tab(ti);
        }
    }

    // ---- find in page --------------------------------------------------------

    fn set_find_flag(view: &WebView, on: bool) {
        // The key script only claims n / N while a search is active.
        view.evaluate_javascript(&format!("window.__railFind={}", u8::from(on)), Some("rail"), None, gio::Cancellable::NONE, |_| {});
    }

    pub(crate) fn find_text(&self, q: &str) {
        let Some(tab) = self.focused_tab() else { return };
        let Some(fc) = tab.view.find_controller() else { return };
        if q.is_empty() {
            fc.search_finish();
            tab.st.finding.set(false);
            Self::set_find_flag(&tab.view, false);
            self.palette_hint.set_text(FIND_HINT);
            return;
        }
        // Smart case: lower-case queries ignore case, anything with a capital does not.
        let mut opts = FindOptions::WRAP_AROUND;
        if !q.chars().any(char::is_uppercase) {
            opts |= FindOptions::CASE_INSENSITIVE;
        }
        fc.search(q, opts.bits(), 1000);
        fc.count_matches(q, opts.bits(), 1000);
        tab.st.finding.set(true);
        Self::set_find_flag(&tab.view, true);
    }

    pub(crate) fn clear_find(&self) {
        if let Some(t) = self.focused_tab() {
            if let Some(fc) = t.view.find_controller() {
                fc.search_finish();
            }
            t.st.finding.set(false);
            Self::set_find_flag(&t.view, false);
        }
    }

    /// n / N (and ↓ / ↑ in the find bar).
    pub(crate) fn find_step(&self, dir: isize) {
        let Some(fc) = self.focused_view().and_then(|v| v.find_controller()) else { return };
        if dir > 0 {
            fc.search_next();
        } else {
            fc.search_previous();
        }
    }

    pub(crate) fn find_result(self: &Rc<Self>, n: u32) {
        if self.palette_open() && self.mode.get() == Mode::Find {
            let text = match n {
                0 => "No matches".to_string(),
                1 => "1 match".to_string(),
                n => format!("{n} matches"),
            };
            self.palette_hint.set_text(&format!("{text}      {FIND_HINT}"));
        } else if n == 0 {
            self.toast("No matches");
        }
    }

    // ---- pinned tabs ---------------------------------------------------------

    pub(crate) fn toggle_pin(self: &Rc<Self>) {
        let ws = self.ws();
        let Some(tab) = self.focused_tab() else { return };
        let on = !tab.st.pinned.get();
        if on {
            let Some(url) = tab.uri().filter(|u| !u.starts_with("about:")) else {
                self.toast("Open a page first, then pin it");
                return;
            };
            *tab.st.pin_url.borrow_mut() = Some(url);
            tab.view.add_css_class("pinned");
        } else {
            *tab.st.pin_url.borrow_mut() = None;
            tab.view.remove_css_class("pinned");
        }
        tab.st.pinned.set(on);
        // Pinned tabs live at the front; this one sits right at the boundary.
        let all = ws.tabs.borrow().clone();
        let this = &tab.view;
        let others = |pinned: bool| all.iter().filter(move |t| &t.view != this && t.st.pinned.get() == pinned).cloned();
        let mut new: Vec<Tab> = others(true).collect();
        new.push(tab.clone());
        new.extend(others(false));
        self.set_order(&ws, new);
        self.save_pinned();
        self.toast(if on { "Pinned" } else { "Unpinned" });
    }

    pub(crate) fn save_pinned(&self) {
        let mut out = String::new();
        for (wi, ws) in self.wss.borrow().iter().enumerate() {
            for t in ws.tabs.borrow().iter().filter(|t| t.st.pinned.get()) {
                let url = t.st.pin_url.borrow().clone().or_else(|| t.uri());
                if let Some(u) = url {
                    out.push_str(&format!("{wi}\t{u}\n"));
                }
            }
        }
        let _ = std::fs::write(data_dir().join("pinned.txt"), out);
    }

    /// Pinned tabs come back on every start, asleep: they load when first focused.
    pub(crate) fn restore_pinned(self: &Rc<Self>) {
        let text = std::fs::read_to_string(data_dir().join("pinned.txt")).unwrap_or_default();
        let home = self.cur.get();
        for line in text.lines() {
            let Some((wi, url)) = line.split_once('\t') else { continue };
            let Ok(wi) = wi.parse::<usize>() else { continue };
            if wi >= self.wss.borrow().len() || url.is_empty() {
                continue;
            }
            self.cur.set(wi);
            let view = self.new_view();
            self.add_view(view.clone());
            if let Some(tab) = self.tab_of(&view) {
                tab.st.pinned.set(true);
                tab.view.add_css_class("pinned");
                *tab.st.pin_url.borrow_mut() = Some(url.to_string());
                self.put_to_sleep(&tab, url.to_string(), host_of(url).unwrap_or_else(|| url.to_string()), None, None, (0.0, 0.0));
            }
        }
        self.cur.set(home);
        self.mark_cur();
        self.refresh_sidebar();
    }

    // ---- mute ----------------------------------------------------------------

    pub(crate) fn toggle_mute(self: &Rc<Self>) {
        let Some(v) = self.focused_view() else { return };
        v.set_is_muted(!v.is_muted());
        self.toast(if v.is_muted() { "Tab muted" } else { "Tab unmuted" });
    }

    // ---- ad & tracker blocking ----------------------------------------------

    pub(crate) fn setup_adblock(self: &Rc<Self>) {
        let dir = data_dir();
        let extra = std::fs::read_to_string(dir.join("blocklist.txt")).unwrap_or_default();
        let json = blocklist::rules_json(&extra);
        let store = webkit6::UserContentFilterStore::new(dir.join("filters").to_str().unwrap_or("filters"));
        let keep = store.clone();
        let app = self.clone();
        store.save("rail-adblock", &glib::Bytes::from(json.as_bytes()), gio::Cancellable::NONE, move |res| {
            let _keep = keep;
            match res {
                Ok(f) => {
                    if app.adblock_on.get() {
                        app.ucm.add_filter(&f);
                    }
                    *app.adblock_filter.borrow_mut() = Some(f);
                }
                Err(e) => eprintln!("rail: ad blocking rules failed to compile: {e}"),
            }
        });
    }

    pub(crate) fn toggle_adblock(self: &Rc<Self>) {
        let on = !self.adblock_on.get();
        self.adblock_on.set(on);
        let marker = data_dir().join("adblock-off");
        if on {
            let _ = std::fs::remove_file(&marker);
        } else {
            let _ = std::fs::write(&marker, "");
        }
        if let Some(f) = self.adblock_filter.borrow().as_ref() {
            if on {
                self.ucm.add_filter(f);
            } else {
                self.ucm.remove_filter(f);
            }
        }
        self.toast(if on { "Ad & tracker blocking on  ·  reload to apply" } else { "Ad & tracker blocking off  ·  reload to apply" });
    }
}

//! Tab suspension: tabs nobody is looking at are put to sleep (the page is
//! unloaded, the tab keeps its title, icon, history and scroll position) and
//! wake up when focused. Pinned tabs also start the browser asleep.
//!
//! A tab is NEVER put to sleep when it is focused or on screen, loading,
//! making sound, using the camera or microphone, downloading, holding unsaved
//! input (you typed into a field), or just woke up. The more tabs are awake,
//! the sooner the idle ones go.

use super::*;
use webkit6::MediaCaptureState;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn placeholder(title: &str, uri: &str) -> String {
    format!(
        "<!doctype html><meta charset=utf-8><meta name=color-scheme content=dark>\
<style>html,body{{height:100%;margin:0}}\
body{{display:grid;place-items:center;background:#16161e;color:#565f89;font:14px/1.5 system-ui,sans-serif}}\
.w{{text-align:center;max-width:80%}}h1{{font-size:20px;font-weight:600;color:#a9b1d6;margin:0 0 4px;\
overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}\
p{{margin:0 0 18px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}\
i{{font-style:normal;color:#7aa2f7;font-size:12px;letter-spacing:.08em;text-transform:uppercase}}</style>\
<div class=w><h1>{}</h1><p>{}</p><i>asleep &middot; wakes when focused</i></div>",
        esc(title),
        esc(uri)
    )
}

/// How long a tab may sit unused, given how many tabs are awake.
fn idle_limit(awake: usize) -> Duration {
    // RAIL_IDLE_SECS exists so this can be tested without waiting twenty minutes.
    if let Some(s) = std::env::var("RAIL_IDLE_SECS").ok().and_then(|s| s.parse().ok()) {
        return Duration::from_secs(s);
    }
    Duration::from_secs(60 * match awake {
        0..=6 => 20,
        7..=12 => 10,
        13..=20 => 5,
        _ => 2,
    })
}

impl App {
    /// Unload the page behind `tab` and show the placeholder instead.
    pub(crate) fn put_to_sleep(
        self: &Rc<Self>,
        tab: &Tab,
        uri: String,
        title: String,
        favicon: Option<gdk::Texture>,
        state: Option<webkit6::WebViewSessionState>,
        scroll: (f64, f64),
    ) {
        tab.view.load_html(&placeholder(&title, &uri), None);
        *tab.st.susp.borrow_mut() = Some(Susp { uri, title, favicon, state, scroll });
        tab.st.scroll_to.set(None);
        tab.st.finding.set(false);
        self.refresh_sidebar();
    }

    /// Wake `tab` up in `ms` milliseconds, if it is still the focused one by then
    /// (so merely sweeping the pointer across tabs doesn't load them all).
    pub(crate) fn schedule_resume(self: &Rc<Self>, tab: &Tab, ms: u64) {
        let gen_ = tab.st.resume_gen.get().wrapping_add(1);
        tab.st.resume_gen.set(gen_);
        let (app, tab) = (self.clone(), tab.clone());
        glib::timeout_add_local_once(Duration::from_millis(ms), move || {
            let still_focused = app.focused_view().as_ref() == Some(&tab.view);
            if tab.st.resume_gen.get() == gen_ && still_focused {
                app.resume(&tab);
            }
        });
    }

    pub(crate) fn resume(self: &Rc<Self>, tab: &Tab) {
        let Some(s) = tab.st.susp.borrow_mut().take() else { return };
        tab.st.last_active.set(Instant::now());
        tab.st.scroll_to.set((s.scroll != (0.0, 0.0)).then_some(s.scroll));
        let v = &tab.view;
        let mut restored = false;
        if let Some(state) = &s.state {
            v.restore_session_state(state);
            if let Some(item) = v.back_forward_list().and_then(|l| l.current_item()) {
                v.go_to_back_forward_list_item(&item);
                restored = true;
            }
        }
        if !restored {
            v.load_uri(&s.uri);
        }
        // Safety net: if the history restore left us on the placeholder, just load the page.
        let (v, uri) = (tab.view.clone(), s.uri.clone());
        glib::timeout_add_local_once(Duration::from_millis(2000), move || {
            let blank = v.uri().is_none_or(|u| u == "about:blank");
            if blank && !v.is_loading() {
                v.load_uri(&uri);
            }
        });
        self.refresh_sidebar();
    }

    /// Is any part of tab `ti` on screen (now, or where the strip is sliding to)?
    fn on_screen(&self, ws: &Ws, ti: usize) -> bool {
        let vw = ws.strip.width() as f64;
        if vw <= 0.0 {
            return true;
        }
        let g = gap(1.0);
        let mut x = 0.0;
        for (i, t) in ws.tabs.borrow().iter().enumerate() {
            let w = col_width_of(&t.view, vw);
            if i == ti {
                return [ws.layout.offset(), ws.target.get()].iter().any(|&o| x < o + vw && x + w > o);
            }
            x += w + g;
        }
        false
    }

    /// Everything that must keep a tab awake, other than where it is on screen.
    fn may_sleep(&self, tab: &Tab) -> bool {
        let v = &tab.view;
        let downloading = self.downloads.borrow().iter().any(|d| {
            d.state.get() == DlState::Active
                && d.handle.borrow().as_ref().and_then(|h| h.web_view()).as_ref() == Some(v)
        });
        !tab.suspended()
            && !v.is_loading()
            && !v.is_playing_audio()
            && v.camera_capture_state() == MediaCaptureState::None
            && v.microphone_capture_state() == MediaCaptureState::None
            && !tab.st.dirty.get()
            && !downloading
            && self.fresh.borrow().as_ref() != Some(v)
            && tab.uri().is_some_and(|u| !u.starts_with("about:") && !u.is_empty())
    }

    /// Runs every half minute.
    pub(crate) fn suspend_sweep(self: &Rc<Self>) {
        let now = Instant::now();
        if self.ov.get() {
            // Overview shows every tab live: looking at them counts as using them.
            for ws in self.wss.borrow().iter() {
                for t in ws.tabs.borrow().iter() {
                    t.st.last_active.set(now);
                }
            }
            return;
        }
        let cur = self.cur.get();
        let awake: usize = self.wss.borrow().iter().map(|w| w.tabs.borrow().iter().filter(|t| !t.suspended()).count()).sum();
        let limit = idle_limit(awake);
        let mut sleepy = Vec::new();
        for (wi, ws) in self.wss.borrow().iter().enumerate() {
            for (ti, t) in ws.tabs.borrow().iter().enumerate() {
                if wi == cur && (ws.focus.get() == ti || self.on_screen(ws, ti)) {
                    t.st.last_active.set(now);
                } else if self.may_sleep(t) && now.duration_since(t.st.last_active.get()) >= limit {
                    sleepy.push(t.clone());
                }
            }
        }
        for t in sleepy {
            self.sleep_tab(t, limit);
        }
    }

    /// Ask the page where it is scrolled to, then (if nothing changed meanwhile) put it to sleep.
    fn sleep_tab(self: &Rc<Self>, tab: Tab, limit: Duration) {
        let app = self.clone();
        let t = tab.clone();
        tab.view.evaluate_javascript(
            "window.scrollX + ',' + window.scrollY",
            None,
            None,
            gio::Cancellable::NONE,
            move |res| {
                // Things may have changed while the page answered: check again.
                let Some((wi, ws)) = app.find_view(&t.view) else { return };
                let ti = ws.tabs.borrow().iter().position(|x| x.view == t.view).unwrap_or(usize::MAX);
                let seen = wi == app.cur.get() && (ws.focus.get() == ti || app.on_screen(&ws, ti));
                if seen || !app.may_sleep(&t) || t.st.last_active.get().elapsed() < limit {
                    return;
                }
                let scroll = res
                    .ok()
                    .map(|v| v.to_str().to_string())
                    .and_then(|s| {
                        let (x, y) = s.split_once(',')?;
                        Some((x.parse().ok()?, y.parse().ok()?))
                    })
                    .unwrap_or((0.0, 0.0));
                let uri = t.uri().unwrap_or_default();
                let title = Self::tab_label(&t);
                let (icon, state) = (t.view.favicon(), t.view.session_state());
                app.put_to_sleep(&t, uri, title, icon, state, scroll);
            },
        );
    }
}

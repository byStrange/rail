mod strip;

use gtk::glib;
use gtk::{gdk, gio};
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use strip::{SIDE_W, ShellLayout, StripLayout, col_width, gap};
use webkit6::prelude::*;
use webkit6::WebView;

const START_PAGE: &str = r#"<!doctype html><meta charset=utf-8><meta name=color-scheme content=dark>
<style>
html,body{height:100%;margin:0}
body{display:grid;place-items:center;background:#16161e;color:#6b7399;font:15px/1.5 system-ui,sans-serif}
.w{text-align:center}
h1{font-weight:600;font-size:60px;letter-spacing:-.03em;margin:0;color:#c0caf5}
h1 b{color:#7aa2f7}
p{margin:4px 0 40px}
.k{display:grid;grid-template-columns:auto auto;gap:12px 32px;text-align:left;justify-content:center}
.k span:nth-child(odd){text-align:right}
kbd{font:12px ui-monospace,monospace;background:#1f2335;border:1px solid #2f334d;border-bottom-width:2px;
    border-radius:6px;padding:2px 7px;color:#a9b1d6;margin-left:3px}
</style>
<div class=w><h1>rail<b>.</b></h1><p>a browser that scrolls sideways</p><div class=k>
<span><kbd>Ctrl</kbd><kbd>G</kbd></span><span>go to address</span>
<span><kbd>Ctrl</kbd><kbd>Enter</kbd></span><span>edit current URL</span>
<span><kbd>Ctrl</kbd><kbd>T</kbd></span><span>new tab</span>
<span><kbd>Ctrl</kbd><kbd>F</kbd></span><span>full / half width</span>
<span><kbd>Ctrl</kbd><kbd>H</kbd> <kbd>L</kbd></span><span>focus left / right</span>
<span><kbd>Ctrl</kbd><kbd>O</kbd></span><span>overview</span>
<span><kbd>Ctrl</kbd><kbd>B</kbd></span><span>sidebar</span>
</div></div>"#;

const CSS: &str = "
window { background: #0f1016; border-radius: 0; box-shadow: none; }
label { color: #a9b1d6; }

/* ---- tabs ---- */
.tab { border-radius: 12px; border: 2px solid #1a1b26; }
.tab.focused { border-color: #7aa2f7; }

/* ---- sidebar ---- */
.brand { font-weight: 700; font-size: 17px; letter-spacing: -0.02em; color: #c0caf5; }
.brand-dot { color: #7aa2f7; }
button.flat-icon { background: transparent; border: none; box-shadow: none; min-width: 28px; min-height: 28px;
                   padding: 0; border-radius: 8px; color: #7982a9; }
button.flat-icon:hover { background: #1f2335; color: #c0caf5; }
button.pill { background: #1a1b26; border: 1px solid #272a40; border-radius: 10px; padding: 8px 12px;
              box-shadow: none; color: #a9b1d6; }
button.pill:hover { background: #1f2335; border-color: #3b4261; }
button.pill label { color: #c0caf5; }
.tablist, .tablist row { background: transparent; }
.tablist row { padding: 0; margin: 1px 0; border-radius: 10px; }
.tablist row:hover { background: #151620; }
.tablist row:selected { background: #24283b; }
.tabrow { padding: 8px 8px 8px 10px; }
.tabrow label { color: #8089b3; font-size: 13px; }
row:selected .tabrow label { color: #c0caf5; }
.tabrow image { color: #7982a9; }
.tab-close { opacity: 0; min-width: 22px; min-height: 22px; padding: 0; border-radius: 6px;
             background: transparent; border: none; box-shadow: none; color: #7982a9; transition: opacity 120ms; }
.tab-close:hover { background: #3b4261; color: #c0caf5; }
row:hover .tab-close, row:selected .tab-close { opacity: 1; }
.hint { color: #3f4561; font-size: 11px; }

.fav, .tabrow label.fav, row:selected .tabrow label.fav { border-radius: 5px; font-size: 10px; font-weight: 700; color: #0f1016; }
.fav-0 { background: #7aa2f7; } .fav-1 { background: #bb9af7; } .fav-2 { background: #9ece6a; }
.fav-3 { background: #e0af68; } .fav-4 { background: #f7768e; } .fav-5 { background: #7dcfff; }
.fav-6 { background: #ff9e64; } .fav-7 { background: #73daca; }

/* ---- load progress ---- */
progressbar.load-bar { min-height: 3px; margin: 0 8px; }
progressbar.load-bar trough { min-height: 3px; background: transparent; border: none; }
progressbar.load-bar progress { min-height: 3px; background: #7aa2f7; border-radius: 3px;
                                box-shadow: 0 0 10px rgba(122,162,247,0.7); }

/* ---- address palette ---- */
.palette { background: #1a1b26; border-radius: 16px; border: 1px solid #2f334d; padding: 16px 20px 12px;
           box-shadow: 0 24px 64px rgba(0,0,0,0.55); min-width: 620px; }
.palette image { color: #7982a9; }
entry.palette-entry, entry.palette-entry:focus-within { background: none; border: none; box-shadow: none; outline: none;
    padding: 0; min-height: 0; font-size: 19px; color: #c0caf5; }
.palette-hint { color: #565f89; font-size: 12px; margin-top: 8px; }

/* ---- overview (the live strip, zoomed out) ---- */
.zoomed .tab { opacity: 0.88; border-radius: 28px; }
.zoomed .tab.focused { opacity: 1; outline: 7px solid #7aa2f7; outline-offset: -5px; }
.ov-hint { background: #1a1b26; color: #8089b3; border: 1px solid #2f334d; border-radius: 10px;
           padding: 8px 16px; font-size: 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.4); }
";

/// How far the strip zooms out in overview (niri uses a similar factor).
const OV_ZOOM: f64 = 0.4;

struct Tab {
    view: WebView,
}

struct App {
    win: gtk::ApplicationWindow,
    shell: gtk::Box,
    shell_layout: ShellLayout,
    side: gtk::Box,
    strip: gtk::Box,
    layout: StripLayout,
    list: gtk::ListBox,
    pill_label: gtk::Label,
    load_bar: gtk::ProgressBar,
    palette: gtk::Box,
    entry: gtk::Entry,
    ov_hint: gtk::Label,
    /// Overview is active: the strip is zoomed out and tabs ignore the pointer.
    ov: Cell<bool>,
    /// Tab that was focused when overview opened; Esc returns here.
    ov_return: Cell<usize>,
    /// Scroll position before the overview, restored when zooming back in.
    ov_saved: Cell<f64>,
    target_zoom: Cell<f64>,
    tabs: RefCell<Vec<Tab>>,
    focus: Cell<usize>,
    target: Cell<f64>,
    animating: Cell<bool>,
    side_open: Cell<bool>,
    side_gen: Cell<u32>,
    /// A just-created tab that has never navigated; Esc discards it.
    fresh: RefCell<Option<WebView>>,
}

fn to_uri(input: &str) -> String {
    let s = input.trim();
    if s.contains("://") || s.starts_with("about:") || s.starts_with("file:") {
        return s.into();
    }
    if s.starts_with("localhost") || s.starts_with("127.0.0.1") {
        return format!("http://{s}");
    }
    if !s.contains(' ') && s.contains('.') {
        return format!("https://{s}");
    }
    format!("https://duckduckgo.com/?q={}", glib::Uri::escape_string(s, None, false))
}

fn host_of(uri: &str) -> Option<String> {
    let u = glib::Uri::parse(uri, glib::UriFlags::NONE).ok()?;
    let h = u.host()?;
    Some(h.trim_start_matches("www.").to_string())
}

impl App {
    fn focused_view(&self) -> Option<WebView> {
        self.tabs.borrow().get(self.focus.get()).map(|t| t.view.clone())
    }

    fn palette_open(&self) -> bool {
        gtk::prelude::WidgetExt::is_visible(&self.palette)
    }

    fn overview_open(&self) -> bool {
        self.ov.get()
    }

    // ---- strip scrolling -------------------------------------------------

    /// Normal mode: scroll the minimum needed to show the focused column.
    /// Overview: center the focused column in the zoomed-out strip.
    fn reveal_focus(self: &Rc<Self>) {
        let vw = self.strip.width() as f64;
        if vw <= 0.0 {
            return;
        }
        let visible = vw / self.target_zoom.get(); // strip width that fits on screen
        let g = gap(self.target_zoom.get());
        let tabs = self.tabs.borrow();
        let mut x = 0.0;
        let mut span = None;
        for (i, t) in tabs.iter().enumerate() {
            let w = col_width(t.view.has_css_class("full"), vw);
            if i == self.focus.get() {
                span = Some((x, x + w));
            }
            x += w + g;
        }
        let total = x - g;
        let Some((l, r)) = span else { return };
        let max = (total - visible).max(0.0);
        let t = if self.ov.get() {
            if total <= visible {
                -(visible - total) / 2.0 // everything fits: center it all
            } else {
                ((l + r) / 2.0 - visible / 2.0).clamp(0.0, max)
            }
        } else {
            let mut t = self.target.get().clamp(0.0, max);
            if l < t {
                t = l;
            } else if r > t + visible {
                t = r - visible;
            }
            t.clamp(0.0, max)
        };
        self.target.set(t);
        drop(tabs);
        self.animate();
    }

    /// Ease both the scroll offset and the zoom toward their targets.
    fn animate(self: &Rc<Self>) {
        if self.animating.get() {
            return;
        }
        let near = |a: f64, b: f64, eps: f64| (a - b).abs() < eps;
        if near(self.target.get(), self.layout.offset(), 0.5) && near(self.target_zoom.get(), self.layout.zoom(), 0.002) {
            self.layout.set_offset(self.target.get());
            self.layout.set_zoom(self.target_zoom.get());
            self.strip.queue_allocate();
            return;
        }
        self.animating.set(true);
        let app = self.clone();
        let last = Cell::new(None::<i64>);
        self.strip.add_tick_callback(move |strip, clock| {
            let now = clock.frame_time();
            let dt = last.replace(Some(now)).map_or(1.0 / 60.0, |l| (now - l) as f64 / 1e6);
            let k = 1.0 - (-dt / 0.06).exp();
            let w = (strip.width() as f64).max(1.0);
            let (oc, ot) = (app.layout.offset(), app.target.get());
            let (zc, zt) = (app.layout.zoom(), app.target_zoom.get());
            // Animate the camera *center* (the strip point at screen center), not the
            // left edge: then zooming pivots on the middle of the screen instead of
            // visibly shrinking toward the left and sliding into place.
            let (cc, ct) = (oc + w / (2.0 * zc), ot + w / (2.0 * zt));
            let mut cn = cc + (ct - cc) * k;
            let mut znext = zc + (zt - zc) * k;
            if near(ct, cn, 0.5) {
                cn = ct;
            }
            if near(zt, znext, 0.002) {
                znext = zt;
            }
            let next = if cn == ct && znext == zt { ot } else { cn - w / (2.0 * znext) };
            let tgt = ot;
            app.layout.set_offset(next);
            app.layout.set_zoom(znext);
            strip.queue_allocate();
            if next == tgt && znext == zt {
                app.animating.set(false);
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    // ---- tabs ------------------------------------------------------------

    fn new_tab(self: &Rc<Self>) {
        self.exit_overview();
        let view = WebView::new();
        view.load_html(START_PAGE, None);
        *self.fresh.borrow_mut() = Some(view.clone());
        self.add_view(view);
        self.show_palette();
    }

    fn add_view(self: &Rc<Self>, view: WebView) {
        view.add_css_class("tab");
        view.set_overflow(gtk::Overflow::Hidden);
        view.set_can_target(!self.ov.get());
        let idx = if self.tabs.borrow().is_empty() { 0 } else { self.focus.get() + 1 };
        {
            let tabs = self.tabs.borrow();
            match idx {
                0 => self.strip.prepend(&view),
                _ => self.strip.insert_child_after(&view, Some(&tabs[idx - 1].view)),
            }
        }
        self.tabs.borrow_mut().insert(idx, Tab { view: view.clone() });

        let app = self.clone();
        view.connect_title_notify(move |_| app.refresh_sidebar());
        let app = self.clone();
        view.connect_uri_notify(move |_| app.refresh_sidebar());
        let app = self.clone();
        view.connect_favicon_notify(move |_| app.refresh_sidebar());
        let app = self.clone();
        view.connect_is_loading_notify(move |_| {
            app.refresh_sidebar();
            app.update_load_bar();
        });
        let app = self.clone();
        view.connect_estimated_load_progress_notify(move |_| app.update_load_bar());
        let app = self.clone();
        view.connect_create(move |v, _| {
            let nv = WebView::builder().related_view(v).build();
            app.add_view(nv.clone());
            Some(nv.upcast())
        });
        let app = self.clone();
        view.connect_close(move |v| app.close_view(v));

        self.focus_tab(idx);
    }

    fn close_view(self: &Rc<Self>, view: &WebView) {
        let Some(idx) = self.tabs.borrow().iter().position(|t| &t.view == view) else { return };
        self.strip.remove(view);
        self.tabs.borrow_mut().remove(idx);
        if self.fresh.borrow().as_ref() == Some(view) {
            *self.fresh.borrow_mut() = None;
        }
        self.focus.set(usize::MAX);
        let len = self.tabs.borrow().len();
        if len == 0 {
            self.new_tab();
        } else {
            self.focus_tab(idx.saturating_sub(1).min(len - 1));
        }
    }

    fn focus_tab(self: &Rc<Self>, idx: usize) {
        if idx >= self.tabs.borrow().len() {
            return;
        }
        self.focus.set(idx);
        for (i, t) in self.tabs.borrow().iter().enumerate() {
            if i == idx {
                t.view.add_css_class("focused");
            } else {
                t.view.remove_css_class("focused");
            }
        }
        if !self.tabs.borrow()[idx].view.is_loading() {
            self.load_bar.set_visible(false);
        }
        self.update_load_bar();
        self.reveal_focus();
        self.refresh_sidebar();
        if !self.palette_open() && !self.ov.get() {
            self.tabs.borrow()[idx].view.grab_focus();
        }
    }

    fn move_tab(self: &Rc<Self>, dir: isize) {
        let i = self.focus.get();
        let Some(j) = i.checked_add_signed(dir).filter(|&j| j < self.tabs.borrow().len()) else { return };
        self.tabs.borrow_mut().swap(i, j);
        let tabs = self.tabs.borrow();
        let mut prev: Option<&WebView> = None;
        for t in tabs.iter() {
            self.strip.reorder_child_after(&t.view, prev);
            prev = Some(&t.view);
        }
        drop(tabs);
        self.focus.set(j);
        self.strip.queue_allocate();
        self.reveal_focus();
        self.refresh_sidebar();
    }

    fn toggle_full(self: &Rc<Self>) {
        let Some(v) = self.focused_view() else { return };
        if v.has_css_class("full") {
            v.remove_css_class("full");
        } else {
            v.add_css_class("full");
        }
        self.strip.queue_allocate();
        self.reveal_focus();
    }

    // ---- sidebar ---------------------------------------------------------

    fn tab_label(t: &Tab) -> String {
        match t.view.title().filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => t
                .view
                .uri()
                .filter(|u| !u.starts_with("about:"))
                .map_or("New tab".into(), |u| host_of(&u).unwrap_or(u.to_string())),
        }
    }

    fn tab_icon(t: &Tab, size: i32) -> gtk::Widget {
        if t.view.is_loading() {
            let s = gtk::Spinner::new();
            s.set_size_request(size, size);
            s.start();
            return s.upcast();
        }
        if let Some(f) = t.view.favicon() {
            let img = gtk::Image::from_paintable(Some(&f));
            img.set_pixel_size(size);
            return img.upcast();
        }
        // No favicon yet: a colored letter badge, stable per site.
        let label = Self::tab_label(t);
        let letter = label.chars().find(|c| c.is_alphanumeric()).unwrap_or('·');
        let seed = label.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        let badge = gtk::Label::new(Some(&letter.to_uppercase().to_string()));
        badge.add_css_class("fav");
        badge.add_css_class(&format!("fav-{}", seed % 8));
        badge.set_size_request(size, size);
        badge.set_valign(gtk::Align::Center);
        badge.upcast()
    }

    fn refresh_sidebar(self: &Rc<Self>) {
        while let Some(r) = self.list.row_at_index(0) {
            self.list.remove(&r);
        }
        for t in self.tabs.borrow().iter() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("tabrow");
            row.append(&Self::tab_icon(t, 16));
            let label = gtk::Label::new(Some(&Self::tab_label(t)));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.set_width_chars(1);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&label);
            let close = gtk::Button::from_icon_name("window-close-symbolic");
            close.add_css_class("tab-close");
            close.set_valign(gtk::Align::Center);
            let (app, v) = (self.clone(), t.view.clone());
            close.connect_clicked(move |_| app.close_view(&v));
            row.append(&close);
            self.list.append(&row);
        }
        if let Some(row) = self.list.row_at_index(self.focus.get() as i32) {
            self.list.select_row(Some(&row));
        }
        let text = match self.focused_view().and_then(|v| v.uri()) {
            Some(u) if !u.starts_with("about:") => host_of(&u).unwrap_or(u.to_string()),
            _ => "Search or enter address".into(),
        };
        self.pill_label.set_text(&text);
    }

    fn toggle_sidebar(self: &Rc<Self>) {
        self.set_sidebar(!self.side_open.get());
    }

    fn set_sidebar(self: &Rc<Self>, open: bool) {
        self.side_open.set(open);
        let epoch = self.side_gen.get().wrapping_add(1);
        self.side_gen.set(epoch);
        if open {
            self.side.set_visible(true);
        }
        let start = self.shell_layout.side();
        let end = if open { SIDE_W as f64 } else { 0.0 };
        let app = self.clone();
        let t0 = Cell::new(None::<i64>);
        self.shell.add_tick_callback(move |shell, clock| {
            if app.side_gen.get() != epoch {
                return glib::ControlFlow::Break; // superseded by a newer toggle
            }
            let now = clock.frame_time();
            let begin = t0.get().unwrap_or_else(|| {
                t0.set(Some(now));
                now
            });
            let p = ((now - begin) as f64 / 200_000.0).min(1.0);
            let e = 1.0 - (1.0 - p).powi(3);
            app.shell_layout.set_side(start + (end - start) * e);
            shell.queue_allocate();
            if p >= 1.0 {
                if !open {
                    app.side.set_visible(false);
                }
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
        if let Some(v) = self.focused_view() {
            v.grab_focus();
        }
    }

    // ---- load feedback ---------------------------------------------------

    /// Thin bar along the top of the window showing the focused tab's progress.
    fn update_load_bar(self: &Rc<Self>) {
        let bar = &self.load_bar;
        let Some(v) = self.focused_view() else {
            bar.set_visible(false);
            return;
        };
        if v.is_loading() {
            bar.set_visible(true);
            // Never show an empty bar: it should read as "started" right away.
            bar.set_fraction(v.estimated_load_progress().max(0.08));
        } else if gtk::prelude::WidgetExt::is_visible(bar) {
            bar.set_fraction(1.0);
            let app = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(250), move || {
                if !app.focused_view().is_some_and(|v| v.is_loading()) {
                    app.load_bar.set_visible(false);
                }
            });
        }
    }

    // ---- address palette -------------------------------------------------

    fn show_palette(&self) {
        let uri = self.focused_view().and_then(|v| v.uri()).filter(|u| !u.starts_with("about:"));
        self.entry.set_text(uri.as_deref().unwrap_or(""));
        self.palette.set_visible(true);
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
    }

    fn hide_palette(&self) {
        self.palette.set_visible(false);
        if let Some(v) = self.focused_view() {
            v.grab_focus();
        }
    }

    /// Esc on the palette: close it, and also drop a tab that was never used.
    fn cancel_palette(self: &Rc<Self>) {
        let view = self.focused_view();
        let fresh = self.fresh.borrow().clone();
        self.hide_palette();
        if let (Some(v), Some(f)) = (view, fresh) {
            if v == f && self.tabs.borrow().len() > 1 {
                self.close_view(&v);
            }
        }
    }

    fn navigate(self: &Rc<Self>, text: &str) {
        if !text.trim().is_empty() {
            if let Some(v) = self.focused_view() {
                let uri = to_uri(text);
                self.pill_label.set_text(&host_of(&uri).unwrap_or(uri.clone()));
                v.load_uri(&uri);
                *self.fresh.borrow_mut() = None;
            }
        }
        self.hide_palette();
    }

    // ---- overview (niri-style: the live strip, zoomed out) ----------------

    fn toggle_overview(self: &Rc<Self>) {
        if self.ov.get() {
            self.exit_overview(); // Ctrl+O again opens whatever is focused now
        } else {
            self.enter_overview();
        }
    }

    fn enter_overview(self: &Rc<Self>) {
        if self.ov.get() || self.tabs.borrow().is_empty() {
            return;
        }
        self.palette.set_visible(false);
        self.ov.set(true);
        self.ov_return.set(self.focus.get());
        self.ov_saved.set(self.target.get());
        self.strip.add_css_class("zoomed");
        self.ov_hint.set_visible(true);
        for t in self.tabs.borrow().iter() {
            t.view.set_can_target(false); // clicks pick a tab, they don't reach the page
        }
        self.target_zoom.set(OV_ZOOM);
        self.reveal_focus();
    }

    /// Zoom back in on whichever tab is focused now.
    fn exit_overview(self: &Rc<Self>) {
        if !self.ov.get() {
            return;
        }
        self.ov.set(false);
        self.strip.remove_css_class("zoomed");
        self.ov_hint.set_visible(false);
        for t in self.tabs.borrow().iter() {
            t.view.set_can_target(true);
        }
        self.target_zoom.set(1.0);
        // Start from the pre-overview scroll: if the focus didn't move we land exactly
        // where we left, otherwise we scroll the minimum to reveal the new tab.
        self.target.set(self.ov_saved.get());
        self.reveal_focus();
        if let Some(v) = self.focused_view() {
            v.grab_focus();
        }
    }

    /// Esc: zoom back in on the tab you started from.
    fn cancel_overview(self: &Rc<Self>) {
        let back = self.ov_return.get().min(self.tabs.borrow().len().saturating_sub(1));
        self.focus_tab(back);
        self.exit_overview();
    }

    /// Click in overview: the tab under the pointer wins; empty space just exits.
    fn ov_click(self: &Rc<Self>, x: f64) {
        let vw = self.strip.width() as f64;
        let (z, off) = (self.layout.zoom(), self.layout.offset());
        let g = gap(z);
        let mut hit = None;
        let mut cx = 0.0;
        for (i, t) in self.tabs.borrow().iter().enumerate() {
            let w = col_width(t.view.has_css_class("full"), vw);
            let left = (cx - off) * z;
            if x >= left && x <= left + w * z {
                hit = Some(i);
                break;
            }
            cx += w + g;
        }
        if let Some(i) = hit {
            self.focus_tab(i);
        }
        self.exit_overview();
    }

    /// Keys while zoomed out. Returns true when consumed.
    fn ov_key(self: &Rc<Self>, k: gdk::Key, m: gdk::ModifierType) -> bool {
        use gdk::ModifierType as M;
        let plain = m.is_empty();
        let f = self.focus.get();
        if plain || m == M::CONTROL_MASK {
            match k {
                gdk::Key::h | gdk::Key::Left => {
                    self.focus_tab(f.saturating_sub(1));
                    return true;
                }
                gdk::Key::l | gdk::Key::Right => {
                    self.focus_tab(f + 1);
                    return true;
                }
                _ => {}
            }
        }
        if plain {
            match k {
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => self.exit_overview(),
                gdk::Key::x | gdk::Key::Delete | gdk::Key::BackSpace => {
                    if let Some(v) = self.focused_view() {
                        self.close_view(&v);
                    }
                }
                _ => {}
            }
        }
        // Swallow stray typing so it never reaches a page.
        plain
    }

    // ---- keys ------------------------------------------------------------

    fn on_key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> bool {
        use gdk::ModifierType as M;
        let m = state & gtk::accelerator_get_default_mod_mask();
        let k = key.to_lower();

        if k == gdk::Key::Escape {
            if self.palette_open() {
                self.cancel_palette();
                return true;
            }
            if self.ov.get() {
                self.cancel_overview();
                return true;
            }
            return false;
        }
        let global = m == M::CONTROL_MASK && matches!(k, gdk::Key::o | gdk::Key::t | gdk::Key::q);
        if self.overview_open() && !global && self.ov_key(k, m) {
            return true;
        }
        if m.is_empty() && k == gdk::Key::F11 {
            self.win.set_fullscreened(!self.win.is_fullscreen());
            return true;
        }
        if m == M::ALT_MASK {
            let v = self.focused_view();
            match k {
                gdk::Key::Left => v.map(|v| v.go_back()),
                gdk::Key::Right => v.map(|v| v.go_forward()),
                _ => return false,
            };
            return true;
        }
        let focus = self.focus.get();
        if m == M::CONTROL_MASK {
            match k {
                gdk::Key::t => self.new_tab(),
                gdk::Key::f => self.toggle_full(),
                gdk::Key::h => self.focus_tab(focus.saturating_sub(1)),
                gdk::Key::l => self.focus_tab(focus + 1),
                gdk::Key::w => {
                    if let Some(v) = self.focused_view() {
                        self.close_view(&v);
                    }
                }
                gdk::Key::b => self.toggle_sidebar(),
                gdk::Key::o => self.toggle_overview(),
                gdk::Key::g => self.show_palette(),
                // Edit the focused tab's URL (already open? let the entry handle it).
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    if self.palette_open() {
                        return false;
                    }
                    self.show_palette();
                }
                gdk::Key::r => {
                    if let Some(v) = self.focused_view() {
                        v.reload();
                    }
                }
                gdk::Key::q => self.win.close(),
                _ => return false,
            }
            return true;
        }
        if m == M::CONTROL_MASK | M::SHIFT_MASK {
            match k {
                gdk::Key::h => self.move_tab(-1),
                gdk::Key::l => self.move_tab(1),
                _ => return false,
            }
            return true;
        }
        false
    }
}

/// Keep logins across restarts: WebKit only writes cookies to disk if told to.
fn setup_storage() {
    let dir = glib::user_data_dir().join("rail");
    let _ = std::fs::create_dir_all(&dir);
    if let Some(cookies) = webkit6::NetworkSession::default().and_then(|s| s.cookie_manager()) {
        cookies.set_persistent_storage(
            dir.join("cookies.sqlite").to_str().unwrap_or("cookies.sqlite"),
            webkit6::CookiePersistentStorage::Sqlite,
        );
    }
}

fn build(gapp: &gtk::Application) {
    setup_storage();
    if let Some(s) = gtk::Settings::default() {
        s.set_gtk_application_prefer_dark_theme(true);
    }
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("no display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // ---- sidebar ----
    let brand = gtk::Label::new(None);
    brand.set_markup("<span weight='bold' size='large' foreground='#c0caf5'>rail</span><span weight='bold' size='large' foreground='#7aa2f7'>.</span>");
    brand.set_hexpand(true);
    brand.set_xalign(0.0);
    let add = gtk::Button::from_icon_name("list-add-symbolic");
    add.add_css_class("flat-icon");
    add.set_tooltip_text(Some("New tab  (Ctrl+T)"));
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    head.set_margin_start(8);
    head.append(&brand);
    head.append(&add);

    let pill_label = gtk::Label::new(Some("Search or enter address"));
    pill_label.set_xalign(0.0);
    pill_label.set_hexpand(true);
    pill_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pill_label.set_width_chars(1);
    let pill_inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    pill_inner.append(&gtk::Image::from_icon_name("system-search-symbolic"));
    pill_inner.append(&pill_label);
    let pill = gtk::Button::new();
    pill.add_css_class("pill");
    pill.set_child(Some(&pill_inner));
    pill.set_tooltip_text(Some("Go to address  (Ctrl+G)"));

    let list = gtk::ListBox::new();
    list.add_css_class("tablist");
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(true);
    let list_scroll = gtk::ScrolledWindow::new();
    list_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    list_scroll.set_vexpand(true);
    list_scroll.set_child(Some(&list));

    let hint = gtk::Label::new(Some("ctrl+b hide  ·  ctrl+o overview"));
    hint.add_css_class("hint");
    hint.set_xalign(0.0);
    hint.set_margin_start(8);

    let side = gtk::Box::new(gtk::Orientation::Vertical, 10);
    side.set_margin_top(10);
    side.set_margin_bottom(10);
    side.set_margin_start(10);
    side.set_margin_end(2);
    side.append(&head);
    side.append(&pill);
    side.append(&list_scroll);
    side.append(&hint);

    // ---- strip + shell ----
    let layout = StripLayout::new();
    let strip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    strip.set_layout_manager(Some(layout.clone()));
    strip.set_overflow(gtk::Overflow::Hidden);
    strip.set_margin_top(8);
    strip.set_margin_bottom(8);
    strip.set_margin_end(8);
    strip.set_margin_start(6);

    let shell_layout = ShellLayout::new();
    shell_layout.set_side(SIDE_W as f64);
    let shell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    shell.set_layout_manager(Some(shell_layout.clone()));
    shell.set_overflow(gtk::Overflow::Hidden);
    shell.set_hexpand(true);
    shell.set_vexpand(true);
    shell.append(&side);
    shell.append(&strip);

    // ---- address palette ----
    let entry = gtk::Entry::new();
    entry.add_css_class("palette-entry");
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Search or enter address"));
    let prow = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let sicon = gtk::Image::from_icon_name("system-search-symbolic");
    sicon.set_pixel_size(20);
    prow.append(&sicon);
    prow.append(&entry);
    let phint = gtk::Label::new(Some("Enter  go      Esc  cancel"));
    phint.add_css_class("palette-hint");
    phint.set_xalign(0.0);
    let palette = gtk::Box::new(gtk::Orientation::Vertical, 0);
    palette.add_css_class("palette");
    palette.set_halign(gtk::Align::Center);
    palette.set_valign(gtk::Align::Start);
    palette.set_margin_top(110);
    palette.append(&prow);
    palette.append(&phint);
    palette.set_visible(false);

    // ---- overview hint (the overview itself is the strip, zoomed) ----
    let ov_hint = gtk::Label::new(Some("h / l  move      Enter  open      x  close      Esc  cancel"));
    ov_hint.add_css_class("ov-hint");
    ov_hint.set_halign(gtk::Align::Center);
    ov_hint.set_valign(gtk::Align::End);
    ov_hint.set_margin_bottom(28);
    ov_hint.set_can_target(false);
    ov_hint.set_visible(false);

    let load_bar = gtk::ProgressBar::new();
    load_bar.add_css_class("load-bar");
    load_bar.set_valign(gtk::Align::Start);
    load_bar.set_can_target(false); // never steal clicks from the page
    load_bar.set_visible(false);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&shell));
    overlay.add_overlay(&load_bar);
    overlay.add_overlay(&palette);
    overlay.add_overlay(&ov_hint);

    let win = gtk::ApplicationWindow::builder()
        .application(gapp)
        .default_width(1400)
        .default_height(850)
        .decorated(false) // no header, no footer: the page is the window
        .child(&overlay)
        .build();

    let app = Rc::new(App {
        win: win.clone(),
        shell: shell.clone(),
        shell_layout,
        side,
        strip,
        layout: layout.clone(),
        list: list.clone(),
        pill_label,
        load_bar,
        palette,
        entry: entry.clone(),
        ov_hint,
        ov: Cell::new(false),
        ov_return: Cell::new(0),
        ov_saved: Cell::new(0.0),
        target_zoom: Cell::new(1.0),
        tabs: RefCell::new(Vec::new()),
        focus: Cell::new(usize::MAX),
        target: Cell::new(0.0),
        animating: Cell::new(false),
        side_open: Cell::new(true),
        side_gen: Cell::new(0),
        fresh: RefCell::new(None),
    });

    let a = app.clone();
    layout.connect_resized(move || a.reveal_focus());
    let a = app.clone();
    list.connect_row_activated(move |_, row| a.focus_tab(row.index() as usize));
    let a = app.clone();
    add.connect_clicked(move |_| a.new_tab());
    let a = app.clone();
    pill.connect_clicked(move |_| a.show_palette());
    let a = app.clone();
    entry.connect_activate(move |e| a.navigate(&e.text()));

    // In overview, clicking a (non-interactive) zoomed tab selects it.
    let pick = gtk::GestureClick::new();
    let a = app.clone();
    pick.connect_released(move |_, _, x, _| {
        if a.ov.get() {
            a.ov_click(x);
        }
    });
    app.strip.add_controller(pick);

    // Clicking anywhere in the page area dismisses the address palette.
    let click = gtk::GestureClick::new();
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let a = app.clone();
    click.connect_pressed(move |_, _, _, _| {
        if a.palette_open() {
            a.hide_palette();
        }
    });
    shell.add_controller(click);

    // Capture phase so the web page never swallows our ctrl+hjkl & friends.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let a = app.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        if a.on_key(key, state) { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    win.add_controller(keys);

    // Fresh every launch: one empty tab, no session restore.
    app.new_tab();
    if let Some(url) = std::env::args().nth(1) {
        app.navigate(&url);
    }
    win.present();
}

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id("dev.rail.browser")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(build);
    app.run_with_args::<&str>(&[])
}

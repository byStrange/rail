mod blocklist;
mod fuzzy;
mod history;
mod menu;
mod nav;
mod pip;
mod search;
mod shot;
mod strip;
mod suspend;

use gtk::glib;
use gtk::{gdk, gio};
use gtk4 as gtk;
use history::{Completion, History, domain_key};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use strip::{OV_ZOOM, SIDE_W, ShellLayout, StackLayout, StripLayout, col_width_of, gap, stack_pitch};
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
<span><kbd>Ctrl</kbd><kbd>T</kbd></span><span>new tab</span>
<span><kbd>Ctrl</kbd><kbd>H</kbd> <kbd>L</kbd></span><span>focus left / right</span>
<span><kbd>Ctrl</kbd><kbd>1</kbd>…<kbd>9</kbd></span><span>workspaces</span>
<span><kbd>F2</kbd></span><span>name this workspace</span>
<span><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>G</kbd></span><span>group tabs by site</span>
<span><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>D</kbd></span><span>downloads</span>
<span><kbd>Ctrl</kbd><kbd>O</kbd></span><span>overview</span>
<span><kbd>Space</kbd><kbd>f</kbd><kbd>w</kbd></span><span>jump to any tab</span>
<span><kbd>/</kbd> <kbd>n</kbd> <kbd>N</kbd></span><span>find in page</span>
<span><kbd>Ctrl</kbd><kbd>P</kbd></span><span>pin tab</span>
<span><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>S</kbd></span><span>screenshot</span>
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
.tablist, .tablist row, .wslist, .wslist row { background: transparent; }
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

/* ---- workspaces ---- */
.section { color: #565f89; font-size: 10px; font-weight: 700; letter-spacing: 0.08em; margin: 2px 0 0 8px; }
.wslist row { padding: 0; margin: 1px 0; border-radius: 9px; }
.wslist row:hover { background: #151620; }
.wslist row:selected { background: #24283b; }
.wsrow { padding: 5px 8px; }
.wsrow label { color: #8089b3; font-size: 13px; }
row:selected .wsrow label { color: #c0caf5; }
.wsrow .ws-num { color: #565f89; font-size: 11px; font-weight: 700; min-width: 14px;
                 font-family: ui-monospace, monospace; }
row:selected .wsrow .ws-num { color: #7aa2f7; }
.wsrow .ws-count { color: #565f89; font-size: 11px; }

/* ---- site groups in the tab list ---- */
.tablist row.group-head { margin: 8px 0 0 0; }
.tablist row.group-head:hover { background: transparent; }
.group-head label { font-size: 11px; font-weight: 700; letter-spacing: 0.02em; }
.tabrow.in-group { margin-left: 8px; border-left: 2px solid #272a40; border-radius: 0; padding-left: 8px; }
.group-head label.gc-0 { color: #7aa2f7; } .group-head label.gc-1 { color: #bb9af7; }
.group-head label.gc-2 { color: #9ece6a; } .group-head label.gc-3 { color: #e0af68; }
.group-head label.gc-4 { color: #f7768e; } .group-head label.gc-5 { color: #7dcfff; }
.group-head label.gc-6 { color: #ff9e64; } .group-head label.gc-7 { color: #73daca; }

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
entry.palette-entry selection { background: rgba(122,162,247,0.35); color: #c0caf5; }
.palette-hint { color: #565f89; font-size: 12px; margin-top: 8px; }
.sugg, .sugg row { background: transparent; }
.sugg { margin-top: 10px; }
.sugg row { border-radius: 10px; padding: 0; }
.sugg row:hover { background: #1f2335; }
.sugg row:selected { background: #24283b; }
.sugg-row { padding: 7px 10px; }
.sugg-title { color: #c0caf5; font-size: 14px; }
.sugg-url { color: #565f89; font-size: 12px; }

/* ---- right-click menu ---- */
popover.ctx > contents { background: #1a1b26; border: 1px solid #2f334d; border-radius: 12px; padding: 6px;
                         box-shadow: 0 16px 40px rgba(0,0,0,0.5); min-width: 230px; }
button.ctx-item { background: transparent; border: none; box-shadow: none; color: #c0caf5; font-size: 13px;
                  padding: 6px 10px; border-radius: 8px; min-height: 0; }
button.ctx-item:hover, button.ctx-item:focus-visible { background: #24283b; outline: none; }
button.ctx-item:focus { outline: none; }
button.ctx-item:disabled { color: #3b4261; background: transparent; }
.ctx-hint { color: #565f89; font-size: 11px; }
separator.ctx-sep { background: #2a2e45; min-height: 1px; margin: 5px 4px; }

/* ---- screenshot bar ---- */
.shot { background: #1a1b26; border: 1px solid #2f334d; border-radius: 14px; padding: 8px 10px;
        box-shadow: 0 16px 40px rgba(0,0,0,0.5); }
.shot-title { color: #a9b1d6; font-size: 13px; font-weight: 600; margin: 0 8px; }
button.shot-btn { background: #24283b; border: none; box-shadow: none; color: #c0caf5; font-size: 13px;
                  padding: 6px 12px; border-radius: 9px; min-height: 0; }
button.shot-btn:hover { background: #2f3550; }
picture.shot-pic { border-radius: 6px; background: #0f1016; }

/* ---- tab badges in the sidebar ---- */
.tab-pin { color: #7aa2f7; -gtk-icon-size: 12px; }
button.tab-audio { background: transparent; border: none; box-shadow: none; color: #7982a9; padding: 0;
                   min-width: 22px; min-height: 22px; border-radius: 6px; }
button.tab-audio:hover { background: #2a2e45; color: #c0caf5; }
.tabrow.asleep label { color: #565f89; }

/* ---- overview (the live strip, zoomed out) ---- */
.zoomed .tab { opacity: 0.88; border-radius: 28px; }
.zoomed .tab.focused { opacity: 1; outline: 7px solid #3b4261; outline-offset: -5px; }
.zoomed.curws .tab.focused { outline-color: #7aa2f7; }
.zoomed:not(.curws) .tab { opacity: 0.6; }
.ov-hint { background: #1a1b26; color: #8089b3; border: 1px solid #2f334d; border-radius: 10px;
           padding: 8px 16px; font-size: 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.4); }

/* ---- toast ---- */
.toast { background: #1a1b26; color: #c0caf5; border: 1px solid #2f334d; border-radius: 12px;
         padding: 9px 18px; font-size: 14px; box-shadow: 0 10px 30px rgba(0,0,0,0.5); }

/* ---- downloads ---- */
button.dl-btn { background: transparent; border: none; box-shadow: none; border-radius: 9px; padding: 5px 8px;
                color: #7982a9; }
button.dl-btn:hover { background: #1f2335; }
button.dl-btn label, button.dl-btn image { color: #7982a9; font-size: 12px; }
button.dl-btn.active label, button.dl-btn.active image { color: #7aa2f7; }
.dl-panel { background: #1a1b26; border: 1px solid #2f334d; border-radius: 16px; padding: 14px 14px 10px;
            box-shadow: 0 24px 64px rgba(0,0,0,0.55); min-width: 400px; }
.dl-title { color: #c0caf5; font-weight: 700; font-size: 14px; }
.dl-empty { color: #565f89; font-size: 13px; padding: 18px 0 12px; }
.dl-row { padding: 8px 4px; border-top: 1px solid #23253a; }
.dl-name { color: #c0caf5; font-size: 13px; }
.dl-status { color: #565f89; font-size: 12px; }
.dl-status.failed { color: #f7768e; }
progressbar.dl-bar, progressbar.dl-bar trough, progressbar.dl-bar progress { min-height: 4px; }
progressbar.dl-bar trough { background: #272a40; border: none; border-radius: 4px; }
progressbar.dl-bar progress { background: #7aa2f7; border-radius: 4px; }
button.dl-act { background: transparent; border: none; box-shadow: none; min-width: 26px; min-height: 26px;
                padding: 0; border-radius: 7px; color: #7982a9; }
button.dl-act:hover { background: #2a2e45; color: #c0caf5; }
button.dl-clear { background: transparent; border: none; box-shadow: none; color: #7982a9; font-size: 12px;
                  padding: 2px 8px; border-radius: 7px; min-height: 0; }
button.dl-clear:hover { background: #2a2e45; color: #c0caf5; }
";

const MAX_WS: usize = 9;

/// What a suspended tab remembers so it can come back exactly where it was.
struct Susp {
    uri: String,
    title: String,
    favicon: Option<gdk::Texture>,
    state: Option<webkit6::WebViewSessionState>,
    scroll: (f64, f64),
}

/// Per-tab bookkeeping that WebKit itself does not keep.
struct TabState {
    pinned: Cell<bool>,
    /// The address a pinned tab is restored with on the next start.
    pin_url: RefCell<Option<String>>,
    /// The user typed into a field: unsaved input must never be thrown away.
    dirty: Cell<bool>,
    /// A find-in-page highlight is showing (Esc clears it).
    finding: Cell<bool>,
    /// Last time the tab was focused, visible, or making noise.
    last_active: Cell<Instant>,
    susp: RefCell<Option<Susp>>,
    /// Scroll position to apply once a resumed page has loaded.
    scroll_to: Cell<Option<(f64, f64)>>,
    resume_gen: Cell<u32>,
}

#[derive(Clone)]
struct Tab {
    view: WebView,
    st: Rc<TabState>,
}

impl Tab {
    fn new(view: WebView) -> Self {
        Tab {
            view,
            st: Rc::new(TabState {
                pinned: Cell::new(false),
                pin_url: RefCell::new(None),
                dirty: Cell::new(false),
                finding: Cell::new(false),
                last_active: Cell::new(Instant::now()),
                susp: RefCell::new(None),
                scroll_to: Cell::new(None),
                resume_gen: Cell::new(0),
            }),
        }
    }

    fn suspended(&self) -> bool {
        self.st.susp.borrow().is_some()
    }

    fn uri(&self) -> Option<String> {
        match &*self.st.susp.borrow() {
            Some(s) => Some(s.uri.clone()),
            None => self.view.uri().map(|u| u.to_string()),
        }
    }

    fn title(&self) -> Option<String> {
        match &*self.st.susp.borrow() {
            Some(s) => Some(s.title.clone()),
            None => self.view.title().map(|t| t.to_string()),
        }
        .filter(|t| !t.is_empty())
    }

    fn favicon(&self) -> Option<gdk::Texture> {
        match &*self.st.susp.borrow() {
            Some(s) => s.favicon.clone(),
            None => self.view.favicon(),
        }
    }
}

/// One workspace: a full, independent, endlessly scrolling strip of tabs.
struct Ws {
    name: RefCell<String>,
    strip: gtk::Box,
    layout: StripLayout,
    tabs: RefCell<Vec<Tab>>,
    focus: Cell<usize>,
    target: Cell<f64>,
    animating: Cell<bool>,
    /// Scroll position before the overview, restored when zooming back in.
    saved: Cell<f64>,
    /// Keep tabs of the same site next to each other.
    grouped: Cell<bool>,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Address,
    Rename,
    /// Fuzzy jump to any open tab.
    Tabs,
    /// Find in page.
    Find,
}

#[derive(Clone, Copy, PartialEq)]
enum DlState {
    Active,
    Done,
    Failed,
}

struct Dl {
    name: String,
    path: PathBuf,
    progress: Cell<f64>,
    state: Cell<DlState>,
    handle: RefCell<Option<webkit6::Download>>,
}

struct App {
    win: gtk::ApplicationWindow,
    shell: gtk::Box,
    shell_layout: ShellLayout,
    side: gtk::Box,
    stack: gtk::Box,
    stack_layout: StackLayout,
    stack_anim: Cell<bool>,
    wss: RefCell<Vec<Rc<Ws>>>,
    cur: Cell<usize>,
    list: gtk::ListBox,
    /// List row -> tab index (group headers map to `None`).
    rowmap: RefCell<Vec<Option<usize>>>,
    wlist: gtk::ListBox,
    pill_label: gtk::Label,
    load_bar: gtk::ProgressBar,
    toast: gtk::Label,
    toast_gen: Cell<u32>,
    // address palette
    palette: gtk::Box,
    palette_icon: gtk::Image,
    palette_hint: gtk::Label,
    entry: gtk::Entry,
    sugg: gtk::ListBox,
    sugg_urls: RefCell<Vec<String>>,
    sugg_sel: Cell<Option<usize>>,
    typed: RefCell<String>,
    completion: RefCell<Option<Completion>>,
    suppress: Cell<bool>,
    last_len: Cell<usize>,
    mode: Cell<Mode>,
    history: RefCell<History>,
    // downloads
    downloads: RefCell<Vec<Rc<Dl>>>,
    dl_panel: gtk::Box,
    dl_rows: gtk::Box,
    dl_btn: gtk::Button,
    dl_btn_label: gtk::Label,
    dl_pending: Cell<bool>,
    // overview
    ov_hint: gtk::Label,
    /// Overview is active: the strip is zoomed out and tabs ignore the pointer.
    ov: Cell<bool>,
    /// (workspace, tab) that was focused when overview opened; Esc returns here.
    ov_return: Cell<(usize, usize)>,
    target_zoom: Cell<f64>,
    side_open: Cell<bool>,
    side_gen: Cell<u32>,
    /// A just-created tab that has never navigated; Esc discards it.
    fresh: RefCell<Option<WebView>>,
    overlay: gtk::Overlay,
    /// Shared by every tab: carries the key script and the ad filter.
    ucm: webkit6::UserContentManager,
    engines: RefCell<search::Engines>,
    /// Tab-search results: (workspace, tab) per row.
    sugg_tabs: RefCell<Vec<(usize, usize)>>,
    shot: RefCell<Option<shot::Shot>>,
    pip: RefCell<Option<pip::Pip>>,
    adblock_on: Cell<bool>,
    adblock_filter: RefCell<Option<webkit6::UserContentFilter>>,
}

/// A closed tab's page can keep playing after its widget is gone (the web process
/// outlives the view), so stop the media and navigate away to tear the player down.
fn silence(view: &WebView) {
    let v = view.clone();
    view.evaluate_javascript(
        "document.querySelectorAll('video,audio').forEach(m=>{m.pause();m.removeAttribute('src');m.load()})",
        None, None, gio::Cancellable::NONE,
        move |_| v.load_uri("about:blank"),
    );
}

fn data_dir() -> PathBuf {
    let dir = glib::user_data_dir().join("rail");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn to_uri(input: &str, engines: &search::Engines) -> String {
    let s = input.trim();
    if let Some(u) = search::expand(s, engines) {
        return u;
    }
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

fn seed_of(s: &str) -> u32 {
    s.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32))
}

/// `name`, or `name (1)`, `name (2)` ... when it is already taken.
fn unique_path(dir: &Path, wanted: &str) -> PathBuf {
    let wanted = wanted.replace('/', "_");
    let wanted = if wanted.trim().is_empty() { "download".to_string() } else { wanted };
    let first = dir.join(&wanted);
    if !first.exists() {
        return first;
    }
    let p = Path::new(&wanted);
    let stem = p.file_stem().map_or(wanted.clone(), |s| s.to_string_lossy().into_owned());
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|c| !c.exists())
        .unwrap_or(first)
}

impl App {
    fn ws(&self) -> Rc<Ws> {
        self.wss.borrow()[self.cur.get()].clone()
    }

    fn find_view(&self, view: &WebView) -> Option<(usize, Rc<Ws>)> {
        self.wss
            .borrow()
            .iter()
            .enumerate()
            .find(|(_, w)| w.tabs.borrow().iter().any(|t| &t.view == view))
            .map(|(i, w)| (i, w.clone()))
    }

    fn focused_tab(&self) -> Option<Tab> {
        let ws = self.ws();
        let tabs = ws.tabs.borrow();
        tabs.get(ws.focus.get()).cloned()
    }

    fn focused_view(&self) -> Option<WebView> {
        let ws = self.ws();
        let tabs = ws.tabs.borrow();
        tabs.get(ws.focus.get()).map(|t| t.view.clone())
    }

    fn palette_open(&self) -> bool {
        gtk::prelude::WidgetExt::is_visible(&self.palette)
    }

    fn overview_open(&self) -> bool {
        self.ov.get()
    }

    // ---- strip scrolling -------------------------------------------------

    fn reveal_focus(self: &Rc<Self>) {
        let ws = self.ws();
        self.reveal_ws(&ws);
    }

    /// Normal mode: scroll the minimum needed to show the focused column.
    /// Overview: center the focused column in the zoomed-out strip.
    fn reveal_ws(self: &Rc<Self>, ws: &Rc<Ws>) {
        let vw = ws.strip.width() as f64;
        if vw <= 0.0 {
            return;
        }
        let (zoom, ov) = (self.target_zoom.get(), self.ov.get());
        let visible = vw / zoom; // strip width that fits on screen
        let g = gap(zoom);
        let tabs = ws.tabs.borrow();
        let mut x = 0.0;
        let mut span = None;
        for (i, t) in tabs.iter().enumerate() {
            let w = col_width_of(&t.view, vw);
            if i == ws.focus.get() {
                span = Some((x, x + w));
            }
            x += w + g;
        }
        let total = x - g;
        let Some((l, r)) = span else { return };
        let max = (total - visible).max(0.0);
        let t = if ov {
            if total <= visible {
                -(visible - total) / 2.0 // everything fits: center it all
            } else {
                ((l + r) / 2.0 - visible / 2.0).clamp(0.0, max)
            }
        } else {
            let mut t = ws.target.get().clamp(0.0, max);
            if l < t {
                t = l;
            } else if r > t + visible {
                t = r - visible;
            }
            t.clamp(0.0, max)
        };
        ws.target.set(t);
        drop(tabs);
        self.animate(ws);
    }

    /// Ease both the scroll offset and the zoom toward their targets.
    fn animate(self: &Rc<Self>, ws: &Rc<Ws>) {
        if ws.animating.get() {
            return;
        }
        let near = |a: f64, b: f64, eps: f64| (a - b).abs() < eps;
        let zt = self.target_zoom.get();
        if near(ws.target.get(), ws.layout.offset(), 0.5) && near(zt, ws.layout.zoom(), 0.002) {
            ws.layout.set_offset(ws.target.get());
            ws.layout.set_zoom(zt);
            ws.strip.queue_allocate();
            return;
        }
        ws.animating.set(true);
        let (app, ws2) = (self.clone(), ws.clone());
        let last = Cell::new(None::<i64>);
        ws.strip.add_tick_callback(move |strip, clock| {
            let ws = &ws2;
            let now = clock.frame_time();
            let dt = last.replace(Some(now)).map_or(1.0 / 60.0, |l| (now - l) as f64 / 1e6);
            let k = 1.0 - (-dt / 0.06).exp();
            let w = (strip.width() as f64).max(1.0);
            let (oc, ot) = (ws.layout.offset(), ws.target.get());
            let zc = ws.layout.zoom();
            let zt = app.target_zoom.get();
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
            ws.layout.set_offset(next);
            ws.layout.set_zoom(znext);
            strip.queue_allocate();
            if next == ot && znext == zt {
                ws.animating.set(false);
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    // ---- workspaces ------------------------------------------------------

    fn add_ws(self: &Rc<Self>, name: &str) -> usize {
        let layout = StripLayout::new();
        let strip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        strip.set_layout_manager(Some(layout.clone()));
        strip.set_overflow(gtk::Overflow::Hidden);
        strip.set_margin_top(8);
        strip.set_margin_bottom(8);
        strip.set_margin_end(8);
        strip.set_margin_start(6);
        let ws = Rc::new(Ws {
            name: RefCell::new(name.to_string()),
            strip: strip.clone(),
            layout: layout.clone(),
            tabs: RefCell::new(Vec::new()),
            focus: Cell::new(usize::MAX),
            target: Cell::new(0.0),
            animating: Cell::new(false),
            saved: Cell::new(0.0),
            grouped: Cell::new(false),
        });
        let (a, w) = (self.clone(), ws.clone());
        layout.connect_resized(move || a.reveal_ws(&w));

        if self.ov.get() {
            strip.add_css_class("zoomed");
            layout.set_zoom(OV_ZOOM);
        }

        self.stack.append(&strip);
        self.wss.borrow_mut().push(ws);
        self.mark_cur();
        self.wss.borrow().len() - 1
    }

    /// The current workspace's strip gets the `curws` class (overview highlights it).
    fn mark_cur(&self) {
        for (i, w) in self.wss.borrow().iter().enumerate() {
            if i == self.cur.get() {
                w.strip.add_css_class("curws");
            } else {
                w.strip.remove_css_class("curws");
            }
        }
    }

    /// Slide (and, for overview, zoom) the workspace stack toward its targets.
    fn animate_stack(self: &Rc<Self>) {
        if self.stack_anim.replace(true) {
            return;
        }
        let app = self.clone();
        let last = Cell::new(None::<i64>);
        self.stack.add_tick_callback(move |stack, clock| {
            let now = clock.frame_time();
            let dt = last.replace(Some(now)).map_or(1.0 / 60.0, |l| (now - l) as f64 / 1e6);
            let k = 1.0 - (-dt / 0.07).exp();
            let (t, p) = (app.cur.get() as f64, app.stack_layout.pos());
            let (zt, zc) = (app.target_zoom.get(), app.stack_layout.zoom());
            let mut n = p + (t - p) * k;
            let mut z = zc + (zt - zc) * k;
            let done = (t - n).abs() < 0.002 && (zt - z).abs() < 0.002;
            if done {
                n = t;
                z = zt;
            }
            app.stack_layout.set_pos(n);
            app.stack_layout.set_zoom(z);
            stack.queue_allocate();
            if done {
                app.stack_anim.set(false);
                if !app.palette_open() && !app.ov.get() {
                    if let Some(v) = app.focused_view() {
                        v.grab_focus();
                    }
                }
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn switch_ws(self: &Rc<Self>, i: usize) {
        let n = self.wss.borrow().len();
        if i >= n {
            return;
        }
        if i != self.cur.get() {
            self.palette.set_visible(false);
            self.cur.set(i);
            self.mark_cur();
            self.animate_stack();
            let ws = self.ws();
            let len = ws.tabs.borrow().len();
            if len > 0 {
                self.focus_tab(ws.focus.get().min(len - 1));
            } else if !self.ov.get() {
                self.new_tab();
            }
            self.refresh_sidebar();
        }
        let name = self.ws().name.borrow().clone();
        self.toast(&format!("{} · {name}", i + 1));
    }

    /// Ctrl+1..9: go to workspace `i`; the next free number creates it.
    fn goto_ws(self: &Rc<Self>, i: usize) {
        let n = self.wss.borrow().len();
        if i == n && n < MAX_WS {
            self.add_ws(&format!("Workspace {}", i + 1));
            self.save_workspaces();
        }
        self.switch_ws(i);
    }

    /// Move the focused tab to workspace `i` (creating it if it is the next one).
    fn send_tab(self: &Rc<Self>, i: usize) {
        let n = self.wss.borrow().len();
        if i > n || i >= MAX_WS || i == self.cur.get() {
            return;
        }
        let Some(view) = self.focused_view() else { return };
        if i == n {
            self.add_ws(&format!("Workspace {}", i + 1));
            self.save_workspaces();
        }
        let src = self.ws();
        let dst = self.wss.borrow()[i].clone();
        let idx = src.focus.get();
        src.strip.remove(&view);
        let tab = src.tabs.borrow_mut().remove(idx);
        view.remove_css_class("focused");
        if tab.st.pinned.replace(false) {
            view.remove_css_class("pinned");
        }
        dst.strip.append(&view);
        dst.tabs.borrow_mut().push(tab);
        dst.focus.set(dst.tabs.borrow().len() - 1);
        if self.fresh.borrow().as_ref() == Some(&view) {
            *self.fresh.borrow_mut() = None;
        }
        let left = src.tabs.borrow().len();
        src.focus.set(if left == 0 { usize::MAX } else { idx.saturating_sub(1).min(left - 1) });
        self.reveal_ws(&src);
        self.save_pinned();
        // Always follow the moved tab, overview or not.
        self.switch_ws(i);
    }

    fn close_ws(self: &Rc<Self>) {
        let n = self.wss.borrow().len();
        if n <= 1 {
            self.toast("The only workspace can't be closed");
            return;
        }
        self.exit_overview();
        self.palette.set_visible(false);
        let i = self.cur.get();
        let ws = self.wss.borrow_mut().remove(i);
        let name = ws.name.borrow().clone();
        if let Some(f) = self.fresh.borrow().clone() {
            if ws.tabs.borrow().iter().any(|t| t.view == f) {
                *self.fresh.borrow_mut() = None;
            }
        }
        for t in ws.tabs.borrow().iter() {
            silence(&t.view);
        }
        self.stack.remove(&ws.strip);
        let new = i.saturating_sub(1);
        self.cur.set(new);
        self.mark_cur();
        self.stack_layout.set_pos(new as f64);
        self.stack.queue_allocate();
        self.save_workspaces();
        let w = self.ws();
        let len = w.tabs.borrow().len();
        if len == 0 {
            self.new_tab();
        } else {
            self.focus_tab(w.focus.get().min(len - 1));
        }
        self.refresh_sidebar();
        self.toast(&format!("Closed workspace “{name}”"));
    }

    fn save_workspaces(&self) {
        let names: Vec<String> = self.wss.borrow().iter().map(|w| w.name.borrow().replace('\n', " ")).collect();
        let _ = std::fs::write(data_dir().join("workspaces.txt"), names.join("\n"));
    }

    // ---- grouping by site ------------------------------------------------

    /// Reorder tabs so each site's tabs are adjacent (groups ordered by first
    /// appearance, tabs inside a group keep their order). True if anything moved.
    fn group_ws(self: &Rc<Self>, ws: &Rc<Ws>) -> bool {
        let n = ws.tabs.borrow().len();
        if n < 2 {
            return false;
        }
        let keys: Vec<Option<String>> =
            ws.tabs.borrow().iter().map(|t| t.uri().and_then(|u| domain_key(&u))).collect();
        let pinned: Vec<bool> = ws.tabs.borrow().iter().map(|t| t.st.pinned.get()).collect();
        // Rank of a tab = position of the first tab of its site. Tabs without a site stand alone.
        let rank: Vec<usize> = (0..n)
            .map(|i| match &keys[i] {
                Some(k) => keys.iter().position(|o| o.as_ref() == Some(k)).unwrap_or(i),
                None => i,
            })
            .collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| (!pinned[i], rank[i], i));
        if order.iter().enumerate().all(|(a, &b)| a == b) {
            return false;
        }
        let old = ws.tabs.borrow().clone();
        let new: Vec<Tab> = order.iter().map(|&i| old[i].clone()).collect();
        self.set_order(ws, new);
        true
    }

    /// Put the workspace's tabs in `new` order (same tabs), keeping the focused one focused.
    fn set_order(self: &Rc<Self>, ws: &Rc<Ws>, new: Vec<Tab>) {
        let focused = ws.tabs.borrow().get(ws.focus.get()).map(|t| t.view.clone());
        let mut prev: Option<&WebView> = None;
        for t in &new {
            ws.strip.reorder_child_after(&t.view, prev);
            prev = Some(&t.view);
        }
        if let Some(f) = focused {
            if let Some(p) = new.iter().position(|t| t.view == f) {
                ws.focus.set(p);
            }
        }
        *ws.tabs.borrow_mut() = new;
        ws.strip.queue_allocate();
        if Rc::ptr_eq(&self.ws(), ws) {
            self.reveal_ws(ws);
            self.refresh_sidebar();
        }
    }

    fn toggle_group(self: &Rc<Self>) {
        let ws = self.ws();
        let on = !ws.grouped.get();
        ws.grouped.set(on);
        if on {
            self.group_ws(&ws);
            self.toast("Tabs grouped by site");
        } else {
            self.toast("Grouping off");
        }
    }

    // ---- tabs ------------------------------------------------------------

    fn new_tab(self: &Rc<Self>) {
        self.exit_overview();
        let view = self.new_view();
        view.load_html(START_PAGE, None);
        *self.fresh.borrow_mut() = Some(view.clone());
        self.add_view(view);
        self.show_palette();
    }

    fn new_view(&self) -> WebView {
        let view = WebView::builder().user_content_manager(&self.ucm).build();
        if std::env::var_os("RAIL_DEBUG_CONSOLE").is_some() {
            if let Some(s) = webkit6::prelude::WebViewExt::settings(&view) {
                s.set_enable_write_console_messages_to_stdout(true);
            }
        }
        view
    }

    fn add_view(self: &Rc<Self>, view: WebView) {
        view.add_css_class("tab");
        view.set_overflow(gtk::Overflow::Hidden);
        view.set_can_target(!self.ov.get());
        let ws = self.ws();
        let idx = {
            let tabs = ws.tabs.borrow();
            let f = ws.focus.get();
            let pinned = tabs.iter().take_while(|t| t.st.pinned.get()).count();
            let after = if tabs.is_empty() { 0 } else if f < tabs.len() { f + 1 } else { tabs.len() };
            after.max(pinned) // never in the middle of the pinned block
        };
        {
            let tabs = ws.tabs.borrow();
            match idx {
                0 => ws.strip.prepend(&view),
                _ => ws.strip.insert_child_after(&view, Some(&tabs[idx - 1].view)),
            }
        }
        ws.tabs.borrow_mut().insert(idx, Tab::new(view.clone()));

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
        // Sound (or its mute switch) changing is both worth showing and a sign of life.
        let (app, v) = (self.clone(), view.clone());
        view.connect_is_playing_audio_notify(move |_| {
            if let Some(t) = app.tab_of(&v) {
                t.st.last_active.set(Instant::now());
            }
            app.refresh_sidebar();
        });
        let app = self.clone();
        view.connect_is_muted_notify(move |_| app.refresh_sidebar());
        if let Some(fc) = view.find_controller() {
            let app = self.clone();
            fc.connect_counted_matches(move |_, n| app.find_result(n));
            let app = self.clone();
            fc.connect_failed_to_find_text(move |_| app.find_result(0));
        }
        // A crashed or killed web process leaves a dead page; bring it back.
        view.connect_web_process_terminated(|v, reason| {
            eprintln!("rail: web process terminated ({reason:?}), reloading");
            v.reload();
        });
        let app = self.clone();
        view.connect_estimated_load_progress_notify(move |_| app.update_load_bar());
        let app = self.clone();
        view.connect_load_changed(move |v, ev| {
            let tab = app.tab_of(v);
            if ev == webkit6::LoadEvent::Committed {
                if let Some(t) = &tab {
                    t.st.dirty.set(false); // a new page has no unsaved input
                    t.st.finding.set(false);
                }
            }
            if ev != webkit6::LoadEvent::Finished || tab.as_ref().is_some_and(|t| t.suspended()) {
                return; // a suspended tab only shows its placeholder: not a visit
            }
            if let Some((x, y)) = tab.as_ref().and_then(|t| t.st.scroll_to.take()) {
                v.evaluate_javascript(&format!("window.scrollTo({x},{y})"), None, None, gio::Cancellable::NONE, |_| {});
            }
            if let Some(uri) = v.uri() {
                app.history.borrow_mut().visit(&uri, v.title().as_deref().unwrap_or(""));
            }
            if let Some((_, ws)) = app.find_view(v) {
                if ws.grouped.get() {
                    app.group_ws(&ws);
                }
            }
        });
        let app = self.clone();
        view.connect_create(move |v, _| {
            let nv = WebView::builder().related_view(v).user_content_manager(&app.ucm).build();
            app.add_view(nv.clone());
            Some(nv.upcast())
        });
        let app = self.clone();
        view.connect_close(move |v| app.close_view(v));
        // Clicking a tab makes it the focused one (the page still gets the click).
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let app = self.clone();
        let v = view.clone();
        click.connect_pressed(move |g, _, _, _| {
            g.set_state(gtk::EventSequenceState::None);
            if let Some((_, ws)) = app.find_view(&v) {
                let i = ws.tabs.borrow().iter().position(|t| t.view == v);
                if let Some(i) = i.filter(|&i| ws.focus.get() != i) {
                    app.focus_tab(i);
                }
            }
        });
        view.add_controller(click);
        // Hovering a tab focuses it too (no scrolling).
        let motion = gtk::EventControllerMotion::new();
        let pointer = Rc::new(Cell::new((0.0, 0.0)));
        let p = pointer.clone();
        motion.connect_motion(move |_, x, y| p.set((x, y)));
        let app = self.clone();
        let v = view.clone();
        motion.connect_enter(move |_, _, _| {
            if app.ov.get() || app.palette_open() {
                return;
            }
            let ws = app.ws();
            let i = ws.tabs.borrow().iter().position(|t| t.view == v);
            if let Some(i) = i.filter(|&i| ws.focus.get() != i) {
                app.focus_tab_opts(i, false);
            }
        });
        view.add_controller(motion);
        // Our own right-click menu replaces WebKit's.
        let app = self.clone();
        view.connect_context_menu(move |v, _, hit| {
            app.show_menu(v, hit, pointer.get());
            true
        });
        // Own the file picker (<input type=file>) instead of relying on WebKit's default.
        view.connect_run_file_chooser(|v, req| {
            let dialog = gtk::FileDialog::new();
            if let Some(f) = req.mime_types_filter() {
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                filters.append(&f);
                dialog.set_filters(Some(&filters));
            }
            let win = v.root().and_downcast::<gtk::Window>();
            let multiple = req.selects_multiple();
            let req = req.clone();
            let done = move |res: Result<Vec<gio::File>, gtk::glib::Error>| match res {
                Ok(files) => {
                    let paths: Vec<String> = files.iter().filter_map(|f| f.path()).map(|p| p.to_string_lossy().into_owned()).collect();
                    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
                    req.select_files(&refs);
                }
                Err(_) => req.cancel(),
            };
            if multiple {
                dialog.open_multiple(win.as_ref(), gio::Cancellable::NONE, move |r| {
                    done(r.map(|l| l.iter::<gio::File>().flatten().collect()))
                });
            } else {
                dialog.open(win.as_ref(), gio::Cancellable::NONE, move |r| done(r.map(|f| vec![f])));
            }
            true
        });

        self.focus_tab(idx);
    }

    fn close_view(self: &Rc<Self>, view: &WebView) {
        if self.pip.borrow().as_ref().is_some_and(|p| &p.tab.view == view) {
            self.pip_discard(); // the page closed itself while popped out
            return;
        }
        let was_pinned = self.tab_of(view).is_some_and(|t| t.st.pinned.get());
        self.detach_view(view);
        silence(view);
        if was_pinned {
            self.save_pinned();
        }
    }

    /// Take a tab out of its strip (without closing the page) and fix up focus.
    /// Returns (workspace index, tab index, the tab).
    fn detach_view(self: &Rc<Self>, view: &WebView) -> Option<(usize, usize, Tab)> {
        let (wi, ws) = self.find_view(view)?;
        let idx = ws.tabs.borrow().iter().position(|t| &t.view == view)?;
        ws.strip.remove(view);
        let tab = ws.tabs.borrow_mut().remove(idx);
        if self.fresh.borrow().as_ref() == Some(view) {
            *self.fresh.borrow_mut() = None;
        }
        let len = ws.tabs.borrow().len();
        if wi != self.cur.get() {
            let f = ws.focus.get();
            ws.focus.set(if len == 0 { usize::MAX } else if f > idx { f - 1 } else { f.min(len - 1) });
            self.refresh_sidebar();
            return Some((wi, idx, tab));
        }
        ws.focus.set(usize::MAX);
        if len == 0 {
            self.new_tab();
        } else {
            self.focus_tab(idx.saturating_sub(1).min(len - 1));
        }
        Some((wi, idx, tab))
    }

    fn tab_of(&self, view: &WebView) -> Option<Tab> {
        self.wss.borrow().iter().find_map(|w| w.tabs.borrow().iter().find(|t| &t.view == view).cloned())
    }

    fn focus_tab(self: &Rc<Self>, idx: usize) {
        self.focus_tab_opts(idx, true);
    }

    /// `reveal` scrolls the tab into view; hover-focus skips it so the strip doesn't slide under the pointer.
    fn focus_tab_opts(self: &Rc<Self>, idx: usize, reveal: bool) {
        let ws = self.ws();
        if idx >= ws.tabs.borrow().len() {
            return;
        }
        ws.focus.set(idx);
        for (i, t) in ws.tabs.borrow().iter().enumerate() {
            if i == idx {
                t.view.add_css_class("focused");
            } else {
                t.view.remove_css_class("focused");
            }
        }
        let tab = ws.tabs.borrow()[idx].clone();
        tab.st.last_active.set(Instant::now());
        if tab.suspended() {
            self.schedule_resume(&tab, if reveal { 120 } else { 700 });
        }
        if !tab.view.is_loading() {
            self.load_bar.set_visible(false);
        }
        self.update_load_bar();
        if reveal {
            self.reveal_focus();
        }
        self.refresh_sidebar();
        if !self.palette_open() && !self.ov.get() {
            ws.tabs.borrow()[idx].view.grab_focus();
        }
    }

    fn move_tab(self: &Rc<Self>, dir: isize) {
        let ws = self.ws();
        let i = ws.focus.get();
        let Some(j) = i.checked_add_signed(dir).filter(|&j| j < ws.tabs.borrow().len()) else { return };
        // The pinned block stays in front: tabs move within their own side of it.
        if ws.tabs.borrow()[i].st.pinned.get() != ws.tabs.borrow()[j].st.pinned.get() {
            return;
        }
        ws.tabs.borrow_mut().swap(i, j);
        let tabs = ws.tabs.borrow();
        let mut prev: Option<&WebView> = None;
        for t in tabs.iter() {
            ws.strip.reorder_child_after(&t.view, prev);
            prev = Some(&t.view);
        }
        drop(tabs);
        ws.focus.set(j);
        ws.strip.queue_allocate();
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
        self.ws().strip.queue_allocate();
        self.reveal_focus();
    }

    // ---- sidebar ---------------------------------------------------------

    fn tab_label(t: &Tab) -> String {
        match t.title() {
            Some(s) => s,
            None => t
                .uri()
                .filter(|u| !u.starts_with("about:"))
                .map_or("New tab".into(), |u| host_of(&u).unwrap_or(u)),
        }
    }

    fn tab_icon(t: &Tab, size: i32) -> gtk::Widget {
        if t.view.is_loading() && !t.suspended() {
            let s = gtk::Spinner::new();
            s.set_size_request(size, size);
            s.start();
            return s.upcast();
        }
        if let Some(f) = t.favicon() {
            let img = gtk::Image::from_paintable(Some(&f));
            img.set_pixel_size(size);
            return img.upcast();
        }
        // No favicon yet: a colored letter badge, stable per site.
        let label = Self::tab_label(t);
        let letter = label.chars().find(|c| c.is_alphanumeric()).unwrap_or('·');
        let badge = gtk::Label::new(Some(&letter.to_uppercase().to_string()));
        badge.add_css_class("fav");
        badge.add_css_class(&format!("fav-{}", seed_of(&label) % 8));
        badge.set_size_request(size, size);
        badge.set_valign(gtk::Align::Center);
        badge.upcast()
    }

    fn refresh_sidebar(self: &Rc<Self>) {
        self.refresh_workspace_list();
        let ws = self.ws();

        while let Some(r) = self.list.row_at_index(0) {
            self.list.remove(&r);
        }
        let mut map: Vec<Option<usize>> = Vec::new();
        let tabs = ws.tabs.borrow();
        let keys: Vec<Option<String>> = tabs.iter().map(|t| t.uri().and_then(|u| domain_key(&u))).collect();
        let mut i = 0;
        while i < tabs.len() {
            let mut j = i + 1;
            while j < tabs.len() && keys[i].is_some() && keys[j] == keys[i] {
                j += 1;
            }
            let group = (j - i >= 2).then(|| keys[i].clone().unwrap_or_default());
            let color = group.as_ref().map(|k| seed_of(k) % 8);
            if let (Some(k), Some(c)) = (&group, color) {
                let head = gtk::Label::new(Some(&format!("{k}  ·  {}", j - i)));
                head.add_css_class(&format!("gc-{c}"));
                head.set_xalign(0.0);
                head.set_margin_start(10);
                head.set_ellipsize(gtk::pango::EllipsizeMode::End);
                let row = gtk::ListBoxRow::new();
                row.add_css_class("group-head");
                row.set_child(Some(&head));
                row.set_selectable(false);
                row.set_activatable(false);
                self.list.append(&row);
                map.push(None);
            }
            for k in i..j {
                let t = &tabs[k];
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                row.add_css_class("tabrow");
                if group.is_some() {
                    row.add_css_class("in-group");
                }
                if t.suspended() {
                    row.add_css_class("asleep");
                }
                row.append(&Self::tab_icon(t, 16));
                let label = gtk::Label::new(Some(&Self::tab_label(t)));
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_width_chars(1);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                row.append(&label);
                if t.st.pinned.get() {
                    let pin = gtk::Image::from_icon_name("view-pin-symbolic");
                    pin.add_css_class("tab-pin");
                    row.append(&pin);
                }
                if t.view.is_muted() || t.view.is_playing_audio() {
                    let icon = if t.view.is_muted() { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" };
                    let snd = gtk::Button::from_icon_name(icon);
                    snd.add_css_class("tab-audio");
                    snd.set_valign(gtk::Align::Center);
                    snd.set_tooltip_text(Some("Mute / unmute tab  (Ctrl+M)"));
                    let v = t.view.clone();
                    snd.connect_clicked(move |_| v.set_is_muted(!v.is_muted()));
                    row.append(&snd);
                }
                let close = gtk::Button::from_icon_name("window-close-symbolic");
                close.add_css_class("tab-close");
                close.set_valign(gtk::Align::Center);
                let (app, v) = (self.clone(), t.view.clone());
                close.connect_clicked(move |_| app.close_view(&v));
                row.append(&close);
                self.list.append(&row);
                map.push(Some(k));
            }
            i = j;
        }
        drop(tabs);
        let sel = map.iter().position(|m| *m == Some(ws.focus.get()));
        *self.rowmap.borrow_mut() = map;
        if let Some(row) = sel.and_then(|p| self.list.row_at_index(p as i32)) {
            self.list.select_row(Some(&row));
        }
        let text = match self.focused_tab().and_then(|t| t.uri()) {
            Some(u) if !u.starts_with("about:") => host_of(&u).unwrap_or(u),
            _ => "Search or enter address".into(),
        };
        self.pill_label.set_text(&text);
    }

    fn refresh_workspace_list(&self) {
        while let Some(r) = self.wlist.row_at_index(0) {
            self.wlist.remove(&r);
        }
        for (i, w) in self.wss.borrow().iter().enumerate() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("wsrow");
            let num = gtk::Label::new(Some(&(i + 1).to_string()));
            num.add_css_class("ws-num");
            row.append(&num);
            let name = gtk::Label::new(Some(&w.name.borrow()));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_width_chars(1);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&name);
            let n = w.tabs.borrow().len();
            if n > 0 {
                let count = gtk::Label::new(Some(&n.to_string()));
                count.add_css_class("ws-count");
                row.append(&count);
            }
            self.wlist.append(&row);
        }
        if let Some(row) = self.wlist.row_at_index(self.cur.get() as i32) {
            self.wlist.select_row(Some(&row));
        }
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

    // ---- toast -----------------------------------------------------------

    fn toast(self: &Rc<Self>, text: &str) {
        self.toast.set_text(text);
        self.toast.set_visible(true);
        let g = self.toast_gen.get().wrapping_add(1);
        self.toast_gen.set(g);
        let app = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(1700), move || {
            if app.toast_gen.get() == g {
                app.toast.set_visible(false);
            }
        });
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
        self.open_palette(Mode::Address);
    }

    fn open_palette(&self, mode: Mode) {
        self.mode.set(mode);
        let text = match mode {
            Mode::Address => self
                .focused_tab()
                .and_then(|t| t.uri())
                .filter(|u| !u.starts_with("about:"))
                .unwrap_or_default(),
            Mode::Tabs => String::new(),
            Mode::Find => self
                .focused_view()
                .and_then(|v| v.find_controller())
                .and_then(|f| f.search_text())
                .map(|t| t.to_string())
                .unwrap_or_default(),
            Mode::Rename => self.ws().name.borrow().clone(),
        };
        self.suppress.set(true);
        self.entry.set_text(&text);
        self.suppress.set(false);
        self.last_len.set(text.len());
        *self.completion.borrow_mut() = None;
        match mode {
            Mode::Address => {
                self.entry.set_placeholder_text(Some("Search or enter address"));
                self.palette_icon.set_icon_name(Some("system-search-symbolic"));
                self.palette_hint.set_text("Enter  go      ↑ ↓  history      Esc  cancel");
                // Prefilled with the current URL -> no list; an empty bar -> most used sites.
                self.update_suggestions(if text.is_empty() { "" } else { "\0" });
            }
            Mode::Rename => {
                self.entry.set_placeholder_text(Some("Name this workspace"));
                self.palette_icon.set_icon_name(Some("document-edit-symbolic"));
                self.palette_hint.set_text("Enter  save      Esc  cancel");
                self.update_suggestions("\0");
            }
            Mode::Tabs => {
                self.entry.set_placeholder_text(Some("Jump to tab"));
                self.palette_icon.set_icon_name(Some("view-list-symbolic"));
                self.palette_hint.set_text("Enter  jump      ↑ ↓  select      Esc  cancel");
                self.update_suggestions("\0");
                self.update_tab_results("");
            }
            Mode::Find => {
                self.entry.set_placeholder_text(Some("Find in page"));
                self.palette_icon.set_icon_name(Some("edit-find-symbolic"));
                self.palette_hint.set_text("Enter  keep      ↑ ↓  next / previous      Esc  clear");
                self.update_suggestions("\0");
            }
        }
        self.palette.set_visible(true);
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
    }

    fn hide_palette(&self) {
        self.palette.set_visible(false);
        self.mode.set(Mode::Address);
        self.clear_suggestions();
        if let Some(v) = self.focused_view() {
            v.grab_focus();
        }
    }

    /// Esc on the palette: close it, and also drop a tab that was never used.
    fn cancel_palette(self: &Rc<Self>) {
        let mode = self.mode.get();
        let rename = mode != Mode::Address;
        let view = self.focused_view();
        let fresh = self.fresh.borrow().clone();
        if mode == Mode::Find {
            self.clear_find();
        }
        self.hide_palette();
        if rename {
            return;
        }
        let ws = self.ws();
        if let (Some(v), Some(f)) = (view, fresh) {
            if v == f && ws.tabs.borrow().len() > 1 {
                self.close_view(&v);
            }
        }
    }

    fn clear_suggestions(&self) {
        while let Some(r) = self.sugg.row_at_index(0) {
            self.sugg.remove(&r);
        }
        self.sugg_urls.borrow_mut().clear();
        self.sugg_tabs.borrow_mut().clear();
        self.sugg_sel.set(None);
        self.sugg.set_visible(false);
    }

    /// Fill the list under the entry with matching history. `"\0"` means "none".
    fn update_suggestions(&self, text: &str) {
        self.clear_suggestions();
        if self.mode.get() != Mode::Address || text == "\0" {
            self.typed.borrow_mut().clear();
            if self.mode.get() == Mode::Address {
                self.palette_hint.set_text("Enter  go      ↑ ↓  history      Esc  cancel");
            }
            return;
        }
        self.palette_hint.set_text("Enter  go      ↑ ↓  history      Esc  cancel");
        if let Some(u) = search::expand(text, &self.engines.borrow()) {
            self.palette_hint.set_text(&format!("Enter  search {}      Esc  cancel", host_of(&u).unwrap_or(u)));
        }
        *self.typed.borrow_mut() = text.to_string();
        let h = self.history.borrow();
        let mut urls = self.sugg_urls.borrow_mut();
        for e in h.suggest(text, 6) {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 1);
            row.add_css_class("sugg-row");
            let title = if e.title.is_empty() { history::strip(&e.url).to_string() } else { e.title.clone() };
            let t = gtk::Label::new(Some(&title));
            t.add_css_class("sugg-title");
            t.set_xalign(0.0);
            t.set_ellipsize(gtk::pango::EllipsizeMode::End);
            t.set_width_chars(1);
            t.set_max_width_chars(1);
            let u = gtk::Label::new(Some(history::strip(&e.url)));
            u.add_css_class("sugg-url");
            u.set_xalign(0.0);
            u.set_ellipsize(gtk::pango::EllipsizeMode::End);
            u.set_width_chars(1);
            u.set_max_width_chars(1);
            row.append(&t);
            row.append(&u);
            self.sugg.append(&row);
            urls.push(e.url.clone());
        }
        self.sugg.set_visible(!urls.is_empty());
    }

    /// ↑ / ↓ in the address bar walk the suggestions (and back to what was typed).
    fn sugg_move(&self, dir: isize) {
        let n = self.sugg_urls.borrow().len();
        if n == 0 {
            return;
        }
        let next = match (self.sugg_sel.get(), dir) {
            (None, d) if d > 0 => Some(0),
            (None, _) => Some(n - 1),
            (Some(i), d) if d > 0 => (i + 1 < n).then_some(i + 1),
            (Some(i), _) => i.checked_sub(1),
        };
        self.sugg_sel.set(next);
        let text = match next {
            Some(i) => {
                if let Some(row) = self.sugg.row_at_index(i as i32) {
                    self.sugg.select_row(Some(&row));
                }
                self.sugg_urls.borrow()[i].clone()
            }
            None => {
                self.sugg.unselect_all();
                self.typed.borrow().clone()
            }
        };
        self.suppress.set(true);
        self.entry.set_text(&text);
        self.entry.set_position(-1);
        self.suppress.set(false);
        self.last_len.set(text.len());
        *self.completion.borrow_mut() = None;
    }

    /// The entry text changed by typing: refresh suggestions and complete inline.
    fn on_typed(self: &Rc<Self>) {
        if self.suppress.get() {
            return;
        }
        match self.mode.get() {
            Mode::Address => {}
            Mode::Tabs => {
                let q = self.entry.text().to_string();
                self.update_tab_results(&q);
                return;
            }
            Mode::Find => {
                let q = self.entry.text().to_string();
                self.find_text(&q);
                return;
            }
            Mode::Rename => return,
        }
        let text = self.entry.text().to_string();
        let grew = text.len() > self.last_len.replace(text.len());
        *self.completion.borrow_mut() = None;
        self.update_suggestions(&text);
        if !grew {
            return; // deleting: never fight the user
        }
        // GTK moves the cursor after it reports the change, so look at it a tick later.
        let app = self.clone();
        glib::idle_add_local_once(move || {
            if app.suppress.get() || app.entry.text() != text || app.entry.position() as usize != text.chars().count() {
                return;
            }
            let Some(c) = app.history.borrow().complete(&text) else { return };
            let typed_chars = text.chars().count() as i32;
            app.suppress.set(true);
            app.entry.set_text(&c.shown);
            app.entry.select_region(typed_chars, -1);
            app.suppress.set(false);
            app.last_len.set(c.shown.len());
            *app.completion.borrow_mut() = Some(c);
        });
    }

    fn accept(self: &Rc<Self>) {
        let text = self.entry.text().to_string();
        match self.mode.get() {
            Mode::Tabs => {
                let pick = self.sugg_sel.get().unwrap_or(0);
                let hit = self.sugg_tabs.borrow().get(pick).copied();
                self.hide_palette();
                if let Some((wi, ti)) = hit {
                    self.jump_to(wi, ti);
                }
                return;
            }
            Mode::Find => {
                self.hide_palette();
                return;
            }
            _ => {}
        }
        if self.mode.get() == Mode::Rename {
            let name = text.trim();
            if !name.is_empty() {
                *self.ws().name.borrow_mut() = name.to_string();
                self.save_workspaces();
                self.refresh_sidebar();
                self.toast(&format!("Workspace named “{name}”"));
            }
            self.hide_palette();
            return;
        }
        let target = self.completion.borrow().as_ref().filter(|c| c.shown == text).map(|c| c.target.clone());
        self.navigate(&target.unwrap_or(text));
    }

    fn navigate(self: &Rc<Self>, text: &str) {
        if !text.trim().is_empty() {
            if let Some(v) = self.focused_view() {
                let uri = to_uri(text, &self.engines.borrow());
                self.pill_label.set_text(&host_of(&uri).unwrap_or(uri.clone()));
                v.load_uri(&uri);
                *self.fresh.borrow_mut() = None;
            }
        }
        self.hide_palette();
    }

    // ---- downloads -------------------------------------------------------

    fn on_download(self: &Rc<Self>, d: &webkit6::Download) {
        let app = self.clone();
        d.connect_decide_destination(move |d, suggested| {
            let dir = glib::user_special_dir(glib::UserDirectory::Downloads)
                .unwrap_or_else(|| glib::home_dir().join("Downloads"));
            let _ = std::fs::create_dir_all(&dir);
            let path = unique_path(&dir, suggested);
            d.set_destination(&path.to_string_lossy());
            let item = Rc::new(Dl {
                name: path.file_name().map_or(suggested.to_string(), |n| n.to_string_lossy().into_owned()),
                path,
                progress: Cell::new(0.0),
                state: Cell::new(DlState::Active),
                handle: RefCell::new(Some(d.clone())),
            });
            app.downloads.borrow_mut().insert(0, item.clone());
            // Typed a file URL into a tab? That tab never shows a page: don't leave it blank.
            if let (Some(v), Some(req)) = (d.web_view(), d.request()) {
                let cur = v.uri().map(|u| u.to_string()).unwrap_or_default();
                if cur.is_empty() || cur == "about:blank" || Some(cur.into()) == req.uri() {
                    let single = app.find_view(&v).is_some_and(|(_, w)| w.tabs.borrow().len() == 1);
                    if single {
                        v.load_html(START_PAGE, None);
                    } else {
                        app.close_view(&v);
                    }
                }
            }
            app.toast(&format!("Downloading {}", item.name));
            app.dl_changed();

            let (a, it) = (app.clone(), item.clone());
            d.connect_estimated_progress_notify(move |d| {
                it.progress.set(d.estimated_progress());
                a.dl_changed();
            });
            let (a, it) = (app.clone(), item.clone());
            d.connect_failed(move |_, _| {
                it.state.set(DlState::Failed);
                *it.handle.borrow_mut() = None;
                a.toast(&format!("Download failed: {}", it.name));
                a.dl_changed();
            });
            let (a, it) = (app.clone(), item.clone());
            d.connect_finished(move |_| {
                if it.state.get() == DlState::Active {
                    it.state.set(DlState::Done);
                    it.progress.set(1.0);
                    a.toast(&format!("Downloaded {}", it.name));
                    a.save_downloads();
                }
                *it.handle.borrow_mut() = None;
                a.dl_changed();
            });
            true
        });
    }

    fn save_downloads(&self) {
        let lines: Vec<String> = self
            .downloads
            .borrow()
            .iter()
            .rev()
            .filter(|d| d.state.get() == DlState::Done)
            .map(|d| d.path.to_string_lossy().replace('\n', " "))
            .collect();
        let _ = std::fs::write(data_dir().join("downloads.txt"), lines.join("\n"));
    }

    /// Coalesce bursts of progress updates into one redraw.
    fn dl_changed(self: &Rc<Self>) {
        if self.dl_pending.replace(true) {
            return;
        }
        let app = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(150), move || {
            app.dl_pending.set(false);
            app.refresh_downloads();
        });
    }

    fn downloads_open(&self) -> bool {
        gtk::prelude::WidgetExt::is_visible(&self.dl_panel)
    }

    fn toggle_downloads(self: &Rc<Self>) {
        if self.downloads_open() {
            self.dl_panel.set_visible(false);
            if let Some(v) = self.focused_view() {
                v.grab_focus();
            }
        } else {
            self.dl_panel.set_visible(true);
            self.refresh_downloads();
        }
    }

    fn refresh_downloads(self: &Rc<Self>) {
        let active = self.downloads.borrow().iter().filter(|d| d.state.get() == DlState::Active).count();
        let total = self.downloads.borrow().len();
        self.dl_btn_label.set_text(&match (active, total) {
            (0, 0) => "Downloads".to_string(),
            (0, n) => format!("Downloads · {n}"),
            (a, _) => format!("Downloading {a}…"),
        });
        if active > 0 {
            self.dl_btn.add_css_class("active");
        } else {
            self.dl_btn.remove_css_class("active");
        }
        if !self.downloads_open() {
            return;
        }
        while let Some(c) = self.dl_rows.first_child() {
            self.dl_rows.remove(&c);
        }
        let items = self.downloads.borrow().clone();
        if items.is_empty() {
            let l = gtk::Label::new(Some("No downloads yet"));
            l.add_css_class("dl-empty");
            self.dl_rows.append(&l);
        }
        for d in items {
            let missing = d.state.get() == DlState::Done && !d.path.exists();
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.add_css_class("dl-row");
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            let info = gtk::Box::new(gtk::Orientation::Vertical, 2);
            info.set_hexpand(true);
            let name = gtk::Label::new(Some(&d.name));
            name.add_css_class("dl-name");
            name.set_xalign(0.0);
            name.set_width_chars(1);
            name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            let status = gtk::Label::new(None);
            status.add_css_class("dl-status");
            status.set_xalign(0.0);
            status.set_text(&match d.state.get() {
                _ if missing => "File was moved or deleted".to_string(),
                DlState::Active => format!("{:.0}%", d.progress.get() * 100.0),
                DlState::Done => d.path.parent().map_or("Done".into(), |p| format!("Done · {}", p.display())),
                DlState::Failed => "Failed".to_string(),
            });
            if d.state.get() == DlState::Failed {
                status.add_css_class("failed");
            }
            status.set_ellipsize(gtk::pango::EllipsizeMode::Start);
            status.set_width_chars(1);
            info.append(&name);
            info.append(&status);
            top.append(&info);
            let act = |icon: &str, tip: &str| {
                let b = gtk::Button::from_icon_name(icon);
                b.add_css_class("dl-act");
                b.set_tooltip_text(Some(tip));
                b.set_valign(gtk::Align::Center);
                b
            };
            match d.state.get() {
                DlState::Active => {
                    let b = act("window-close-symbolic", "Cancel");
                    let dd = d.clone();
                    b.connect_clicked(move |_| {
                        if let Some(h) = dd.handle.borrow().as_ref() {
                            h.cancel();
                        }
                    });
                    top.append(&b);
                }
                DlState::Done if !missing => {
                    let b = act("text-x-generic-symbolic", "Open");
                    let p = d.path.clone();
                    b.connect_clicked(move |_| {
                        let _ = gio::AppInfo::launch_default_for_uri(
                            &gio::File::for_path(&p).uri(),
                            None::<&gio::AppLaunchContext>,
                        );
                    });
                    top.append(&b);
                    let b = act("folder-open-symbolic", "Show in folder");
                    let p = d.path.clone();
                    b.connect_clicked(move |_| {
                        if let Some(dir) = p.parent() {
                            let _ = gio::AppInfo::launch_default_for_uri(
                                &gio::File::for_path(dir).uri(),
                                None::<&gio::AppLaunchContext>,
                            );
                        }
                    });
                    top.append(&b);
                }
                _ => {}
            }
            row.append(&top);
            if d.state.get() == DlState::Active {
                let bar = gtk::ProgressBar::new();
                bar.add_css_class("dl-bar");
                bar.set_fraction(d.progress.get());
                row.append(&bar);
            }
            self.dl_rows.append(&row);
        }
    }

    fn clear_downloads(self: &Rc<Self>) {
        self.downloads.borrow_mut().retain(|d| d.state.get() == DlState::Active);
        self.save_downloads();
        self.refresh_downloads();
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
        if self.ov.get() || self.ws().tabs.borrow().is_empty() {
            return;
        }
        self.palette.set_visible(false);
        self.ov.set(true);
        self.ov_return.set((self.cur.get(), self.ws().focus.get()));
        self.ov_hint.set_visible(true);
        self.target_zoom.set(OV_ZOOM);
        // Every workspace zooms out: the whole thing is one overview.
        let all = self.wss.borrow().clone();
        for ws in &all {
            ws.saved.set(ws.target.get());
            ws.strip.add_css_class("zoomed");
            for t in ws.tabs.borrow().iter() {
                t.view.set_can_target(false); // clicks pick a tab, they don't reach the page
            }
        }
        self.animate_stack();
        for ws in &all {
            self.reveal_ws(ws);
        }
    }

    /// Zoom back in on whichever tab is focused now.
    fn exit_overview(self: &Rc<Self>) {
        if !self.ov.get() {
            return;
        }
        self.ov.set(false);
        self.ov_hint.set_visible(false);
        self.target_zoom.set(1.0);
        let all = self.wss.borrow().clone();
        for ws in &all {
            ws.strip.remove_css_class("zoomed");
            for t in ws.tabs.borrow().iter() {
                t.view.set_can_target(true);
            }
            // Start from the pre-overview scroll: if the focus didn't move we land exactly
            // where we left, otherwise we scroll the minimum to reveal the new tab.
            ws.target.set(ws.saved.get());
        }
        self.animate_stack();
        for ws in &all {
            self.reveal_ws(ws);
        }
        if self.ws().tabs.borrow().is_empty() {
            self.new_tab(); // landed on an empty workspace
        } else if let Some(v) = self.focused_view() {
            v.grab_focus();
        }
    }

    /// Esc: zoom back in on the tab you started from.
    fn cancel_overview(self: &Rc<Self>) {
        let (w, t) = self.ov_return.get();
        self.switch_ws(w);
        let len = self.ws().tabs.borrow().len();
        if len > 0 {
            self.focus_tab(t.min(len - 1));
        }
        self.exit_overview();
    }

    /// Click in overview: the tab under the pointer wins; empty space just exits.
    /// (x, y) are in the workspace stack's coordinates.
    fn ov_click(self: &Rc<Self>, x: f64, y: f64) {
        let h = self.stack.height() as f64;
        let z = self.stack_layout.zoom();
        let pitch = stack_pitch(h, z);
        let pos = self.stack_layout.pos();
        let n = self.wss.borrow().len();
        let band = (0..n).find(|&i| ((i as f64 - pos) * pitch + h / 2.0 - y).abs() <= h * z / 2.0);
        if let Some(wi) = band {
            self.switch_ws(wi);
            let ws = self.ws();
            let vw = ws.strip.width() as f64;
            let (z, off) = (ws.layout.zoom(), ws.layout.offset());
            let g = gap(z);
            let x = x - ws.strip.margin_start() as f64;
            let mut hit = None;
            let mut cx = 0.0;
            for (i, t) in ws.tabs.borrow().iter().enumerate() {
                let w = col_width_of(&t.view, vw);
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
        }
        self.exit_overview();
    }

    /// Keys while zoomed out. Returns true when consumed.
    fn ov_key(self: &Rc<Self>, k: gdk::Key, m: gdk::ModifierType) -> bool {
        use gdk::ModifierType as M;
        let plain = m.is_empty();
        let f = self.ws().focus.get();
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
                gdk::Key::j | gdk::Key::Down => {
                    self.switch_ws(self.cur.get() + 1);
                    return true;
                }
                gdk::Key::k | gdk::Key::Up => {
                    self.switch_ws(self.cur.get().saturating_sub(1));
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

    fn on_key(self: &Rc<Self>, key: gdk::Key, code: u32, state: gdk::ModifierType) -> bool {
        use gdk::ModifierType as M;
        let m = state & gtk::accelerator_get_default_mod_mask();
        if std::env::var_os("RAIL_DEBUG_KEYS").is_some() {
            eprintln!("key {:?} code {code} state {state:?} focus {:?}", key.name(), gtk::prelude::GtkWindowExt::focus(&self.win).map(|w| w.type_().name()));
        }
        // The letter we want comes from the physical key, so layouts that remap
        // Shift+letter (or non-Latin layouts) still hit our shortcuts.
        let physical = match code {
            43 => Some(gdk::Key::h),
            44 => Some(gdk::Key::j),
            45 => Some(gdk::Key::k),
            46 => Some(gdk::Key::l),
            42 => Some(gdk::Key::g),
            40 => Some(gdk::Key::d),
            33 => Some(gdk::Key::p),
            41 => Some(gdk::Key::f),
            58 => Some(gdk::Key::m),
            39 => Some(gdk::Key::s),
            38 => Some(gdk::Key::a),
            _ => None,
        };
        let k = match physical {
            Some(p) if m.intersects(M::CONTROL_MASK) => p,
            _ => key.to_lower(),
        };

        if k == gdk::Key::Escape {
            if self.palette_open() {
                self.cancel_palette();
                return true;
            }
            if self.shot.borrow().is_some() {
                self.close_shot();
                return true;
            }
            if self.downloads_open() {
                self.toggle_downloads();
                return true;
            }
            if self.ov.get() {
                self.cancel_overview();
                return true;
            }
            // A find-in-page highlight goes first; the page only sees Esc once it is gone.
            if self.focused_tab().is_some_and(|t| t.st.finding.get()) {
                self.clear_find();
                return true;
            }
            return false;
        }
        if self.shot_key(k, m) {
            return true;
        }
        if self.palette_open() {
            let mode = self.mode.get();
            let ctrl = m == M::CONTROL_MASK && mode == Mode::Tabs;
            let dir = match k {
                gdk::Key::Down if m.is_empty() => Some(1),
                gdk::Key::Up if m.is_empty() => Some(-1),
                gdk::Key::n | gdk::Key::j if ctrl => Some(1),
                gdk::Key::p | gdk::Key::k if ctrl => Some(-1),
                _ => None,
            };
            if let Some(d) = dir {
                match mode {
                    Mode::Tabs => self.tabs_move(d),
                    Mode::Find => self.find_step(d),
                    _ => self.sugg_move(d),
                }
                return true;
            }
        }
        // Digit keys by physical position (evdev code + 8), so any layout works.
        let digit = (10..=18).contains(&code).then(|| (code - 10) as usize);
        let global = m == M::CONTROL_MASK && matches!(k, gdk::Key::o | gdk::Key::t | gdk::Key::q);
        let ws_key = digit.is_some() && (m == M::CONTROL_MASK || m == M::CONTROL_MASK | M::SHIFT_MASK);
        if self.overview_open() && !global && !ws_key && self.ov_key(k, m) {
            return true;
        }
        if m.is_empty() && k == gdk::Key::F11 {
            self.win.set_fullscreened(!self.win.is_fullscreen());
            return true;
        }
        if m.is_empty() && k == gdk::Key::F2 {
            self.open_palette(Mode::Rename);
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
        if let Some(d) = digit {
            if m == M::CONTROL_MASK {
                self.goto_ws(d);
                return true;
            }
            if m == M::CONTROL_MASK | M::SHIFT_MASK {
                self.send_tab(d);
                return true;
            }
        }
        if m == M::CONTROL_MASK | M::ALT_MASK {
            let cur = self.cur.get();
            match k {
                gdk::Key::j | gdk::Key::Down => self.switch_ws(cur + 1),
                gdk::Key::k | gdk::Key::Up => self.switch_ws(cur.saturating_sub(1)),
                gdk::Key::w => self.close_ws(),
                _ => return false,
            }
            return true;
        }
        let focus = self.ws().focus.get();
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
                gdk::Key::p if !self.palette_open() => self.toggle_pin(),
                gdk::Key::m if !self.palette_open() => self.toggle_mute(),
                _ => return false,
            }
            return true;
        }
        if m == M::CONTROL_MASK | M::SHIFT_MASK {
            match k {
                gdk::Key::f if !self.palette_open() => {
                    self.exit_overview();
                    self.open_palette(Mode::Tabs);
                }
                gdk::Key::p if !self.palette_open() => self.toggle_pip(),
                gdk::Key::s if !self.palette_open() => self.toggle_shot(),
                gdk::Key::a => self.toggle_adblock(),
                gdk::Key::h => self.move_tab(-1),
                gdk::Key::l => self.move_tab(1),
                gdk::Key::g => self.toggle_group(),
                gdk::Key::d => self.toggle_downloads(),
                _ => return false,
            }
            return true;
        }
        false
    }
}

/// Keep logins across restarts, and learn about downloads.
fn setup_storage(app: &Rc<App>) {
    let Some(session) = webkit6::NetworkSession::default() else { return };
    if let Some(cookies) = session.cookie_manager() {
        cookies.set_persistent_storage(
            data_dir().join("cookies.sqlite").to_str().unwrap_or("cookies.sqlite"),
            webkit6::CookiePersistentStorage::Sqlite,
        );
    }
    let a = app.clone();
    session.connect_download_started(move |_, d| a.on_download(d));
}

fn build(gapp: &gtk::Application) {
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

    let ws_title = gtk::Label::new(Some("WORKSPACES"));
    ws_title.add_css_class("section");
    ws_title.set_xalign(0.0);
    let wlist = gtk::ListBox::new();
    wlist.add_css_class("wslist");
    wlist.set_selection_mode(gtk::SelectionMode::Single);
    wlist.set_activate_on_single_click(true);

    let tab_title = gtk::Label::new(Some("TABS"));
    tab_title.add_css_class("section");
    tab_title.set_xalign(0.0);
    let list = gtk::ListBox::new();
    list.add_css_class("tablist");
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(true);
    let list_scroll = gtk::ScrolledWindow::new();
    list_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    list_scroll.set_vexpand(true);
    list_scroll.set_child(Some(&list));

    let dl_btn_label = gtk::Label::new(Some("Downloads"));
    let dl_inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    dl_inner.append(&gtk::Image::from_icon_name("folder-download-symbolic"));
    dl_inner.append(&dl_btn_label);
    let dl_btn = gtk::Button::new();
    dl_btn.add_css_class("dl-btn");
    dl_btn.set_child(Some(&dl_inner));
    dl_btn.set_halign(gtk::Align::Start);
    dl_btn.set_tooltip_text(Some("Downloads  (Ctrl+Shift+D)"));

    let hint = gtk::Label::new(Some("ctrl+1-9 workspace · f2 rename · ctrl+b hide"));
    hint.add_css_class("hint");
    hint.set_xalign(0.0);
    hint.set_margin_start(8);

    let side = gtk::Box::new(gtk::Orientation::Vertical, 8);
    side.set_margin_top(10);
    side.set_margin_bottom(10);
    side.set_margin_start(10);
    side.set_margin_end(2);
    side.append(&head);
    side.append(&pill);
    side.append(&ws_title);
    side.append(&wlist);
    side.append(&tab_title);
    side.append(&list_scroll);
    side.append(&dl_btn);
    side.append(&hint);

    // ---- workspace stack + shell ----
    let stack_layout = StackLayout::new();
    let stack = gtk::Box::new(gtk::Orientation::Vertical, 0);
    stack.set_layout_manager(Some(stack_layout.clone()));
    stack.set_overflow(gtk::Overflow::Hidden);

    let shell_layout = ShellLayout::new();
    shell_layout.set_side(SIDE_W as f64);
    let shell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    shell.set_layout_manager(Some(shell_layout.clone()));
    shell.set_overflow(gtk::Overflow::Hidden);
    shell.set_hexpand(true);
    shell.set_vexpand(true);
    shell.append(&side);
    shell.append(&stack);

    // ---- address palette ----
    let entry = gtk::Entry::new();
    entry.add_css_class("palette-entry");
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Search or enter address"));
    let prow = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let palette_icon = gtk::Image::from_icon_name("system-search-symbolic");
    palette_icon.set_pixel_size(20);
    prow.append(&palette_icon);
    prow.append(&entry);
    let sugg = gtk::ListBox::new();
    sugg.add_css_class("sugg");
    sugg.set_selection_mode(gtk::SelectionMode::Single);
    sugg.set_activate_on_single_click(true);
    sugg.set_can_focus(false);
    sugg.set_visible(false);
    let palette_hint = gtk::Label::new(Some("Enter  go      Esc  cancel"));
    palette_hint.add_css_class("palette-hint");
    palette_hint.set_xalign(0.0);
    let palette = gtk::Box::new(gtk::Orientation::Vertical, 0);
    palette.add_css_class("palette");
    palette.set_halign(gtk::Align::Center);
    palette.set_valign(gtk::Align::Start);
    palette.set_margin_top(110);
    palette.append(&prow);
    palette.append(&sugg);
    palette.append(&palette_hint);
    palette.set_visible(false);

    // ---- overview hint (the overview itself is the strip, zoomed) ----
    let ov_hint = gtk::Label::new(Some("h l  tabs      j k  workspaces      Enter  open      x  close      Esc  cancel"));
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

    let toast = gtk::Label::new(None);
    toast.add_css_class("toast");
    toast.set_halign(gtk::Align::Center);
    toast.set_valign(gtk::Align::Start);
    toast.set_margin_top(22);
    toast.set_can_target(false);
    toast.set_visible(false);

    // ---- downloads panel ----
    let dl_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let dl_scroll = gtk::ScrolledWindow::new();
    dl_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    dl_scroll.set_propagate_natural_height(true);
    dl_scroll.set_max_content_height(380);
    dl_scroll.set_child(Some(&dl_rows));
    let dl_title = gtk::Label::new(Some("Downloads"));
    dl_title.add_css_class("dl-title");
    dl_title.set_xalign(0.0);
    dl_title.set_hexpand(true);
    let dl_clear = gtk::Button::with_label("Clear finished");
    dl_clear.add_css_class("dl-clear");
    let dl_head = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dl_head.set_margin_start(4);
    dl_head.set_margin_bottom(6);
    dl_head.append(&dl_title);
    dl_head.append(&dl_clear);
    let dl_panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dl_panel.add_css_class("dl-panel");
    dl_panel.set_halign(gtk::Align::End);
    dl_panel.set_valign(gtk::Align::End);
    dl_panel.set_margin_end(24);
    dl_panel.set_margin_bottom(24);
    dl_panel.append(&dl_head);
    dl_panel.append(&dl_scroll);
    dl_panel.set_visible(false);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&shell));
    overlay.add_overlay(&load_bar);
    overlay.add_overlay(&dl_panel);
    overlay.add_overlay(&palette);
    overlay.add_overlay(&ov_hint);
    overlay.add_overlay(&toast);

    let win = gtk::ApplicationWindow::builder()
        .application(gapp)
        .default_width(1400)
        .default_height(850)
        .decorated(false) // no header, no footer: the page is the window
        .child(&overlay)
        .build();

    let dir = data_dir();
    // Vim-style keys live in the page itself (see page.js); `nokeys.txt` lists hosts to leave alone.
    let off: Vec<String> = std::fs::read_to_string(dir.join("nokeys.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| format!("{:?}", l.trim_start_matches("www.")))
        .collect();
    let ucm = webkit6::UserContentManager::new();
    ucm.register_script_message_handler("rail", Some("rail"));
    ucm.add_script(&webkit6::UserScript::for_world(
        &include_str!("page.js").replace("/*OFF*/[]", &format!("[{}]", off.join(","))),
        webkit6::UserContentInjectedFrames::TopFrame,
        webkit6::UserScriptInjectionTime::Start,
        "rail",
        &[],
        &[],
    ));
    let downloads: Vec<Rc<Dl>> = std::fs::read_to_string(dir.join("downloads.txt"))
        .unwrap_or_default()
        .lines()
        .rev()
        .take(50)
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .map(|path| {
            Rc::new(Dl {
                name: path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()),
                path,
                progress: Cell::new(1.0),
                state: Cell::new(DlState::Done),
                handle: RefCell::new(None),
            })
        })
        .collect();

    let app = Rc::new(App {
        win: win.clone(),
        shell: shell.clone(),
        shell_layout,
        side,
        stack,
        stack_layout,
        stack_anim: Cell::new(false),
        wss: RefCell::new(Vec::new()),
        cur: Cell::new(0),
        list: list.clone(),
        rowmap: RefCell::new(Vec::new()),
        wlist: wlist.clone(),
        pill_label,
        load_bar,
        toast,
        toast_gen: Cell::new(0),
        palette,
        palette_icon,
        palette_hint,
        entry: entry.clone(),
        sugg: sugg.clone(),
        sugg_urls: RefCell::new(Vec::new()),
        sugg_sel: Cell::new(None),
        typed: RefCell::new(String::new()),
        completion: RefCell::new(None),
        suppress: Cell::new(false),
        last_len: Cell::new(0),
        mode: Cell::new(Mode::Address),
        history: RefCell::new(History::load(dir.join("history.tsv"))),
        downloads: RefCell::new(downloads),
        dl_panel,
        dl_rows,
        dl_btn: dl_btn.clone(),
        dl_btn_label,
        dl_pending: Cell::new(false),
        ov_hint,
        ov: Cell::new(false),
        ov_return: Cell::new((0, 0)),
        target_zoom: Cell::new(1.0),
        side_open: Cell::new(true),
        side_gen: Cell::new(0),
        fresh: RefCell::new(None),
        overlay: overlay.clone(),
        ucm: ucm.clone(),
        engines: RefCell::new(search::load(&dir.join("searchengines.txt"))),
        sugg_tabs: RefCell::new(Vec::new()),
        shot: RefCell::new(None),
        pip: RefCell::new(None),
        adblock_on: Cell::new(!dir.join("adblock-off").exists()),
        adblock_filter: RefCell::new(None),
    });
    setup_storage(&app);
    let a = app.clone();
    ucm.connect_script_message_received(Some("rail"), move |_, v| a.on_page_msg(&v.to_str()));
    app.setup_adblock();

    // Workspace names are remembered; the tabs in them are not.
    let names: Vec<String> = std::fs::read_to_string(dir.join("workspaces.txt"))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(MAX_WS)
        .map(String::from)
        .collect();
    if names.is_empty() {
        app.add_ws("main");
    } else {
        for n in &names {
            app.add_ws(n);
        }
    }

    let a = app.clone();
    list.connect_row_activated(move |_, row| {
        let t = a.rowmap.borrow().get(row.index() as usize).copied().flatten();
        if let Some(t) = t {
            a.focus_tab(t);
        }
    });
    let a = app.clone();
    wlist.connect_row_activated(move |_, row| a.switch_ws(row.index() as usize));
    let a = app.clone();
    add.connect_clicked(move |_| a.new_tab());
    let a = app.clone();
    pill.connect_clicked(move |_| a.show_palette());
    let a = app.clone();
    dl_btn.connect_clicked(move |_| a.toggle_downloads());
    let a = app.clone();
    dl_clear.connect_clicked(move |_| a.clear_downloads());
    let a = app.clone();
    entry.connect_activate(move |_| a.accept());
    let a = app.clone();
    entry.connect_changed(move |_| a.on_typed());
    let a = app.clone();
    sugg.connect_row_activated(move |_, row| {
        if a.mode.get() == Mode::Tabs {
            let hit = a.sugg_tabs.borrow().get(row.index() as usize).copied();
            a.hide_palette();
            if let Some((wi, ti)) = hit {
                a.jump_to(wi, ti);
            }
            return;
        }
        let url = a.sugg_urls.borrow().get(row.index() as usize).cloned();
        if let Some(u) = url {
            a.navigate(&u);
        }
    });

    // In overview, clicking a (non-interactive) zoomed tab selects it.
    let pick = gtk::GestureClick::new();
    let a = app.clone();
    pick.connect_released(move |_, _, x, y| {
        if a.ov.get() {
            a.ov_click(x, y);
        }
    });
    app.stack.add_controller(pick);

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
    keys.connect_key_pressed(move |_, key, code, state| {
        if a.on_key(key, code, state) { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    win.add_controller(keys);

    // Fresh every launch: one empty tab, no session restore -- except pinned tabs.
    app.refresh_downloads();
    app.restore_pinned();
    app.new_tab();
    let a = app.clone();
    let every = std::env::var("RAIL_SWEEP_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30);
    glib::timeout_add_seconds_local(every, move || {
        a.suspend_sweep();
        glib::ControlFlow::Continue
    });
    if let Some(url) = std::env::args().nth(1) {
        app.navigate(&url);
    }
    win.present();
    // Without this a page opened from the command line starts without keyboard focus.
    if !app.palette_open() {
        if let Some(v) = app.focused_view() {
            v.grab_focus();
        }
    }
}

/// Rail is launched without a terminal, so a panic (which aborts inside GTK
/// callbacks) would vanish silently. Leave a trace in `crash.log` instead.
fn install_crash_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let bt = std::backtrace::Backtrace::force_capture();
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(data_dir().join("crash.log")) {
            let _ = writeln!(f, "--- panic at unix {t} ---\n{info}\n{bt}\n");
        }
        default(info);
    }));
}

fn main() -> glib::ExitCode {
    install_crash_log();
    let app = gtk::Application::builder()
        .application_id("dev.rail.browser")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(build);
    app.run_with_args::<&str>(&[])
}

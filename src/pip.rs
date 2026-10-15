//! Picture-in-picture. WebKitGTK has no PiP API, so the tab itself is popped
//! out into a small window and its biggest video is stretched over the whole
//! page (see `__railPipOn` in page.js). Closing the window puts the tab back.
//!
//! On a tiling compositor, make the window float with a rule on the title
//! "Picture in Picture".

use super::*;

pub(crate) struct Pip {
    pub tab: Tab,
    ws: usize,
    idx: usize,
    win: gtk::Window,
}

impl App {
    pub(crate) fn toggle_pip(self: &Rc<Self>) {
        if self.pip.borrow().is_some() {
            self.pip_close();
            return;
        }
        let Some(tab) = self.focused_tab() else { return };
        if tab.suspended() {
            return;
        }
        let (app, view) = (self.clone(), tab.view.clone());
        tab.view.evaluate_javascript(
            "window.__railPipOn ? window.__railPipOn() : 'null'",
            Some("rail"),
            None,
            gio::Cancellable::NONE,
            move |res| {
                let dims = res.ok().map(|v| v.to_str().to_string()).and_then(|s| {
                    let (w, h) = s.trim_matches(['[', ']']).split_once(',')?;
                    Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?))
                });
                match dims {
                    Some((w, h)) => app.pip_open(&view, w, h),
                    None => app.toast("No video found on this page"),
                }
            },
        );
    }

    fn pip_open(self: &Rc<Self>, view: &WebView, w: f64, h: f64) {
        if self.pip.borrow().is_some() {
            return;
        }
        let Some((ws, idx, tab)) = self.detach_view(view) else { return };
        if self.palette_open() && self.fresh.borrow().is_some() {
            self.hide_palette(); // popping out the only tab leaves a new empty one behind
        }
        let height = if w > 0.0 && h > 0.0 { (480.0 * h / w).clamp(120.0, 480.0) as i32 } else { 270 };
        let win = gtk::Window::builder()
            .title("Picture in Picture")
            .default_width(480)
            .default_height(height)
            .decorated(false)
            .transient_for(&self.win)
            .build();
        view.set_can_target(true);
        win.set_child(Some(view));

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let app = self.clone();
        keys.connect_key_pressed(move |_, key, _, m| {
            let ctrl_shift_p = key.to_lower() == gdk::Key::p && m.contains(gdk::ModifierType::CONTROL_MASK);
            if key == gdk::Key::Escape || ctrl_shift_p {
                app.pip_close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        win.add_controller(keys);
        let app = self.clone();
        win.connect_close_request(move |_| {
            app.pip_close();
            glib::Propagation::Stop
        });

        *self.pip.borrow_mut() = Some(Pip { tab, ws, idx, win: win.clone() });
        win.present();
        view.grab_focus();
        Self::kick_video(view);
        self.toast("Picture in Picture  ·  Esc returns the tab");
    }

    /// Put the popped-out tab back where it came from.
    pub(crate) fn pip_close(self: &Rc<Self>) {
        let Some(p) = self.pip.borrow_mut().take() else { return };
        let view = p.tab.view.clone();
        view.evaluate_javascript("window.__railPipOff && window.__railPipOff()", Some("rail"), None, gio::Cancellable::NONE, |_| {});
        p.win.set_child(None::<&gtk::Widget>);
        view.set_can_target(!self.ov.get());
        let ws = self.wss.borrow().get(p.ws).cloned().unwrap_or_else(|| self.ws());
        let idx = p.idx.min(ws.tabs.borrow().len());
        {
            let tabs = ws.tabs.borrow();
            match idx {
                0 => ws.strip.prepend(&view),
                _ => ws.strip.insert_child_after(&view, Some(&tabs[idx - 1].view)),
            }
        }
        ws.tabs.borrow_mut().insert(idx, p.tab.clone());
        let wi = self.wss.borrow().iter().position(|w| Rc::ptr_eq(w, &ws)).unwrap_or(0);
        if wi != self.cur.get() {
            self.switch_ws(wi);
        }
        self.focus_tab(idx);
        let win = p.win.clone();
        glib::idle_add_local_once(move || win.destroy());
        Self::kick_video(&view);
    }

    /// Reparenting pauses the page for a moment; ask the video to carry on once the dust settles.
    fn kick_video(view: &WebView) {
        for ms in [250, 900] {
            let v = view.clone();
            glib::timeout_add_local_once(Duration::from_millis(ms), move || {
                v.evaluate_javascript("window.__railKick && window.__railKick()", Some("rail"), None, gio::Cancellable::NONE, |_| {});
            });
        }
    }

    /// The page closed itself while popped out: nothing to put back.
    pub(crate) fn pip_discard(self: &Rc<Self>) {
        if let Some(p) = self.pip.borrow_mut().take() {
            p.win.set_child(None::<&gtk::Widget>);
            let win = p.win.clone();
            glib::idle_add_local_once(move || win.destroy());
        }
    }
}

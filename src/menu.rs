//! The right-click menu (replaces WebKit's own).

use super::*;

fn copy_text(text: &str) {
    if let Some(d) = gdk::Display::default() {
        d.clipboard().set_text(text);
    }
}

fn item(bx: &gtk::Box, pop: &gtk::Popover, label: &str, hint: &str, f: impl FnOnce() + 'static) {
    let b = gtk::Button::new();
    b.add_css_class("ctx-item");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    l.set_hexpand(true);
    row.append(&l);
    if !hint.is_empty() {
        let h = gtk::Label::new(Some(hint));
        h.add_css_class("ctx-hint");
        row.append(&h);
    }
    b.set_child(Some(&row));
    let (pop, f) = (pop.clone(), RefCell::new(Some(f)));
    b.connect_clicked(move |_| {
        pop.popdown();
        // After the popover is gone, so focus lands where the action wants it.
        if let Some(f) = f.borrow_mut().take() {
            glib::idle_add_local_once(f);
        }
    });
    bx.append(&b);
}

fn sep(bx: &gtk::Box) {
    let s = gtk::Separator::new(gtk::Orientation::Horizontal);
    s.add_css_class("ctx-sep");
    bx.append(&s);
}

impl App {
    pub(crate) fn open_in_new_tab(self: &Rc<Self>, uri: &str) {
        self.exit_overview();
        let view = self.new_view();
        self.add_view(view.clone());
        view.load_uri(uri);
    }

    pub(crate) fn show_menu(self: &Rc<Self>, view: &WebView, hit: &webkit6::HitTestResult, at: (f64, f64)) {
        let hit = hit.clone();
        if hit.context_is_selection() && !hit.context_is_editable() {
            let (app, v) = (self.clone(), view.clone());
            view.evaluate_javascript("window.getSelection().toString()", None, None, gio::Cancellable::NONE, move |res| {
                let sel = res.ok().map(|x| x.to_str().to_string()).filter(|s| !s.trim().is_empty());
                app.build_menu(&v, &hit, at, sel);
            });
        } else {
            self.build_menu(view, &hit, at, None);
        }
    }

    fn build_menu(self: &Rc<Self>, view: &WebView, hit: &webkit6::HitTestResult, at: (f64, f64), selection: Option<String>) {
        let pop = gtk::Popover::new();
        pop.add_css_class("ctx");
        pop.set_has_arrow(false);
        pop.set_halign(gtk::Align::Start);
        let bx = gtk::Box::new(gtk::Orientation::Vertical, 0);
        pop.set_child(Some(&bx));

        // back / forward / reload
        let nav = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        nav.set_homogeneous(true);
        for (icon, tip, enabled, act) in [
            ("go-previous-symbolic", "Back", view.can_go_back(), 0),
            ("go-next-symbolic", "Forward", view.can_go_forward(), 1),
            ("view-refresh-symbolic", "Reload", true, 2),
        ] {
            let b = gtk::Button::from_icon_name(icon);
            b.add_css_class("ctx-item");
            b.set_tooltip_text(Some(tip));
            b.set_sensitive(enabled);
            let (pop, v) = (pop.clone(), view.clone());
            b.connect_clicked(move |_| {
                pop.popdown();
                match act {
                    0 => v.go_back(),
                    1 => v.go_forward(),
                    _ => v.reload(),
                }
            });
            nav.append(&b);
        }
        bx.append(&nav);
        sep(&bx);

        if let Some(url) = hit.link_uri().map(|u| u.to_string()) {
            let a = self.clone();
            let u = url.clone();
            item(&bx, &pop, "Open link in new tab", "", move || a.open_in_new_tab(&u));
            item(&bx, &pop, "Copy link address", "", move || copy_text(&url));
            sep(&bx);
        }
        if hit.context_is_image() {
            if let Some(url) = hit.image_uri().map(|u| u.to_string()) {
                let a = self.clone();
                let u = url.clone();
                item(&bx, &pop, "Open image in new tab", "", move || a.open_in_new_tab(&u));
                let u = url.clone();
                item(&bx, &pop, "Copy image address", "", move || copy_text(&u));
                let v = view.clone();
                item(&bx, &pop, "Save image", "", move || {
                    v.download_uri(&url);
                });
                sep(&bx);
            }
        }
        if hit.context_is_media() {
            if let Some(url) = hit.media_uri().map(|u| u.to_string()) {
                let u = url.clone();
                item(&bx, &pop, "Copy media address", "", move || copy_text(&u));
                let v = view.clone();
                item(&bx, &pop, "Save media", "", move || {
                    v.download_uri(&url);
                });
                sep(&bx);
            }
        }
        if hit.context_is_editable() {
            for (label, cmd) in [("Cut", "Cut"), ("Copy", "Copy"), ("Paste", "Paste"), ("Select all", "SelectAll")] {
                let v = view.clone();
                item(&bx, &pop, label, "", move || v.execute_editing_command(cmd));
            }
            sep(&bx);
        } else if let Some(text) = selection {
            let v = view.clone();
            item(&bx, &pop, "Copy", "", move || v.execute_editing_command("Copy"));
            let short: String = text.trim().chars().take(24).collect();
            let more = if text.trim().chars().count() > 24 { "…" } else { "" };
            let a = self.clone();
            item(&bx, &pop, &format!("Search for “{short}{more}”"), "", move || {
                let uri = to_uri(&text, &a.engines.borrow());
                a.open_in_new_tab(&uri);
            });
            sep(&bx);
        }

        let a = self.clone();
        item(&bx, &pop, "Take screenshot…", "Ctrl+Shift+S", move || a.toggle_shot());
        let a = self.clone();
        item(&bx, &pop, "Picture in Picture", "Ctrl+Shift+P", move || a.toggle_pip());
        let a = self.clone();
        item(&bx, &pop, "Find in page…", "/", move || a.open_palette(Mode::Find));
        sep(&bx);
        let pinned = self.tab_of(view).is_some_and(|t| t.st.pinned.get());
        let a = self.clone();
        item(&bx, &pop, if pinned { "Unpin tab" } else { "Pin tab" }, "Ctrl+P", move || a.toggle_pin());
        if view.is_playing_audio() || view.is_muted() {
            let a = self.clone();
            item(&bx, &pop, if view.is_muted() { "Unmute tab" } else { "Mute tab" }, "Ctrl+M", move || a.toggle_mute());
        }
        if let Some(uri) = view.uri().map(|u| u.to_string()).filter(|u| !u.starts_with("about:")) {
            item(&bx, &pop, "Copy page address", "", move || copy_text(&uri));
        }
        let (a, v) = (self.clone(), view.clone());
        item(&bx, &pop, "Close tab", "Ctrl+W", move || a.close_view(&v));

        pop.set_parent(view);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(at.0 as i32, at.1 as i32, 1, 1)));
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
    }
}

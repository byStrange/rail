//! Screenshots, Firefox style: pick "visible area" or "full page", look at the
//! result, then copy it to the clipboard or save it as a PNG.

use super::*;
use webkit6::{SnapshotOptions, SnapshotRegion};

pub(crate) struct Shot {
    bar: gtk::Box,
    view: WebView,
    /// Set once a capture finished; until then the bar offers the two regions.
    tex: Option<gdk::Texture>,
    busy: bool,
}

fn clear(bx: &gtk::Box) {
    while let Some(c) = bx.first_child() {
        bx.remove(&c);
    }
}

#[allow(deprecated)] // the replacement needs a PNG round-trip; this is fine for a thumbnail
/// The top of the capture, scaled to a 190 px wide thumbnail (at most 110 px tall).
fn thumbnail(tex: &gdk::Texture) -> Option<gdk::Texture> {
    let full = gdk::pixbuf_get_from_texture(tex)?;
    let (w, h) = (full.width(), full.height());
    let keep = h.min((f64::from(w) * 110.0 / 190.0).round() as i32).max(1);
    let top = full.new_subpixbuf(0, 0, w, keep);
    let th = (f64::from(keep) * 190.0 / f64::from(w)).round().max(1.0) as i32;
    let small = top.scale_simple(190, th, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    Some(gdk::Texture::for_pixbuf(&small))
}

fn button(label: &str, key: &str) -> gtk::Button {
    let b = gtk::Button::new();
    b.add_css_class("shot-btn");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&gtk::Label::new(Some(label)));
    let k = gtk::Label::new(Some(key));
    k.add_css_class("ctx-hint");
    row.append(&k);
    b.set_child(Some(&row));
    b
}

impl App {
    pub(crate) fn toggle_shot(self: &Rc<Self>) {
        if self.shot.borrow().is_some() {
            self.close_shot();
        } else {
            self.open_shot();
        }
    }

    pub(crate) fn close_shot(&self) {
        if let Some(s) = self.shot.borrow_mut().take() {
            self.overlay.remove_overlay(&s.bar);
        }
        if let Some(v) = self.focused_view() {
            v.grab_focus();
        }
    }

    fn open_shot(self: &Rc<Self>) {
        let Some(view) = self.focused_view() else { return };
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.add_css_class("shot");
        bar.set_halign(gtk::Align::Center);
        bar.set_valign(gtk::Align::Start);
        bar.set_margin_top(22);
        self.overlay.add_overlay(&bar);
        *self.shot.borrow_mut() = Some(Shot { bar, view, tex: None, busy: false });
        self.shot_choose();
    }

    fn shot_choose(self: &Rc<Self>) {
        let Some(bar) = self.shot.borrow().as_ref().map(|s| s.bar.clone()) else { return };
        clear(&bar);
        let title = gtk::Label::new(Some("Screenshot"));
        title.add_css_class("shot-title");
        bar.append(&title);
        let vis = button("Visible area", "V");
        let full = button("Full page", "F");
        let x = gtk::Button::from_icon_name("window-close-symbolic");
        x.add_css_class("shot-btn");
        let a = self.clone();
        vis.connect_clicked(move |_| a.shot_capture(SnapshotRegion::Visible));
        let a = self.clone();
        full.connect_clicked(move |_| a.shot_capture(SnapshotRegion::FullDocument));
        let a = self.clone();
        x.connect_clicked(move |_| a.close_shot());
        bar.append(&vis);
        bar.append(&full);
        bar.append(&x);
    }

    fn shot_capture(self: &Rc<Self>, region: SnapshotRegion) {
        let view = {
            let mut g = self.shot.borrow_mut();
            let Some(s) = g.as_mut().filter(|s| !s.busy && s.tex.is_none()) else { return };
            s.busy = true;
            clear(&s.bar);
            let l = gtk::Label::new(Some("Capturing…"));
            l.add_css_class("shot-title");
            s.bar.append(&l);
            s.view.clone()
        };
        let app = self.clone();
        view.snapshot(region, SnapshotOptions::NONE, gio::Cancellable::NONE, move |res| match res {
            Ok(tex) => app.shot_review(tex),
            Err(e) => {
                app.close_shot();
                app.toast(&format!("Screenshot failed: {e}"));
            }
        });
    }

    fn shot_review(self: &Rc<Self>, tex: gdk::Texture) {
        let bar = {
            let mut g = self.shot.borrow_mut();
            let Some(s) = g.as_mut() else { return }; // cancelled while capturing
            s.tex = Some(tex.clone());
            s.busy = false;
            s.bar.clone()
        };
        clear(&bar);
        // A full-page capture can be tens of thousands of pixels tall: show its top, small.
        let thumb = thumbnail(&tex).unwrap_or_else(|| tex.clone());
        let pic = gtk::Picture::for_paintable(&thumb);
        pic.add_css_class("shot-pic");
        pic.set_valign(gtk::Align::Center);
        bar.append(&pic);
        let info = gtk::Label::new(Some(&format!("{} × {}", tex.width(), tex.height())));
        info.add_css_class("shot-title");
        bar.append(&info);
        let copy = button("Copy", "C");
        let save = button("Save", "S");
        let x = gtk::Button::from_icon_name("window-close-symbolic");
        x.add_css_class("shot-btn");
        let a = self.clone();
        copy.connect_clicked(move |_| a.shot_copy());
        let a = self.clone();
        save.connect_clicked(move |_| a.shot_save());
        let a = self.clone();
        x.connect_clicked(move |_| a.close_shot());
        bar.append(&copy);
        bar.append(&save);
        bar.append(&x);
    }

    fn shot_tex(&self) -> Option<gdk::Texture> {
        self.shot.borrow().as_ref().and_then(|s| s.tex.clone())
    }

    fn shot_copy(self: &Rc<Self>) {
        let Some(tex) = self.shot_tex() else { return };
        if let Some(d) = gdk::Display::default() {
            d.clipboard().set_texture(&tex);
        }
        self.close_shot();
        self.toast("Screenshot copied to the clipboard");
    }

    fn shot_save(self: &Rc<Self>) {
        let Some(tex) = self.shot_tex() else { return };
        let dir = glib::user_special_dir(glib::UserDirectory::Pictures)
            .unwrap_or_else(|| glib::home_dir().join("Pictures"))
            .join("Screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let stamp = glib::DateTime::now_local()
            .ok()
            .and_then(|d| d.format("%Y-%m-%d_%H-%M-%S").ok())
            .map_or_else(|| "shot".to_string(), |s| s.to_string());
        let path = unique_path(&dir, &format!("rail-{stamp}.png"));
        let res = tex.save_to_png(&path);
        self.close_shot();
        match res {
            Ok(()) => self.toast(&format!("Saved {}", path.display())),
            Err(e) => self.toast(&format!("Couldn't save the screenshot: {e}")),
        }
    }

    /// Keys while the screenshot bar is open. True when consumed.
    pub(crate) fn shot_key(self: &Rc<Self>, k: gdk::Key, m: gdk::ModifierType) -> bool {
        let reviewing = match self.shot.borrow().as_ref() {
            None => return false,
            Some(s) => s.tex.is_some(),
        };
        if !m.is_empty() {
            return false;
        }
        match (reviewing, k) {
            (_, gdk::Key::Escape) => self.close_shot(),
            (false, gdk::Key::v) => self.shot_capture(SnapshotRegion::Visible),
            (false, gdk::Key::f) => self.shot_capture(SnapshotRegion::FullDocument),
            (true, gdk::Key::c) => self.shot_copy(),
            (true, gdk::Key::s | gdk::Key::Return | gdk::Key::KP_Enter) => self.shot_save(),
            _ => return false,
        }
        true
    }
}

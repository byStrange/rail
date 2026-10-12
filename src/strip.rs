//! The niri-style horizontal strip: a custom GTK layout manager that places
//! tabs side by side in columns and slides the whole strip by `offset`.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{graphene, gsk};
use gtk4 as gtk;

pub const GAP: f64 = 6.0;

/// Space between columns, in strip coordinates. It grows as the strip zooms
/// out so scaled-down tabs keep a readable gap instead of nearly touching.
pub fn gap(zoom: f64) -> f64 {
    GAP + (1.0 - zoom) * 40.0
}

/// Half columns fill exactly two-per-screen; full columns fill the viewport.
pub fn col_width(full: bool, viewport: f64) -> f64 {
    if full { viewport } else { ((viewport - GAP) / 2.0).floor() }
}

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[derive(Default)]
    pub struct StripLayout {
        /// Left edge of the visible region, in unscaled strip coordinates.
        pub offset: Cell<f64>,
        /// 1.0 normally; < 1.0 in overview, where the live tabs are scaled down.
        pub zoom: Cell<f64>,
        pub width: Cell<i32>,
        pub on_resize: RefCell<Option<Rc<dyn Fn()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StripLayout {
        const NAME: &'static str = "RailStripLayout";
        type Type = super::StripLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for StripLayout {}

    impl LayoutManagerImpl for StripLayout {
        fn measure(&self, _w: &gtk::Widget, _o: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            // The strip never dictates window size; content scrolls instead.
            (0, 0, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            if self.width.replace(width) != width {
                // Deferred: we must not touch layout state from inside allocate.
                if let Some(cb) = self.on_resize.borrow().clone() {
                    glib::idle_add_local_once(move || cb());
                }
            }
            let vw = width as f64;
            let (offset, z) = (self.offset.get(), self.zoom.get());
            // Scaled content stays vertically centered (niri-style zoom-out).
            let ty = height as f64 / 2.0 * (1.0 - z);
            let mut x = 0.0;
            let mut child = widget.first_child();
            while let Some(c) = child {
                let cw = col_width(c.has_css_class("full"), vw);
                // Children keep their real size; the transform does the scaling,
                // so pages never re-layout while zooming.
                let t = gsk::Transform::new()
                    .translate(&graphene::Point::new(((x - offset) * z) as f32, ty as f32))
                    .scale(z as f32, z as f32);
                c.allocate(cw as i32, height, -1, Some(t));
                x += cw + gap(z);
                child = c.next_sibling();
            }
        }
    }
}

glib::wrapper! {
    pub struct StripLayout(ObjectSubclass<imp::StripLayout>) @extends gtk::LayoutManager;
}

impl StripLayout {
    pub fn new() -> Self {
        let l: Self = glib::Object::new();
        l.imp().zoom.set(1.0);
        l
    }
    pub fn offset(&self) -> f64 {
        self.imp().offset.get()
    }
    pub fn set_offset(&self, v: f64) {
        self.imp().offset.set(v);
    }
    pub fn zoom(&self) -> f64 {
        self.imp().zoom.get()
    }
    pub fn set_zoom(&self, v: f64) {
        self.imp().zoom.set(v);
    }
    pub fn connect_resized(&self, f: impl Fn() + 'static) {
        *self.imp().on_resize.borrow_mut() = Some(std::rc::Rc::new(f));
    }
}

// ---------------------------------------------------------------------------
// Shell layout: [sidebar | strip]. The sidebar is always allocated at its full
// width and slid in/out with a transform, so it can only ever be fully open,
// fully closed, or mid-animation -- never a stuck half width.
// ---------------------------------------------------------------------------

pub const SIDE_W: i32 = 248;

mod shell_imp {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    pub struct ShellLayout {
        /// Currently visible sidebar width, 0..=SIDE_W.
        pub side: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ShellLayout {
        const NAME: &'static str = "RailShellLayout";
        type Type = super::ShellLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for ShellLayout {}

    impl LayoutManagerImpl for ShellLayout {
        fn measure(&self, _w: &gtk::Widget, _o: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            let sw = self.side.get().round() as i32;
            let Some(side) = widget.first_child() else { return };
            if side.is_visible() {
                let t = gsk::Transform::new().translate(&graphene::Point::new((sw - SIDE_W) as f32, 0.0));
                side.allocate(SIDE_W, height, -1, Some(t));
            }
            if let Some(main) = side.next_sibling() {
                let t = gsk::Transform::new().translate(&graphene::Point::new(sw as f32, 0.0));
                main.allocate((width - sw).max(0), height, -1, Some(t));
            }
        }
    }
}

glib::wrapper! {
    pub struct ShellLayout(ObjectSubclass<shell_imp::ShellLayout>) @extends gtk::LayoutManager;
}

impl ShellLayout {
    pub fn new() -> Self {
        glib::Object::new()
    }
    pub fn side(&self) -> f64 {
        self.imp().side.get()
    }
    pub fn set_side(&self, v: f64) {
        self.imp().side.set(v);
    }
}

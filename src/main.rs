#[macro_use]
extern crate lazy_static;

use std::thread::{spawn, sleep, sleep_ms};
use std::time::Duration;
use std::sync::{Mutex, Arc};

use gio::prelude::*;

use glib::glib_sys::g_path_skip_root;

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Button, Editable};

use serde::{Serialize, Deserialize};

mod winapi_stuff;
use winapi_stuff::*;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Model {
    Clip,
}

fn set_visual(window: &gtk::ApplicationWindow, _screen: Option<&gdk::Screen>) {
    if let Some(screen) = window.get_screen() {
        if let Some(ref visual) = screen.get_rgba_visual() {
            window.set_visual(Some(visual)); // crucial for transparency
        }
    }
}

fn draw(_window: &gtk::ApplicationWindow, ctx: &cairo::Context) -> Inhibit {
    // crucial for transparency
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    ctx.set_operator(cairo::Operator::Screen);
    ctx.paint();
    Inhibit(false)
}

fn draw_entry_background(_window: &gtk::Box, ctx: &cairo::Context) -> Inhibit {
    // crucial for transparency
    ctx.set_source_rgba(1.0, 0.0, 0.0, 0.5);
    ctx.set_operator(cairo::Operator::Screen);
    ctx.paint();
    Inhibit(false)
}

fn spawn_entry(text : &str) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);

    container.connect_draw(draw_entry_background);
    let entry = gtk::Label::new(None);
    entry.set_selectable(true);
    entry.set_markup(text);

    container.add(&entry);

    container
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);
    window.connect_screen_changed(set_visual);
    window.connect_draw(draw);
    window.set_app_paintable(true); // crucial for transparency
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    HotkeyData::do_it(WindowsApiEvent::AddClipboardListener { handler : Arc::new(move |clip| {
        tx.send(clip); }
    )} );

    window.set_title("pusz");
    window.set_border_width(0);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(840, 480);
    window.set_decorated(false);

    let input_field = gtk::Entry::new();

    let row = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let scroll_container = gtk::ScrolledWindow::new( gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);

    let scroll_insides = gtk::Box::new(gtk::Orientation::Vertical, 1);
    scroll_container.add(&scroll_insides);

    for i in 0..10 {
        scroll_insides.add(&spawn_entry(&format!("abc: {}", i)));
    }

    row.pack_start(&input_field, false, false, 10);
    row.add(&scroll_container);

    let mut visible = true;

    window.add(&row);

    window.show_all();

    rx.attach(None, move |val| {
        println!("got clip update: {}", val);
        visible = !visible;
        if visible {
            scroll_container.hide();
        } else {
            scroll_container.show();

        }

        glib::Continue(true)
    });
}

fn main() {
    let application = Application::new(Some("com.github.gtk-rs.examples.basic"), Default::default())
        .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        build_ui(app);
    });

    spawn(|| {
        sleep_ms(5000);
       HotkeyData::set_clipboard("potatkowa kraina");
    });

    application.run(&[]);
}


#[cfg(test)]
mod model_tests {
    use super::*;
}
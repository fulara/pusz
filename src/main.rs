#[macro_use]
extern crate lazy_static;

use std::thread::{spawn, sleep};
use std::time::{Duration,
                SystemTime};
use std::sync::{Arc};
use std::rc::Rc;
use std::cell::RefCell;

use gio::prelude::*;

use gtk::prelude::*;
use gtk::{Application};

use serde::{Serialize, Deserialize};

mod winapi_stuff;
use winapi_stuff::*;
use std::io::Write;

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
    if _window.has_focus() {
        ctx.set_source_rgba(1.0, 0.0, 1.0, 1.0);
    } else {
        ctx.set_source_rgba(1.0, 0.0, 0.0, 1.0);
    }
    ctx.set_operator(cairo::Operator::Screen);
    ctx.paint();
    Inhibit(false)
}

fn spawn_entry(main_edit : gtk::Entry,text : &str) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
//    container.connect("key-press-event", true, |x| { println!("got: {:?}", x); Some(true.to_value()) });

    container.connect_key_press_event(move |_, event_key| {
        use gdk::enums::key::*;
        #[allow(non_upper_case_globals)]
        match event_key.get_keyval() {
            Return => {
                println!("got return.. copied to clip");
                Inhibit(false)
            }
            Down | Up  => {
                Inhibit(false)
            }
            _ => {
                main_edit.grab_focus_without_selecting();
                let _ = main_edit.emit("key-press-event", &[&event_key.to_value()]);
                Inhibit(true)
            }
        }
        });
    container.set_can_focus(true);
//    container.set_has_window(true);
    container.connect_draw(draw_entry_background);

    let workaround_button = gtk::Button::new_with_label(text);

    let text = text.to_owned();
    workaround_button.connect_clicked(move |_| {
        HotkeyData::set_clipboard(&text);
    });

    workaround_button.emit_clicked();

    container.add(&workaround_button);

    container
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataEntry {
    text : String,
    last_use_timestamp : SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DataModel {
    clips : Vec<DataEntry>
}

fn load_data_model(file : &str) -> DataModel {
    use std::fs;

    let contents = fs::read_to_string(file).unwrap_or_default();

    serde_json::from_str(&contents).unwrap_or_default()
}

fn save_data_model(file : &str, model : &DataModel) {
    use std::fs::File;

    let mut file = File::create(file).expect("couldnt create a file.");
    file.write_all(serde_json::to_string(model).expect("failed to serialize").as_bytes()).expect("couldnt dump data model.");
}

struct Context {
    model : DataModel,
}

impl Context {
    fn add_entry(&mut self, text : &str) {
        for e in &mut self.model.clips {
            if e.text == text {
                e.last_use_timestamp = SystemTime::now();
                return;
            }
        }

        self.model.clips.push(DataEntry {text : text.to_owned(), last_use_timestamp : SystemTime::now() });

        save_data_model("pusz.json", &self.model);
    }

    fn find_matching_entries(&self, needle : &str) -> impl Iterator<Item = &DataEntry> {
        let needle = needle.to_lowercase();
        //ineff but to be improved
        self.model.clips.iter().filter(move |e| e.text.to_ascii_lowercase().contains(&needle))
    }
}

fn build_ui(application: &gtk::Application) {
    let ctx = Rc::new(RefCell::new(Context{
        model : load_data_model("pusz.json")
    }));

    let window = gtk::ApplicationWindow::new(application);
    window.connect_screen_changed(set_visual);
    window.connect_draw(draw);
    window.set_app_paintable(true); // crucial for transparency
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    HotkeyData::do_it(WindowsApiEvent::AddClipboardListener { handler : Arc::new(move |clip| {
        tx.send(clip).expect("send failure");
    }
    )} );

    window.set_title("pusz");
    window.set_border_width(0);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(840, 480);
    window.set_decorated(false);

//    let _ = window.connect("focus-in-event", true, |a| { println!("hai.") ;Some(true.to_value())});

    let input_field = gtk::Entry::new();

    let row = gtk::Box::new(gtk::Orientation::Vertical, 1);

    let scroll_container = gtk::ScrolledWindow::new( gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
    scroll_container.set_max_content_height(400);


    let scroll_insides = gtk::Box::new(gtk::Orientation::Vertical, 1);
    scroll_container.add(&scroll_insides);

    for i in 0..1 {
        scroll_insides.add(&spawn_entry(input_field.clone(), &format!("abc: {}", i)));
    }

    row.add(&input_field);
//    row.pack_start(&input_field, false, false, 10);
    row.add(&scroll_container);
    row.set_child_expand(&scroll_container, true);

//    let mut visible = true;

    window.add(&row);

    window.show_all();
    {
        let ctx = Rc::clone(&ctx);
        let if_ = input_field.clone();
        if_.connect_changed(move |entry| {
            for c in &scroll_insides.get_children() {
                scroll_insides.remove(c);
            }

            if let Some(text) = entry.get_text() {
                for data_entry in ctx.borrow().find_matching_entries(&text) {
                    println!("indeed found sth: {:?}", data_entry);
                    scroll_insides.add(&spawn_entry(input_field.clone(), &data_entry.text));

                    scroll_insides.show_all();
                }
            }

        });
    }

    rx.attach(None, move |val| {
        ctx.borrow_mut().add_entry(&val);

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
        sleep(Duration::from_secs(5));
       HotkeyData::set_clipboard("potatkowa kraina");
    });

    application.run(&[]);
}


#[cfg(test)]
mod model_tests {
//    use super::*;
}
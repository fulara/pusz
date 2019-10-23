extern crate lazy_static;

use std::thread::{spawn, sleep};
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

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Data {
    description: String,
    model : Model,
}

impl Data {
    const N : usize = 2;

    fn indices() -> [u32; Self::N] {
        [0, 1]
    }

    fn col_types() -> [gtk::Type; Data::N] {
        [gtk::Type::String, gtk::Type::String]
    }

    // meh. rework when learned how to put custom types into gtk::Value.
    fn values(&self) -> [String; Self::N] {
        [self.description.clone(), serde_json::to_string(&self).expect("cant serialize.")]
    }

    fn deser(from : &gtk::TreeModel, iter : &gtk::TreeIter) -> Self {
        serde_json::from_str(&from.get_value(iter, 1).get::<String>().unwrap()).unwrap()
    }
}

fn create_list_model(ctx : &Context) -> gtk::ListStore {
    let store = gtk::ListStore::new(&Data::col_types());
    for d in ctx.data.iter() {
        let values = d.values();
        let refs: [&dyn ToValue; Data::N] = [&values[0], &values[1]];
        store.set(&store.append(), &Data::indices(), &refs);
    }
    store
}

fn match_fun(entry : &gtk::EntryCompletion, text_so_far : &str, iter: &gtk::TreeIter) -> bool {
    true
}

struct Context {
    data : Vec<Data>,
}

impl Context {
    fn new() -> Self {
        Self {
            data: vec![]
        }
    }
}

fn build_ui(application: &gtk::Application) {
    let mut ctx = Context::new();
    let window = gtk::ApplicationWindow::new(application);
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    HotkeyData::do_it(WindowsApiEvent::AddClipboardListener { handler : Arc::new(move |clip| {
        tx.send(clip); }
    )} );

    window.set_title("Entry with autocompletion");
    window.set_border_width(5);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(840, 480);

    // Create a title label
    let win_title = gtk::Label::new(None);
    win_title.set_markup("<big>Which country would you like to spend a holiday in?</big>");

    // Create an EntryCompletion widget
    let completion_countries = gtk::EntryCompletion::new();
    // Use the first (and only) column available to set the autocompletion text
    completion_countries.set_text_column(0);

    completion_countries.set_match_func(|entry : &gtk::EntryCompletion, text_so_far : &str, iter: &gtk::TreeIter| {

        if let Some(model) = entry.get_model() {
            let data = Data::deser(&model, iter);
                println!("model: {:?}", data);
        }

        true
    });
    // how many keystrokes to wait before attempting to autocomplete?
    completion_countries.set_minimum_key_length(1);
    // whether the completions should be presented in a popup window
    completion_countries.set_popup_completion(true);

    // Create a ListStore of items
    // These will be the source for the autocompletion
    // as the user types into the field
    // For a more evolved example of ListStore see src/bin/list_store.rs
    let ls = create_list_model(&ctx);

    completion_countries.set_model(Some(&ls));

    let input_field = gtk::Entry::new();
    input_field.set_completion(Some(&completion_countries));

    let row = gtk::Box::new(gtk::Orientation::Vertical, 5);
    row.add(&win_title);
    row.pack_start(&input_field, false, false, 10);

    rx.attach(None, move |val| {
        ctx.data.push(Data {
            model : Model::Clip,
            description : val.clone(),
        });

        completion_countries.set_model(Some(&create_list_model(&ctx)));

        glib::Continue(true)
    });

    // window.add(&win_title);
    window.add(&row);

    // show everything
    window.show_all();
}

fn main() {
    let application = Application::new(Some("com.github.gtk-rs.examples.basic"), Default::default())
        .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&[]);
}


#[cfg(test)]
mod model_tests {
    use super::*;
}
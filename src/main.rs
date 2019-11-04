#[macro_use]
extern crate lazy_static;

use std::io::Write;
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
use std::collections::HashMap;
use plugin_interface::{PuszDisplayRow, Entry};

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

fn special_entry(ctx : &Context, text : &str) -> Vec<Entry> {
    let mut entries = vec![];

    for (regex, base) in ctx.special_entries_builders.iter() {
        for cap in regex.captures_iter(text) {
            entries.push(Entry {
                label:  format!("snow link: {}", cap[1].to_owned()),
                content: format!("https://ig.service-now.com/{}.do?sysparm_query=number={}", base, cap[1].to_owned()),
            })
        }
    }

    entries
}

fn spawn_entry(ctx : Rc<RefCell<Context>>, main_edit : gtk::Entry, row : PuszDisplayRow, is_removable : bool ) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    let main_handler = {
        let text = row.main_entry.content.clone();
        move || { HotkeyData::set_clipboard(&text) }
    };

    let handler_clone = main_handler.clone();
    container.connect_key_press_event(move |_, event_key| {
        use gdk::enums::key::*;
        #[allow(non_upper_case_globals)]
        match event_key.get_keyval() {
            Return => {
                handler_clone();
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
//    container.set_has_window(true); crashes app.
    container.connect_draw(draw_entry_background);

    let workaround_button = gtk::Button::new_with_label(&row.main_entry.label);

    workaround_button.connect_clicked(move |_| {
        main_handler();
    });
    container.add(&workaround_button);

    for entry in row.additional_entries {
        let button = gtk::Button::new_with_label(&entry.label);
        button.connect_clicked(move |_| {
            HotkeyData::set_clipboard(&entry.content);
        });

        container.add(&button);
    }

    if is_removable {
        let text = row.main_entry.label.to_string();
        let removal_button = gtk::Button::new_with_label("X");
        {
            let container = container.clone();
            container.add(&removal_button);
            removal_button.connect_clicked(move |_| {
                let ctx: &mut Context = &mut ctx.borrow_mut();
                ctx.remove_entry(&text);

                container.hide();
                container.grab_focus();
            });
        }
    }

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
    file.write_all(serde_json::to_string_pretty(model).expect("failed to serialize").as_bytes()).expect("couldnt dump data model.");
}

struct Query {
    action : String,
    query: String,
}

impl Query {
    fn action(&self) -> &str {
        if self.action.is_empty() {
            "clip"
        } else {
            self.action()
        }
    }
}

struct Context {
    special_entries_builders : Vec<(regex::Regex, String)>,

    model : DataModel,

    plugins : HashMap<String, Box<dyn plugin_interface::Plugin>>,
}

impl Context {
    fn new(model : DataModel) -> Self {
        let r = [(r"(INC\d{4,})", "incident"),
            (r"(RITM\d{4,})", "sc_req_item"),
            (r"(CHG\d{4,})", "change_request"),
            (r"(PRB\d{4,})", "problem")
            ,(r"(PRBTASK\d{4,})", "problem_task")];


        Self {
            model,
            special_entries_builders : r.iter().map(|(pattern, base)| (regex::Regex::new(pattern).expect(&format!("failure to build regex from {}", pattern)), base.to_string())).collect(),

            plugins : load_plugins(),
        }

    }

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

    fn remove_entry(&mut self, text : &str) {
        // need more efficient sol! :)
        self.model.clips.retain( |e| e.text != text);

        save_data_model("pusz.json", &self.model);
    }

    fn query(&self, query : Query) -> Vec<&DataEntry> {
        use fuzzy_matcher::skim::fuzzy_match;
        let mut matched = self.model.clips.iter().filter_map(|e| fuzzy_match(&e.text, &query.query).map(|match_score| (e, match_score))).collect::<Vec<_>>();

        matched.sort_by(|(_, score_a), (_, score_b)| score_b.cmp(score_a));

        matched.iter().map(|(e, ..)| *e).collect()
    }
}

enum PuszEvent {
    ClipboardChanged(String),
    BringToFront,
}

fn build_ui(application: &gtk::Application) {
    let ctx = Rc::new(RefCell::new(Context::new(load_data_model("pusz.json"))));

    let window = gtk::ApplicationWindow::new(application);
    window.connect_screen_changed(set_visual);
    window.connect_draw(draw);
    window.set_app_paintable(true); // crucial for transparency
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    {
        let tx = tx.clone();
        HotkeyData::do_it(WindowsApiEvent::AddClipboardListener {
            handler: Arc::new(move |clip| {
                tx.send(PuszEvent::ClipboardChanged(clip)).expect("send failure");
            }
            )
        });
    }

    window.set_title("pusz");
    window.set_border_width(0);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(840, 480);
    window.set_decorated(false);

//    window.connect_focus_in_event(|_, event| {
//        println!("gained focus.");
//        Inhibit(false)
//    } );

//    window.connect_focus_out_event(|_, event| {
//        println!("lost focus.");
//        Inhibit(false)
//    } );

    let input_field = gtk::Entry::new();

    let row = gtk::Box::new(gtk::Orientation::Vertical, 1);

    let scroll_container = gtk::ScrolledWindow::new( gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
    scroll_container.set_max_content_height(400);


    let scroll_insides = gtk::Box::new(gtk::Orientation::Vertical, 1);
    scroll_container.add(&scroll_insides);

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
                let mut words = text.split_whitespace();
                if text.starts_with("/") {
                    let command = &words.next().unwrap()[1..];
                    let result = if let Some(plugin) = ctx.borrow_mut().plugins.get_mut(command) {
                        let query : String = words.collect();
                        plugin.query(&query)
                    } else {
                        // hint about what plugins are available
                        return;
                    };

                    use plugin_interface::*;
                    if let PluginResult::Ok(results) = result {
                        for r in results {
                                scroll_insides.add(&spawn_entry(ctx.clone(), input_field.clone(), r, false));
                        }
                    } else {
                        let err_message = format!("{:?}", result);
                        let err_row = PuszDisplayRow {
                            additional_entries: vec![],
                            main_entry: Entry {
                                label: err_message.clone(),
                                content: err_message,
                            }
                        };
                        scroll_insides.add(&spawn_entry(ctx.clone(), input_field.clone(), err_row, false));
                    }
                } else {
                    for data_entry in ctx.borrow().query(Query { query: text.to_string(), action: String::new() }) {
                        scroll_insides.add(&spawn_entry(ctx.clone(), input_field.clone(), PuszDisplayRow { main_entry : Entry { label : data_entry.text.clone(), content : data_entry.text.clone() } , additional_entries  : special_entry(&ctx.borrow(), &data_entry.text)}, true));
                    }
                }

                scroll_insides.show_all();
            }

        });
    }

    rx.attach(None, move |event| {
        match event {
            PuszEvent::ClipboardChanged(clipboard) => {
                            ctx.borrow_mut().add_entry(&clipboard);
            },
            PuszEvent::BringToFront => {
                window.present();
            },
        }
        glib::Continue(true)
    });

    HotkeyData::register_hotkey(13, winapi_stuff::Key::F1, Modifier::None, Arc::new(move |_| {
        tx.send(PuszEvent::BringToFront).unwrap();
    }));
}

fn load_plugins() -> HashMap<String, Box<dyn plugin_interface::Plugin>> {
    let mut plugin : Box<dyn plugin_interface::Plugin> =
        unsafe {
            let lib = libloading::Library::new("target/debug/calc_plugin.dll").expect("failed to load");
            let load: libloading::Symbol<plugin_interface::LoadFn> = lib.get(b"load").expect("failed to load introduce");
            let plugin = load(plugin_interface::COMMON_INTERFACE_VERSION);

            //well - we dont want to unload plugins ever.
            ::std::mem::forget(lib);

            plugin
        }.expect("couldnt load plugin!");

    vec![plugin].into_iter().map(|p| (p.name().to_string(), p)).collect()
}

fn main() {
//    let mut plugins = load_plugins();
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
    use plugin_interface::Entry;
    #[test]
    fn special_entry_snow() {

        let ctx = Context::new(DataModel { clips : vec![]});

        assert_eq!(special_entry(&ctx,"invalid"), vec![]);
        assert_eq!(special_entry(&ctx,"INC0123"),
                   vec![Entry { label : "snow link: INC0123".to_owned(), content : "https://ig.service-now.com/incident.do?sysparm_query=number=INC0123".to_owned() }]);
        assert_eq!(special_entry(&ctx,"CHG0123"),
                   vec![Entry { label : "snow link: CHG0123".to_owned(), content : "https://ig.service-now.com/change_request.do?sysparm_query=number=CHG0123".to_owned() }]);
        assert_eq!(special_entry(&ctx,"RITM0123"),
                   vec![Entry { label : "snow link: RITM0123".to_owned(), content : "https://ig.service-now.com/sc_req_item.do?sysparm_query=number=RITM0123".to_owned() }]);
        assert_eq!(special_entry(&ctx,"PRBTASK0123"),
                   vec![Entry { label : "snow link: PRBTASK0123".to_owned(), content : "https://ig.service-now.com/problem_task.do?sysparm_query=number=PRBTASK0123".to_owned() }]);
        assert_eq!(special_entry(&ctx,"PRB0123"),
                   vec![Entry { label : "snow link: PRB0123".to_owned(), content : "https://ig.service-now.com/problem.do?sysparm_query=number=PRB0123".to_owned() }]);
    }

    #[test]
    fn fuzzy_match_showcase() {
        use fuzzy_matcher::skim::fuzzy_match;

        assert_eq!(Some(106), fuzzy_match("choice", "choice"));
        //hm. better result than 1:1 string?
        assert_eq!(Some(110), fuzzy_match("c-hoice", "choice"));
        assert_eq!(Some(46), fuzzy_match("cxhxoxixcxex", "choice"));
    }
}
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

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
use plugin_interface::{PuszRow,
                       PuszRowBuilder,
                       PuszRowIdentifier,
                       PuszClipEntry,
                       PuszEntry,
                       PluginEvent,
                      };

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

fn special_entry(ctx : &Context, text : &str) -> Vec<PuszEntry> {
    let mut entries = vec![];

    for (regex, base) in ctx.special_entries_builders.iter() {
        for cap in regex.captures_iter(text) {
            entries.push(PuszEntry::Display(PuszClipEntry {
                label:  format!("snow link: {}", cap[1].to_owned()),
                content: format!("https://ig.service-now.com/{}.do?sysparm_query=number={}", base, cap[1].to_owned()),
            }))
        }
    }

    entries
}

fn spawn_entry(ctx : Rc<RefCell<Context>>, main_edit : gtk::Entry, row : PuszRow) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    let text = row.main_entry.content.clone();

    let text_cloned = text.clone();
    container.connect_key_press_event(move |_, event_key| {
        use gdk::enums::key::*;
        #[allow(non_upper_case_globals)]
        match event_key.get_keyval() {
            Return => {
                HotkeyData::set_clipboard(&text_cloned);
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

    workaround_button.connect_button_press_event(move |_, event| {
        match event.get_event_type() {
            gdk::EventType::ButtonPress => {
                HotkeyData::set_clipboard(&text);
            },
            gdk::EventType::DoubleButtonPress => {
                if url::Url::parse(&text).is_ok() {
                    webbrowser::open(&text);
                }
            }

            _ => {}
        }

        Inhibit(true)
    });
    container.add(&workaround_button);

    for entry in row.additional_entries {
        match entry {
            PuszEntry::Display(entry) => {
                let button = gtk::Button::new_with_label(&entry.label);
                button.connect_button_press_event(move |_, event| {
                    match event.get_event_type() {
                        gdk::EventType::ButtonPress => {
                            HotkeyData::set_clipboard(&entry.content);
                        },
                        gdk::EventType::DoubleButtonPress => {
                            if url::Url::parse(&entry.content).is_ok() {
                                webbrowser::open(&entry.content);
                            }
                        }

                        _ => {}
                    }

                    Inhibit(true)
                });

                container.add(&button);
            },
            PuszEntry::Action(action) => panic!(),
        }
    }

    if row.is_removable {
        let text = row.main_entry.label.to_string();
        let removal_button = gtk::Button::new_with_label("X");
        {
            let container = container.clone();
            container.add(&removal_button);
            removal_button.connect_button_press_event(move |_, _| {
                let ctx: &mut Context = &mut ctx.borrow_mut();
                ctx.remove_entry(&text);

                container.hide();
                container.grab_focus();

                Inhibit(true)
            });
        }
    }

    container
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

    plugins : HashMap<String, Box<dyn plugin_interface::Plugin>>,
}

impl Context {
    fn new() -> Self {
        let r = [(r"(INC\d{4,})", "incident"),
            (r"(RITM\d{4,})", "sc_req_item"),
            (r"(CHG\d{4,})", "change_request"),
            (r"(PRB\d{4,})", "problem")
            , (r"(PRBTASK\d{4,})", "problem_task")];


        Self {
            special_entries_builders: r.iter().map(|(pattern, base)| (regex::Regex::new(pattern).expect(&format!("failure to build regex from {}", pattern)), base.to_string())).collect(),

            plugins: load_plugins(),
        }
    }

    fn remove_entry(&mut self, text: &str) {
        // need more efficient sol! :)
//        self.model.clips.retain( |e| e.text != text);

//        save_data_model("pusz.json", &self.model);
    }
}

enum PuszEvent {
    ClipboardChanged(String),
    BringToFront,
}

fn main_id() -> PuszRowIdentifier {
    PuszRowIdentifier::MainApi
}

fn build_ui(application: &gtk::Application) {
    let ctx = Rc::new(RefCell::new(Context::new()));

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
        let input_field = input_field.clone();
        input_field.clone().connect_changed(move |entry| {
            for c in &scroll_insides.get_children() {
                scroll_insides.remove(c);
            }

            if let Some(text) = entry.get_text() {
                let mut words = text.split_whitespace();
                let (query, command) = if text.starts_with("/") {
                    let command = &words.next().unwrap()[1..];
                    let query: String = words.collect();

                    (query, Some(command))
                } else {
                    (text.to_string(), None)
                };

                //would current borrowck allow me to store plugins into vec rather than doing the below abom?

                let results: Vec<PluginResult> =
                    ctx.borrow_mut()
                        .plugins
                        .iter_mut()
                        .filter(|(name, plugin)| Some(plugin.name()) == command || !plugin.settings().requies_explicit_query)
                        .map(|(_name, plugin)| {
                    plugin.query(&query)
                    // hint about what plugins are available
                }).collect();

                use plugin_interface::*;
                // better handle a case where nothing matches.
//                    if results.is_empty() {
//                        let err_message = format!("{:?}", result);
//                        let err_row = PuszRowBuilder::new(err_message, main_id()).build().unwrap();
//
//                        scroll_insides.add(&spawn_entry(ctx.clone(), input_field.clone(), err_row));
//                    } else {
                for result in results {
                    if let PluginResult::Ok(results) = result {
                        for r in results {
                            scroll_insides.add(&spawn_entry(ctx.clone(), input_field.clone(), r));
                        }
                    }
                }
            }
            scroll_insides.show_all();
        });
    }

    rx.attach(None, move |event| {
        match event {
            PuszEvent::ClipboardChanged(clipboard) => {
                for (_, plugin) in ctx.borrow_mut().plugins.iter_mut() {
                    if plugin.settings().interested_in_clipboard {
                        plugin.on_subscribed_event(&PluginEvent::Clipboard(clipboard.clone()));
                    }
                }
            },
            PuszEvent::BringToFront => {
                window.present();
                if let Some(clip) = HotkeyData::get_clipboard() {
                    input_field.grab_focus();
                    input_field.emit_paste_clipboard();
                }
            },
        }
        glib::Continue(true)
    });

    HotkeyData::register_hotkey(13, winapi_stuff::Key::F1, Modifier::None, Arc::new(move |_| {
        tx.send(PuszEvent::BringToFront).unwrap();
    }));
}

fn load_plugins() -> HashMap<String, Box<dyn plugin_interface::Plugin>> {
    use std::fs;

    let mut dll_paths =
    if let Ok(entries) = fs::read_dir("plugins") {
        entries.filter_map(|e| e.ok()).filter_map(|e| e.path().into_os_string().into_string().ok()).filter(|file_name| file_name.ends_with(".dll")).collect::<Vec<_>>()
    } else {
        println!("couldnt read plugins dir?");
        return HashMap::new();
    };

    //hacky solution for development purposes.
    if std::path::Path::new("target/debug/calc_plugin.dll").exists() {
        dll_paths.push("target/debug/calc_plugin.dll".to_string());
    }

    if std::path::Path::new("target/debug/clipboard_plugin.dll").exists() {
        dll_paths.push("target/debug/clipboard_plugin.dll".to_string());
    }

   let mut plugins : Vec<Box<dyn plugin_interface::Plugin>> =
        unsafe {
            dll_paths.into_iter().map(|dll_path| {
                let lib = libloading::Library::new(dll_path).expect("failed to load");
                let load: libloading::Symbol<plugin_interface::LoadFn> = lib.get(b"load").expect("failed to load introduce");
                let plugin = load(plugin_interface::COMMON_INTERFACE_VERSION);

                //well - we dont want to unload plugins ever.
                ::std::mem::forget(lib);

                plugin.expect("couldnt load plugin!")
            }).collect()
        };

    plugins.into_iter().map(|p| (p.name().to_string(), p)).collect()
}

fn main() {
    use simplelog::*;
    use std::fs::File;
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed).unwrap(),
            WriteLogger::new(LevelFilter::Info, Config::default(), File::create("pusz.log").unwrap()),
        ]
    ).unwrap();

    info!("Pusz application initializing.");

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
    use plugin_interface::PuszClipEntry;
    #[test]
    fn special_entry_snow() {

        let ctx = Context::new();

        assert_eq!(special_entry(&ctx,"invalid"), vec![]);
        assert_eq!(special_entry(&ctx,"INC0123"),
                   vec![PuszEntry::Display(PuszClipEntry { label : "snow link: INC0123".to_owned(), content : "https://ig.service-now.com/incident.do?sysparm_query=number=INC0123".to_owned() })]);
        assert_eq!(special_entry(&ctx,"CHG0123"),
                   vec![PuszEntry::Display(PuszClipEntry { label : "snow link: CHG0123".to_owned(), content : "https://ig.service-now.com/change_request.do?sysparm_query=number=CHG0123".to_owned() })]);
        assert_eq!(special_entry(&ctx,"RITM0123"),
                   vec![PuszEntry::Display(PuszClipEntry { label : "snow link: RITM0123".to_owned(), content : "https://ig.service-now.com/sc_req_item.do?sysparm_query=number=RITM0123".to_owned() })]);
        assert_eq!(special_entry(&ctx,"PRBTASK0123"),
                   vec![PuszEntry::Display(PuszClipEntry { label : "snow link: PRBTASK0123".to_owned(), content : "https://ig.service-now.com/problem_task.do?sysparm_query=number=PRBTASK0123".to_owned() })]);
        assert_eq!(special_entry(&ctx,"PRB0123"),
                   vec![PuszEntry::Display(PuszClipEntry { label : "snow link: PRB0123".to_owned(), content : "https://ig.service-now.com/problem.do?sysparm_query=number=PRB0123".to_owned() })]);
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
use std::thread::{spawn, sleep};
use std::time::Duration;
use std::sync::{Mutex, Arc};

use gtk::prelude::*;
use gio::prelude::*;
use gtk::{Application, ApplicationWindow, Button, Editable};

use inputbot::KeybdKey::{RKey, AKey, LShiftKey};
use inputbot::handle_input_events;

use serde::{Serialize, Deserialize};
use glib::glib_sys::g_path_skip_root;
use winapi::um::mmeapi::waveInAddBuffer;

#[macro_use]
extern crate lazy_static;

fn bind_combo<F: Fn() + Send + Sync + 'static + Clone>(keys : Vec<inputbot::KeybdKey>, callback : F) {
    let kc = keys.clone();
    let handler = move || {
        if kc.iter().all(|k| k.is_pressed()) {
            callback();
        }
    };

    keys.iter().for_each(|k| k.bind(handler.clone()));
}

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

fn create_list_model() -> gtk::ListStore {
    let data: [Data; 4] = [
        Data {
            description: "France".to_string(),
            model : Model::Clip,
        },
        Data {
            description: "Italy".to_string(),
            model : Model::Clip,
        },
        Data {
            description: "Sweden".to_string(),
            model : Model::Clip,
        },
        Data {
            description: "Switzerland".to_string(),
            model : Model::Clip,
        },
    ];
    let store = gtk::ListStore::new(&Data::col_types());
    for d in data.iter() {
        let values = d.values();
        let refs: [&dyn ToValue; Data::N] = [&values[0], &values[1]];
        store.set(&store.append(), &Data::indices(), &refs);
    }
    store
}

fn match_fun(entry : &gtk::EntryCompletion, text_so_far : &str, iter: &gtk::TreeIter) -> bool {

    true
}

fn build_ui(application: &gtk::Application) {
    // create the main window
    let window = gtk::ApplicationWindow::new(application);
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
    let ls = create_list_model();

    completion_countries.set_model(Some(&ls));

    let input_field = gtk::Entry::new();
    input_field.set_completion(Some(&completion_countries));

    let row = gtk::Box::new(gtk::Orientation::Vertical, 5);
    row.add(&win_title);
    row.pack_start(&input_field, false, false, 10);

    // window.add(&win_title);
    window.add(&row);

    // show everything
    window.show_all();
}


fn wait_for_hotkey() {


}

use std::sync::atomic;
use std::sync;
use std::sync::mpsc::{Sender, Receiver, self, channel};


enum WindowsApiEvent {
    HotkeyRegister { id : i32, modifiers : u32, vk : u32},
}

struct HotkeyData {
    thread_handle : Option<::std::thread::JoinHandle<()>>,
    thread_id : usize,
    tx : Option<Sender<WindowsApiEvent>>,
}

struct HotkeyProxy {
    thread_id : usize,
    tx : Sender<WindowsApiEvent>,
}

impl HotkeyProxy {
    fn post_event(&self, event : WindowsApiEvent) {
        unsafe { winapi::um::winuser::PostThreadMessageA(self.thread_id as u32, 30000, 0, 0) };
        self.tx.send(event);
    }
}

enum ReceivedMessage {
    Hotkey { id: i32},
    Nothing,
}

fn get_single_message() -> ReceivedMessage {
    use winapi::um::winuser::{LPMSG, GetMessageA, MSG, *};
    use std::default::Default;
    let mut msg = Default::default();
    if unsafe { GetMessageA(&mut msg, 0 as winapi::shared::windef::HWND, 0, 0) != 0 } {
        match msg.message {
            WM_HOTKEY => {
                println!("got hotkey: {:?}", msg.wParam);
                return ReceivedMessage::Hotkey {id : msg.wParam as i32 };
            }
            30000 => {
                println!("hey, you see my dummy message :) ");
            }
            _ => {

            }
        }
    }

    ReceivedMessage::Nothing

}

impl HotkeyData {
    fn do_it(hotkey : WindowsApiEvent) {
        Self::init().post_event(hotkey);
    }

    fn init() -> HotkeyProxy {
        let context = &mut (*HOTKEY_DAYA.lock().unwrap());
        if context.is_none() {
            let ( tx_tid, rx_tid) =  mpsc::channel();
            let (tx, rx) = mpsc::channel();
            let thread_handle = Some(::std::thread::spawn(move || {
                let win_thread_id = unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() } as usize;
                if win_thread_id == 0 {
                    panic!("win_thread_id == 0?");
                }
                tx_tid.send(win_thread_id);

                loop {
                    match get_single_message() {
                        ReceivedMessage::Hotkey { id } => { println!("hotkey id'd triggered: {}", id) },
                        ReceivedMessage::Nothing => {},
                    }

                    if let Ok(request) = rx.try_recv() {
                        match request {
                            WindowsApiEvent::HotkeyRegister { id, modifiers, vk } => {
                                println!("got request to register hotkey!");
                                unsafe {
                                    winapi::um::winuser::RegisterHotKey(
                                        0 as winapi::shared::windef::HWND,
                                        id,
                                        modifiers, vk
                                    );
                                }
                            },
                        }
                    }
                }
//                println!("got hotkey: {:?}", hotkey);
            }));

            let thread_id = rx_tid.recv().expect("failed to recv thread_handle");

            *context = Some(HotkeyData {
                thread_id,
                thread_handle,
                tx : Some(tx),
            });

            println!("thread id: {:?}", context.as_ref().unwrap().thread_id);
        }

        let context = context.as_ref().unwrap();

        HotkeyProxy {
            thread_id : context.thread_id,
            tx : context.tx.clone().unwrap(),
        }
    }

}

lazy_static! {
    static ref HOTKEY_DAYA: Mutex<Option<HotkeyData>> = {
            Mutex::new(None)
//            let (tx, rx) = channel();
//            Mutex::new(HotkeyData {
//                thread : None,
//                thread_handle : usize,
//                tx : None,
//            })
    };
}

fn main() {
    HotkeyData::do_it(WindowsApiEvent::HotkeyRegister {modifiers : 0, id : 7, vk : 0x42});

    loop {
//        wait_for_hotkey();

//        unsafe { HOTKEY_DAYA.sth() };
        ::std::thread::sleep_ms(1000);
    }
//    let application = Application::new(Some("com.github.gtk-rs.examples.basic"), Default::default())
//        .expect("failed to initialize GTK application");
//
////    bind_combo(vec![AKey, RKey], || { println!( " multi press!"); });
//
//    application.connect_activate(|app| {
//        build_ui(app);
//    });
//
//    ::std::thread::spawn(|| {
//        loop {
//            ::std::thread::sleep_ms(1000);
//            let clip = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
//
//            println!("hai: {:?}", clip.wait_for_text());
//        }
//    .set_text("pXoXtXaXtXoX");

//    clip.request_text( | cp, maybe_text| {
//        println!("got clip!: {:?}", maybe_text);
//    });
//    });

//    application.connect_activate(move |app| {
//        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
//
//        let window = ApplicationWindow::new(app);
//        window.set_keep_above(true);
//
//        window.set_title("First GTK+ Program");
//        window.set_default_size(350, 70);
//
//        let button = Button::new_with_label("Click me!");
//        button.connect_clicked(|_| {
//            println!("Clicked!");
//        });
//        window.add(&button);
//
//        let editable = Editable::new();
//
//        window.show_all();
//
//        spawn(move || {
//            let mut c = 0;
//
//            loop {
//                handle_input_events();
//                c += 1;
////                tx.send(c);
//                sleep(Duration::from_secs(1));
//            }
//        });
//
////        let t = tx.clone();
////        bind_combo(vec![AKey], move || { t.send(0); });
////        let t = tx.clone();
////        bind_combo(vec![RKey], move || { t.send(1); });
//
//        rx.attach(None, move |val| {
//            window.set_keep_above( val % 2 == 1);
//
//            glib::Continue(true)
//        });
//    });

//    application.run(&[]);
}


#[cfg(test)]
mod model_tests {
    use super::*;
}
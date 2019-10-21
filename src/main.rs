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
use std::collections::HashMap;

pub type BindHandler = Arc<dyn Fn() + Send + Sync + 'static>;
pub type ClipboardHandler = Arc<dyn Fn(String) + Send + Sync + 'static>;

enum WindowsApiEvent {
    HotkeyRegister { id : i32, modifiers : u32, vk : u32, handler : BindHandler},
    AddClipboardListener { handler  : ClipboardHandler},
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
    ClipboardUpdate,
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
            WM_CLIPBOARDUPDATE => {
                println!("got clipboard update.");
                return ReceivedMessage::ClipboardUpdate;
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

unsafe extern "system"
fn find_window(hwnd: winapi::shared::windef::HWND, pid: winapi::shared::minwindef::LPARAM) -> i32 {
//    println!("potatko iteration");
    let mut process_id = 0;
    winapi::um::winuser::GetWindowThreadProcessId(hwnd, &mut process_id);

    println!("my pid is: {} search is: {}", pid, process_id);
    if process_id == (pid as u32) {
        println!("found it yupi.");
    }
    return 1;
}

fn to_wstring(str: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    ::std::ffi::OsStr::new(str).encode_wide().chain(Some(0).into_iter()).collect()
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

                let mut handlers : HashMap<i32, BindHandler> = HashMap::new();

                let mut clipboard_handlers : Vec<ClipboardHandler> = vec![];

                let hwnd = unsafe {
                    let class_name = to_wstring("meh_window");

                    let wnd_class = winapi::um::winuser::WNDCLASSW {
                        style : winapi::um::winuser::CS_OWNDC,		// Style
                        lpfnWndProc : Some(winapi::um::winuser::DefWindowProcW),			// The callbackfunction for any window event that can occur in our window!!! Here you could react to events like WM_SIZE or WM_QUIT.
                        hInstance : winapi::um::libloaderapi::GetModuleHandleW(  ::std::ptr::null_mut() ),							// The instance handle for our application which we can retrieve by calling GetModuleHandleW.
                        lpszClassName : class_name.as_ptr(),					// Our class name which needs to be a UTF-16 string (defined earlier before unsafe). as_ptr() (Rust's own function) returns a raw pointer to the slice's buffer
                        cbClsExtra : 0,
                        cbWndExtra : 0,
                        hIcon: ::std::ptr::null_mut(),
                        hCursor: ::std::ptr::null_mut(),
                        hbrBackground: ::std::ptr::null_mut(),
                        lpszMenuName: ::std::ptr::null_mut(),
                    };

                    // We have to register this class for Windows to use
                    winapi::um::winuser::RegisterClassW( &wnd_class );

                    let window_name = to_wstring("pusz temporary workaround to receive clipboard.");
                    let hwnd = winapi::um::winuser::CreateWindowExW(
                        0,

                        class_name.as_ptr(),
                        window_name.as_ptr(),
                        winapi::um::winuser::WS_VISIBLE,
                        0,
                        0,
                        0,
                        0,
                        ::std::ptr::null_mut(),
                        ::std::ptr::null_mut(),
                        ::std::ptr::null_mut(),
                        ::std::ptr::null_mut());

                    winapi::um::winuser::ShowWindow(hwnd, winapi::um::winuser::SW_HIDE);
                    println!("hwnd is: {:?} err: {:?}", hwnd, winapi::um::errhandlingapi::GetLastError());

                    hwnd
                };

                let win_pid = unsafe { winapi::um::processthreadsapi::GetCurrentProcessId() } ;
//                winapi::um::winuser::GetWindowThreadProcessId()

                    unsafe {
//                        winapi::um::winuser::EnumWindows(Some(find_window), win_pid as isize);
                    };

                loop {
                    match get_single_message() {
                        ReceivedMessage::Hotkey { id } => { println!("hotkey id'd triggered: {}", id);
                            if let Some(handler) = handlers.get(&id) {
                                handler();
                            }
                        },
                        ReceivedMessage::Nothing => {},
                        ReceivedMessage::ClipboardUpdate => {
                            println!("hai! we got a clipboard update now is: {:?}", clipboard_win::get_clipboard_string());
                        }
                    }

                    if let Ok(request) = rx.try_recv() {
                        match request {
                            WindowsApiEvent::HotkeyRegister { id, modifiers, vk, handler } => {
                                println!("got request to register hotkey!");
                                handlers.insert(id, handler);
                                unsafe {
                                    winapi::um::winuser::RegisterHotKey(
                                        0 as winapi::shared::windef::HWND,
                                        id,
                                        modifiers, vk
                                    );
                                }
                            },
                            WindowsApiEvent::AddClipboardListener { handler } => {
                                if clipboard_handlers.is_empty() {
                                    let result = unsafe { winapi::um::winuser::AddClipboardFormatListener(hwnd) };

                                    println!("we now added clip listener, eh? {} {:?}", result, unsafe { winapi::um::errhandlingapi::GetLastError() } );
                                }

                                clipboard_handlers.push(handler);
                            }
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Modifier {
    None = 0,
    Alt = 1,
    Ctrl = 2,
    Shift = 4,
    Win = 8,
}

impl Modifier {
    fn v(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Key {
    Return = winapi::um::winuser::VK_RETURN as isize,
    Control = winapi::um::winuser::VK_CONTROL as isize,
    Alt = winapi::um::winuser::VK_MENU as isize,
    Shift = winapi::um::winuser::VK_SHIFT as isize,
    A = 'A' as isize,
    B = 'B' as isize,
    C = 'C' as isize,
    D = 'D' as isize,
    E = 'E' as isize,
    F = 'F' as isize,
    G = 'G' as isize,
    H = 'H' as isize,
    I = 'I' as isize,
    J = 'J' as isize,
    K = 'K' as isize,
    L = 'L' as isize,
    M = 'M' as isize,
    N = 'N' as isize,
    O = 'O' as isize,
    P = 'P' as isize,
    Q = 'Q' as isize,
    R = 'R' as isize,
    S = 'S' as isize,
    T = 'T' as isize,
    U = 'U' as isize,
    V = 'V' as isize,
    W = 'W' as isize,
    X = 'X' as isize,
    Y = 'Y' as isize,
    Z = 'Z' as isize,
}

impl Key {
    fn v(self) -> u32 {
        self as u32
    }
}

lazy_static! {
    static ref HOTKEY_DAYA: Mutex<Option<HotkeyData>> = {
            Mutex::new(None)
    };
}

fn main() {
    HotkeyData::do_it(WindowsApiEvent::HotkeyRegister {modifiers : Modifier::None.v(), id : 7, vk : Key::B.v(), handler : Arc::new(|| { println!("hoho"); })});

    HotkeyData::do_it(WindowsApiEvent::AddClipboardListener { handler :  Arc::new(|text : String| println!("heya got clip: {}", text))});
    loop {
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
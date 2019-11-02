#![allow(unused)]

use std::sync::atomic;
use std::sync::{self, Mutex, Arc};
use std::sync::mpsc::{Sender, Receiver, self, channel};
use std::collections::HashMap;
use clipboard_win::{get_clipboard_string, set_clipboard_string};
use gdk::Window;

pub type BindHandler = Arc<dyn Fn(i32) + Send + Sync + 'static>;
pub type ClipboardHandler = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[allow(unused)]
pub enum WindowsApiEvent {
    HotkeyRegister { id : i32, modifiers : u32, vk : u32, handler : BindHandler},

    SetClipboard { text : String },
    AddClipboardListener { handler  : ClipboardHandler},
}

pub struct HotkeyData {
    thread_handle : Option<::std::thread::JoinHandle<()>>,
    thread_id : usize,
    tx : Option<Sender<WindowsApiEvent>>,
}

pub struct HotkeyProxy {
    thread_id : usize,
    tx : Sender<WindowsApiEvent>,
}

impl HotkeyProxy {
    pub fn post_event(&self, event : WindowsApiEvent) {
        self.tx.send(event).expect("post event failure");
        unsafe { winapi::um::winuser::PostThreadMessageA(self.thread_id as u32, 30000, 0, 0) };
    }
}

pub enum ReceivedMessage {
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
                return ReceivedMessage::Hotkey {id : msg.wParam as i32 };
            }
            WM_CLIPBOARDUPDATE => {
                return ReceivedMessage::ClipboardUpdate;
            }
            30000 => {
            }
            _ => {

            }
        }
    }

    ReceivedMessage::Nothing

}

fn to_wstring(str: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    ::std::ffi::OsStr::new(str).encode_wide().chain(Some(0).into_iter()).collect()
}

impl HotkeyData {
    pub fn do_it(hotkey : WindowsApiEvent) {
        Self::init().post_event(hotkey);
    }

    pub fn set_clipboard(text: &str) {
        Self::do_it(WindowsApiEvent::SetClipboard { text : text.to_owned()});
    }

    pub fn get_clipboard() -> Option<String> {
        get_clipboard_string().ok()
    }

    pub fn register_hotkey( id : i32, key : Key, modifiers : Modifier, handler : BindHandler) {
        Self::do_it(WindowsApiEvent::HotkeyRegister { id, handler, vk : key.v(), modifiers : modifiers.v()} )
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
                tx_tid.send(win_thread_id).expect("tid send failure");

                let mut handlers : HashMap<i32, BindHandler> = HashMap::new();
                let mut clipboard_handlers : Vec<ClipboardHandler> = vec![];

                let mut last_set_clipboard = String::new();

                let hwnd = unsafe {
                    let class_name = to_wstring("meh_window");

                    let wnd_class = winapi::um::winuser::WNDCLASSW {
                        style : winapi::um::winuser::CS_OWNDC,		// Style
                        lpfnWndProc : Some(winapi::um::winuser::DefWindowProcW),
                        hInstance : winapi::um::libloaderapi::GetModuleHandleW(  ::std::ptr::null_mut() ),
                        lpszClassName : class_name.as_ptr(),
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

                    hwnd
                };

                let win_pid = unsafe { winapi::um::processthreadsapi::GetCurrentProcessId() } ;
                loop {
                    match get_single_message() {
                        ReceivedMessage::Hotkey { id } => {
                            if let Some(handler) = handlers.get(&id) {
                                handler(id);
                            }
                        },
                        ReceivedMessage::Nothing => {},
                        ReceivedMessage::ClipboardUpdate => {
                            if let Ok(text) = get_clipboard_string() {
                                if text != last_set_clipboard {
                                    for listener in &clipboard_handlers {
                                        listener(text.clone());
                                    }
                                }
                            }

                            last_set_clipboard.clear();
                        }
                    }

                    if let Ok(request) = rx.try_recv() {
                        match request {
                            WindowsApiEvent::HotkeyRegister { id, modifiers, vk, handler } => {
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
                                    last_set_clipboard = get_clipboard_string().unwrap_or_default();

                                    unsafe { winapi::um::winuser::AddClipboardFormatListener(hwnd) };
                                }

                                clipboard_handlers.push(handler);
                            }
                            WindowsApiEvent::SetClipboard { text } => {
                                last_set_clipboard = text.clone();
                                let _ = set_clipboard_string(&text);
                            }
                        }
                    }
                }
            }));

            let thread_id = rx_tid.recv().expect("failed to recv thread_handle");

            *context = Some(HotkeyData {
                thread_id,
                thread_handle,
                tx : Some(tx),
            });
        }

        let context = context.as_ref().unwrap();

        HotkeyProxy {
            thread_id : context.thread_id,
            tx : context.tx.clone().unwrap(),
        }
    }

}

#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(unused)]
pub enum Modifier {
    None = 0,
    Alt = 1,
    Ctrl = 2,
    Shift = 4,
    Win = 8,
}

impl Modifier {
    pub fn v(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(unused)]
pub enum Key {
    Return = winapi::um::winuser::VK_RETURN as isize,
    Control = winapi::um::winuser::VK_CONTROL as isize,
    Alt = winapi::um::winuser::VK_MENU as isize,
    Shift = winapi::um::winuser::VK_SHIFT as isize,
    F1 = winapi::um::winuser::VK_F1 as isize,
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
    pub fn v(self) -> u32 {
        self as u32
    }
}

lazy_static! {
    static ref HOTKEY_DAYA: Mutex<Option<HotkeyData>> = {
            Mutex::new(None)
    };
}
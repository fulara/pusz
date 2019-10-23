use std::sync::atomic;
use std::sync::{self, Mutex, Arc};
use std::sync::mpsc::{Sender, Receiver, self, channel};
use std::collections::HashMap;
use clipboard_win::get_clipboard_string;

pub type BindHandler = Arc<dyn Fn() + Send + Sync + 'static>;
pub type ClipboardHandler = Arc<dyn Fn(String) + Send + Sync + 'static>;

pub enum WindowsApiEvent {
    HotkeyRegister { id : i32, modifiers : u32, vk : u32, handler : BindHandler},
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
        unsafe { winapi::um::winuser::PostThreadMessageA(self.thread_id as u32, 30000, 0, 0) };
        self.tx.send(event);
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
    pub fn do_it(hotkey : WindowsApiEvent) {
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
                    println!("hwnd is: {:?} err: {:?}", hwnd, winapi::um::errhandlingapi::GetLastError());

                    hwnd
                };

                let win_pid = unsafe { winapi::um::processthreadsapi::GetCurrentProcessId() } ;
                loop {
                    match get_single_message() {
                        ReceivedMessage::Hotkey { id } => { println!("hotkey id'd triggered: {}", id);
                            if let Some(handler) = handlers.get(&id) {
                                handler();
                            }
                        },
                        ReceivedMessage::Nothing => {},
                        ReceivedMessage::ClipboardUpdate => {
                            for listener in &clipboard_handlers {
                                if let Ok(text) = get_clipboard_string() {
                                    listener(text);
                                }
                            }
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
pub enum Key {
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
    pub fn v(self) -> u32 {
        self as u32
    }
}

lazy_static! {
    static ref HOTKEY_DAYA: Mutex<Option<HotkeyData>> = {
            Mutex::new(None)
    };
}
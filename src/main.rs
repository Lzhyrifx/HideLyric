use std::env;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::CloseHandle;

use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot,
    Process32FirstW,
    Process32NextW,
    PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::Graphics::Gdi::{
    CreateRectRgn,
    SetWindowRgn,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW,
    IsWindow,
    ShowWindow,
    SW_SHOWNOACTIVATE,
};


const SERVER_ADDR: &str = "127.0.0.1:10086";


// 超时退出
const DAEMON_TIMEOUT: Duration = Duration::from_secs(10);


struct ServerState {
    // 当前希望窗口是什么状态
    // true = show
    // false = hide
    desired_show: AtomicBool,

    // 每次状态变化都会增加
    // 用来让旧的hide重试失效
    generation: AtomicU64,

    // 最后一次收到JS请求的时间
    last_ping: Mutex<Instant>,
}


impl ServerState {
    fn new() -> Self {
        Self {
            desired_show: AtomicBool::new(true),
            generation: AtomicU64::new(0),
            last_ping: Mutex::new(Instant::now()),
        }
    }


    fn touch(&self) {
        if let Ok(mut last_ping) = self.last_ping.lock() {
            *last_ping = Instant::now();
        }
    }


    fn should_exit(&self) -> bool {
        match self.last_ping.lock() {
            Ok(last_ping) => (*last_ping).elapsed() > DAEMON_TIMEOUT,
            Err(_) => true,
        }
    }
}

// ==================================================
// 检测 cloudmusic.exe 是否正在运行
// ==================================================

fn is_cloudmusic_running() -> bool {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(
            TH32CS_SNAPPROCESS,
            0,
        ) {
            Ok(snapshot) => snapshot,

            Err(_) => {
                // 检测失败时不误判为进程已退出
                return true;
            }
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut found = false;

        if Process32FirstW(
            snapshot,
            &mut entry,
        ).is_ok() {
            loop {
                let process_name =
                    String::from_utf16_lossy(
                        &entry.szExeFile,
                    );

                if process_name
                    .trim_end_matches('\0')
                    .eq_ignore_ascii_case("cloudmusic.exe")
                {
                    found = true;
                    break;
                }

                if Process32NextW(
                    snapshot,
                    &mut entry,
                ).is_err()
                {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);

        found
    }
}

// 查找目标窗口
fn find_lyrics_window() -> Option<HWND> {
    unsafe {
        match FindWindowW(w!("DesktopLyrics"), None) {
            Ok(hwnd) => {
                if IsWindow(Some(hwnd)).as_bool() {
                    Some(hwnd)
                } else {
                    None
                }
            }

            Err(_) => None,
        }
    }
}


// 隐藏
fn hide_lyrics() -> bool {
    let hwnd = match find_lyrics_window() {
        Some(hwnd) => hwnd,
        None => {
            println!("[HideLyric] hide:找不到DesktopLyrics");
            return false;
        }
    };

    unsafe {
        // 创建滚木区域
        let region = CreateRectRgn(0, 0, 0, 0);

        let result = SetWindowRgn(
            hwnd,
            Some(region),
            true,
        );

        if result == 0 {
            println!("[HideLyric] hide:SetWindowRgn失败");
            return false;
        }

        true
    }
}


// 显示
fn show_lyrics() -> bool {
    let hwnd = match find_lyrics_window() {
        Some(hwnd) => hwnd,
        None => {
            println!("[HideLyric] show:找不到DesktopLyrics");
            return false;
        }
    };

    unsafe {
        let result = SetWindowRgn(
            hwnd,
            None,
            true,
        );

        if result == 0 {
            println!("[HideLyric] show:SetWindowRgn失败");
            return false;
        }

        let _ = ShowWindow(
            hwnd,
            SW_SHOWNOACTIVATE,
        );

        true
    }
}


// Hide+重试
fn start_hide(state: Arc<ServerState>) {
    (*state).desired_show.store(false, Ordering::SeqCst);

    let generation = (*state)
        .generation
        .fetch_add(1, Ordering::SeqCst)
        + 1;


    hide_lyrics();

    let retry_delays = [
        Duration::from_millis(50),
        Duration::from_millis(150),
        Duration::from_millis(350),
    ];

    for delay in retry_delays {
        let state_clone = Arc::clone(&state);

        thread::spawn(move || {
            thread::sleep(delay);

            // 已经出现新的show/hide操作,本次重试失效
            if (*state_clone)
                .generation
                .load(Ordering::SeqCst)
                != generation
            {
                return;
            }

            // 当前目标已经变为show,本次hide重试失效
            if (*state_clone)
                .desired_show
                .load(Ordering::SeqCst)
            {
                return;
            }

            hide_lyrics();
        });
    }
}


// Show
fn start_show(state: Arc<ServerState>) {
    (*state).desired_show.store(true, Ordering::SeqCst);

    // 让之前所有hide重试失效
    (*state).generation.fetch_add(1, Ordering::SeqCst);

    show_lyrics();
}


// HTTP 响应
fn write_response(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
) {
    let body_bytes = body.as_bytes();

    let response = format!(
        "HTTP/1.1 {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Access-Control-Allow-Private-Network: true\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        body_bytes.len(),
        body
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}


// HTTP 请求处理
fn handle_connection(
    mut stream: TcpStream,
    state: Arc<ServerState>,
) {
    let mut buffer = [0u8; 4096];

    let size = match stream.read(&mut buffer) {
        Ok(size) => size,
        Err(_) => return,
    };

    if size == 0 {
        return;
    }

    (*state).touch();

    let request = String::from_utf8_lossy(
        &buffer[..size],
    );

    let first_line = request
        .lines()
        .next()
        .unwrap_or("");

    println!(
        "[HideLyric] HTTP: {}",
        first_line
    );


    if first_line.starts_with("OPTIONS ") {
        write_response(
            &mut stream,
            "204 No Content",
            "",
        );

        let _ = stream.shutdown(Shutdown::Both);
        return;
    }


    let mut parts =
        first_line.split_whitespace();

    let method =
        parts.next().unwrap_or("");

    let path =
        parts.next().unwrap_or("");

    if method != "GET" {
        write_response(
            &mut stream,
            "405 Method Not Allowed",
            "method not allowed",
        );

        let _ = stream.shutdown(Shutdown::Both);
        return;
    }


    // ping
    if path == "/ping" {
        write_response(
            &mut stream,
            "200 OK",
            "pong",
        );

        let _ = stream.shutdown(Shutdown::Both);
        return;
    }


    // hide
    if path == "/hide" {
        start_hide(
            Arc::clone(&state),
        );

        write_response(
            &mut stream,
            "200 OK",
            "hidden",
        );

        let _ = stream.shutdown(Shutdown::Both);
        return;
    }


    // show
    if path == "/show" {
        start_show(
            Arc::clone(&state),
        );

        write_response(
            &mut stream,
            "200 OK",
            "shown",
        );

        let _ = stream.shutdown(Shutdown::Both);
        return;
    }


    // 404
    write_response(
        &mut stream,
        "404 Not Found",
        "not found",
    );

    let _ = stream.shutdown(Shutdown::Both);
}


// Rust常驻服务器
fn run_daemon() {
    println!(
        "[HideLyric] Listening on http://{}",
        SERVER_ADDR
    );

    // 创建TCP监听器
    let listener =
        match TcpListener::bind(SERVER_ADDR) {
            Ok(listener) => listener,

            Err(error) => {
                eprintln!(
                    "[HideLyric] 无法监听 {}: {:?}",
                    SERVER_ADDR,
                    error
                );

                return;
            }
        };

    // 非阻塞
    if let Err(error) =
        listener.set_nonblocking(true)
    {
        eprintln!(
            "[HideLyric] 设置 nonblocking 失败: {:?}",
            error
        );

        return;
    }

    let state =
        Arc::new(ServerState::new());

    // ==================================================
    // 心跳与 cloudmusic.exe 进程监控线程
    // ==================================================

    {
        let state = Arc::clone(&state);

        thread::spawn(move || {

            // 是否检测到过网易云进程
            let mut cloudmusic_seen = false;

            loop {
                thread::sleep(
                    Duration::from_millis(200)
                );

                // ------------------------------------------
                // 检测网易云进程
                // ------------------------------------------

                if is_cloudmusic_running() {
                    cloudmusic_seen = true;

                } else if cloudmusic_seen {
                    println!(
                        "[HideLyric] 检测到 cloudmusic.exe 已退出，daemon退出"
                    );

                    std::process::exit(0);
                }

                // ------------------------------------------
                // 检测 JS 心跳超时
                // ------------------------------------------

                if state.should_exit() {
                    println!(
                        "[HideLyric] JS心跳超时，daemon退出"
                    );

                    std::process::exit(0);
                }
            }
        });
    }


    // HTTP 循环
    loop {
        match listener.accept() {

            Ok((stream, _addr)) => {
                let state =
                    Arc::clone(&state);

                thread::spawn(move || {
                    handle_connection(
                        stream,
                        state,
                    );
                });
            }

            Err(error)
            if error.kind()
                == std::io::ErrorKind::WouldBlock =>
                {
                    thread::sleep(
                        Duration::from_millis(5)
                    );
                }

            Err(error) => {
                eprintln!(
                    "[HideLyric] accept失败: {:?}",
                    error
                );

                thread::sleep(
                    Duration::from_millis(100)
                );
            }
        }
    }
}


fn main() {
    let args: Vec<String> =
        env::args().collect();

    if args.len() < 2 {
        println!(
            "Usage: HideLyric.exe daemon"
        );

        return;
    }

    match args[1].as_str() {

        "daemon" => {
            run_daemon();
        }

        _ => {
            println!(
                "Usage: HideLyric.exe daemon"
            );
        }
    }
}
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

pub fn setup_spinner(
    loading: Arc<AtomicBool>,
    message: Option<&'static str>,
) -> thread::JoinHandle<()> {
    let loading_thread = loading.clone();
    let message = message.unwrap_or("");
    thread::spawn(move || {
        const FRAMES: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        let mut idx = 0;
        print!("\x1b[?25l");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        while loading_thread.load(Ordering::Relaxed) {
            print!("\r[{}] {}\r\x1b[0m", FRAMES[idx], message);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            idx = (idx + 1) % FRAMES.len();
            thread::sleep(Duration::from_millis(100));
        }
        print!("\r \r\x1b[?25h");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
    })
}

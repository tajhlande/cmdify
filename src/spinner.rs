use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const FRAMES: &[char] = &['|', '\\', '/', '-'];

const FRAMES_BRAILLE: &[char] = &['⣾', '⣷', '⣯', '⣟', '⡿', '⢿', '⣻', '⣽'];

const FRAMES_DOTS: &[char] = &['⋅', '.', '˳', '˳', '.', '⋅', 'ॱ', '˙', '˙', 'ॱ'];

const FRAME_INTERVAL: Duration = Duration::from_millis(120);

fn frames_for(selection: u8) -> &'static [char] {
    match selection {
        2 => FRAMES_BRAILLE,
        3 => FRAMES_DOTS,
        _ => FRAMES,
    }
}

pub struct Spinner {
    running: Option<Arc<AtomicBool>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.store(false, Ordering::Relaxed);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let mut stderr = io::stderr();
        let _ = write!(stderr, "\r   \r");
        let _ = stderr.flush();
    }
}

impl Spinner {
    pub fn start(selection: u8) -> Self {
        if !io::stderr().is_terminal() {
            return Self {
                running: None,
                handle: None,
            };
        }

        let frames = frames_for(selection);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let mut stderr = io::stderr();

        let handle = thread::spawn(move || {
            let mut frame_idx = 0;
            while running_clone.load(Ordering::Relaxed) {
                let frame = frames[frame_idx % frames.len()];
                let _ = write!(stderr, "\r{} ", frame);
                let _ = stderr.flush();
                frame_idx += 1;
                thread::sleep(FRAME_INTERVAL);
            }
        });

        Self {
            running: Some(running),
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        if let Some(running) = self.running.take() {
            running.store(false, Ordering::Relaxed);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let mut stderr = io::stderr();
        let _ = write!(stderr, "\r   \r");
        let _ = stderr.flush();
    }
}

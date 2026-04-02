use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const FRAMES: &[char] = &['|', '\\', '/', '-'];
const FRAMES_2: &[char] = &['⣾', '⣷', '⣯', '⣟', '⣻', '⣽', '⣾', '⣷'];
const FRAMES_3: &[char] = &['⋅', '.', '˳', '˳', '.', '⋅', 'ॱ', '˙', '˙', 'ॱ'];
const FRAME_INTERVAL: Duration = Duration::from_millis(120);

pub struct Throbber {
    running: Option<Arc<AtomicBool>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Throbber {
    pub fn start() -> Self {
        if !io::stderr().is_terminal() {
            return Self {
                running: None,
                handle: None,
            };
        }

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let mut stderr = io::stderr();

        let handle = thread::spawn(move || {
            let mut frame_idx = 0;
            while running_clone.load(Ordering::Relaxed) {
                let frame = FRAMES_3[frame_idx % FRAMES_3.len()];
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

    pub fn stop(self) {
        if let Some(running) = self.running {
            running.store(false, Ordering::Relaxed);
        }
        if let Some(handle) = self.handle {
            let _ = handle.join();
        }
        let mut stderr = io::stderr();
        let _ = write!(stderr, "\r   \r");
        let _ = stderr.flush();
    }
}

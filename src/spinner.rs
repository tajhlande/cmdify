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

#[derive(Clone)]
pub struct SpinnerPause {
    paused: Arc<AtomicBool>,
}

impl SpinnerPause {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
}

pub struct Spinner {
    running: Option<Arc<AtomicBool>>,
    handle: Option<thread::JoinHandle<()>>,
    #[allow(dead_code)]
    pause: SpinnerPause,
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
                pause: SpinnerPause {
                    paused: Arc::new(AtomicBool::new(false)),
                },
            };
        }

        let frames = frames_for(selection);
        let running = Arc::new(AtomicBool::new(true));
        let paused = Arc::new(AtomicBool::new(false));
        let running_clone = running.clone();
        let paused_clone = paused.clone();
        let mut stderr = io::stderr();

        let handle = thread::spawn(move || {
            let mut frame_idx = 0;
            while running_clone.load(Ordering::Relaxed) {
                if !paused_clone.load(Ordering::Relaxed) {
                    let frame = frames[frame_idx % frames.len()];
                    let _ = write!(stderr, "\r{} ", frame);
                    let _ = stderr.flush();
                    frame_idx += 1;
                }
                thread::sleep(FRAME_INTERVAL);
            }
        });

        Self {
            running: Some(running),
            handle: Some(handle),
            pause: SpinnerPause { paused },
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

    pub fn pause_handle(&self) -> SpinnerPause {
        self.pause.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_handle_defaults_not_paused() {
        let pause = SpinnerPause {
            paused: Arc::new(AtomicBool::new(false)),
        };
        assert!(!pause.is_paused());
    }

    #[test]
    fn pause_and_resume() {
        let pause = SpinnerPause {
            paused: Arc::new(AtomicBool::new(false)),
        };
        pause.pause();
        assert!(pause.is_paused());
        pause.resume();
        assert!(!pause.is_paused());
    }

    #[test]
    fn pause_handle_cloned_independently() {
        let pause = SpinnerPause {
            paused: Arc::new(AtomicBool::new(false)),
        };
        let clone = pause.clone();
        pause.pause();
        assert!(clone.is_paused());
        clone.resume();
        assert!(!pause.is_paused());
    }
}

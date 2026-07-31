//! Framework-free file watching.
//!
//! Watches a single file and invokes a callback when it changes on disk. The caller decides
//! what to do — the viewers flag the open file as *outdated* rather than auto-reloading. Uses
//! `notify` with a debouncer so an editor's burst of save events collapses into one notification.

use std::path::Path;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};

/// An active file watch. Dropping it stops watching.
pub struct FileWatch {
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// Watch `path` for on-disk modifications, calling `on_change` (debounced by `debounce`) each
/// time it changes. Returns an error if the path can't be watched.
pub fn watch_file(
    path: impl AsRef<Path>,
    debounce: Duration,
    on_change: impl Fn() + Send + 'static,
) -> Result<FileWatch, String> {
    let mut debouncer = new_debouncer(debounce, move |res: DebounceEventResult| {
        // A non-empty batch means the file changed; the error case (watch dropped/errored) is
        // ignored — the worst outcome is simply not flagging the file as outdated.
        if let Ok(events) = res {
            if !events.is_empty() {
                on_change();
            }
        }
    })
    .map_err(|e| e.to_string())?;

    debouncer
        .watcher()
        .watch(path.as_ref(), RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    Ok(FileWatch {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::mpsc;

    #[test]
    fn fires_on_modification() {
        let path =
            std::env::temp_dir().join(format!("window_core_watch_{}.txt", std::process::id()));
        std::fs::write(&path, b"one").unwrap();

        let (tx, rx) = mpsc::channel();
        let _watch = watch_file(&path, Duration::from_millis(100), move || {
            let _ = tx.send(());
        })
        .expect("watch should start");

        // Let the watcher arm, then modify the file.
        std::thread::sleep(Duration::from_millis(300));
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            f.write_all(b" two").unwrap();
            f.flush().unwrap();
        }

        let fired = rx.recv_timeout(Duration::from_secs(5)).is_ok();
        let _ = std::fs::remove_file(&path);
        assert!(fired, "watcher should fire after the file is modified");
    }
}

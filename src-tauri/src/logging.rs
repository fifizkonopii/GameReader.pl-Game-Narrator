use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

// In-memory ring buffer of recent formatted log lines, so the debug console can
// show history when it's opened (not just lines emitted while it was open).
const MAX_HISTORY: usize = 2000;
static LOG_HISTORY: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn history() -> &'static Mutex<VecDeque<String>> {
    LOG_HISTORY.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_HISTORY)))
}

fn push_history(line: &str) {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
        return;
    }
    if let Ok(mut h) = history().lock() {
        if h.len() >= MAX_HISTORY {
            h.pop_front();
        }
        h.push_back(trimmed.to_string());
    }
}

/// Return the recent log lines (oldest first) for populating the debug console.
pub fn recent_logs() -> Vec<String> {
    history()
        .lock()
        .map(|h| h.iter().cloned().collect())
        .unwrap_or_default()
}

// ============================================================
// USER-FRIENDLY LOG STREAM
// ============================================================
// A second, plain-language stream meant for non-technical users. Key program
// events (reader on/off, OCR, matching, audio, presets, errors) call
// `user_log(...)` with a short human-readable message. These are buffered and
// forwarded to the debug console's "Prosty" (simple) view.
static USER_LOG_TX: OnceLock<UnboundedSender<String>> = OnceLock::new();
static USER_LOG_HISTORY: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn user_history() -> &'static Mutex<VecDeque<String>> {
    USER_LOG_HISTORY.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_HISTORY)))
}

/// Emit a human-friendly log line (shown in the debug console's simple view).
/// Safe to call from any thread; if logging isn't initialized yet the message
/// is still kept in history.
pub fn user_log(message: impl AsRef<str>) {
    let ts = chrono::Local::now().format("%H:%M:%S");
    let line = format!("{}  {}", ts, message.as_ref());
    if let Ok(mut h) = user_history().lock() {
        if h.len() >= MAX_HISTORY {
            h.pop_front();
        }
        h.push_back(line.clone());
    }
    if let Some(tx) = USER_LOG_TX.get() {
        let _ = tx.send(line);
    }
}

/// Return recent user-friendly log lines for populating the simple view.
pub fn recent_user_logs() -> Vec<String> {
    user_history()
        .lock()
        .map(|h| h.iter().cloned().collect())
        .unwrap_or_default()
}

/// Receivers returned from `init_logging`, drained in `lib.rs` and forwarded to
/// the debug console as events.
pub struct LogReceivers {
    pub raw: UnboundedReceiver<String>,
    pub user: UnboundedReceiver<String>,
}

/// A writer that records each formatted log line into the history ring buffer
/// and forwards it through a channel (drained in `lib.rs` and emitted to the
/// debug window as `log_line` events).
struct ChannelWriter {
    tx: UnboundedSender<String>,
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            push_history(s);
            let line = s.trim_end_matches(['\r', '\n']).to_string();
            if !line.is_empty() {
                let _ = self.tx.send(line);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
struct ChannelMakeWriter {
    tx: UnboundedSender<String>,
}

impl<'a> fmt::MakeWriter<'a> for ChannelMakeWriter {
    type Writer = ChannelWriter;
    fn make_writer(&'a self) -> Self::Writer {
        ChannelWriter { tx: self.tx.clone() }
    }
}

/// Initialize logging infrastructure.
///
/// Logs go to three places with identical formatting:
/// - stdout / cmd terminal
/// - `log.txt` in the app data directory
/// - the in-app debug console (via the returned channel receiver)
///
/// Returns a receiver that yields every formatted log line; `lib.rs` drains it
/// and forwards lines to the debug window.
pub fn init_logging(app_dir: PathBuf) -> anyhow::Result<LogReceivers> {
    // Create log file appender in app directory.
    // Rotate daily and keep a limited number of files so the log directory
    // doesn't grow without bound (was a single ever-growing log.txt).
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("gamereader")
        .filename_suffix("log")
        .max_log_files(7)
        .build(app_dir)?;

    // Create a formatting layer for the file
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false) // No ANSI colors in file
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(true);

    // Create a formatting layer for stdout (console)
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true) // Colors in terminal
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(true);

    // Channel layer: mirrors the same output to the in-app debug console.
    let (tx, rx) = unbounded_channel::<String>();
    let channel_layer = fmt::layer()
        .with_writer(ChannelMakeWriter { tx })
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(true);

    // Set up env filter (default to INFO level)
    // Can be overridden with RUST_LOG environment variable.
    //
    // The symphonia MP3 decoder emits a flood of harmless WARN messages
    // ("skipping junk", "invalid mpeg audio header") for files that contain
    // ID3 tags / metadata. Silence those decoder crates to keep logs readable.
    let default_filter = "info,\
symphonia_bundle_mp3=error,\
symphonia_core=error,\
symphonia_metadata=error,\
symphonia_format_ogg=error,\
symphonia=error";
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));

    // Combine layers and initialize global subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(stdout_layer)
        .with(channel_layer)
        .init();

    tracing::info!(
        "Logging initialized - version {} {}",
        crate::constants::APP_VERSION,
        crate::constants::APP_VERSION_TAG
    );

    // Set up the user-friendly log channel.
    let (user_tx, user_rx) = unbounded_channel::<String>();
    let _ = USER_LOG_TX.set(user_tx);

    Ok(LogReceivers { raw: rx, user: user_rx })
}

/// Log levels match Python logger levels:
/// - ERROR: Critical errors
/// - WARN: Warnings
/// - INFO: General information
/// - DEBUG: Detailed debug information
/// - TRACE: Very detailed trace information

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_logging_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let app_dir = temp_dir.path().to_path_buf();
        
        // Initialize logging
        let result = init_logging(app_dir);
        assert!(result.is_ok());
        
        // Note: log file is created on first write
    }
}

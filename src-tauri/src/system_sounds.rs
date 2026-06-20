//! Bundled UI/system notification sounds.
//!
//! The small .ogg files are embedded directly into the binary (`include_bytes!`)
//! and played on a short-lived, detached audio stream. This is independent of
//! the dialogue audio pipeline and works identically in dev and release without
//! any filesystem path resolution.

use std::io::Cursor;

const ON: &[u8] = include_bytes!("../sounds/on.ogg");
const OFF: &[u8] = include_bytes!("../sounds/off.ogg");
const TEST: &[u8] = include_bytes!("../sounds/test.ogg");
const PING: &[u8] = include_bytes!("../sounds/ping.ogg");
const AREA1: &[u8] = include_bytes!("../sounds/area1.ogg");
const AREA2: &[u8] = include_bytes!("../sounds/area2.ogg");
const ANNOUNCEMENT: &[u8] = include_bytes!("../sounds/announcement.ogg");

fn bytes_for(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "on" => ON,
        "off" => OFF,
        "test" => TEST,
        "ping" => PING,
        "area1" => AREA1,
        "area2" => AREA2,
        "announcement" => ANNOUNCEMENT,
        _ => return None,
    })
}

/// Play a bundled system sound by name (e.g. "on", "off", "test", "area1").
///
/// Non-blocking: runs on a short-lived thread with its own output stream, so it
/// never interferes with the dialogue audio and can be called from any thread.
pub fn play(name: &str) {
    let Some(data) = bytes_for(name) else {
        tracing::warn!("Unknown system sound: {}", name);
        return;
    };
    let name = name.to_string();
    std::thread::spawn(move || {
        let (_stream, handle) = match rodio::OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("System sound '{}': no audio output: {}", name, e);
                return;
            }
        };
        let sink = match rodio::Sink::try_new(&handle) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("System sound '{}': failed to create sink: {}", name, e);
                return;
            }
        };
        match rodio::Decoder::new(Cursor::new(data)) {
            Ok(decoder) => {
                sink.append(decoder);
                sink.sleep_until_end();
            }
            Err(e) => tracing::warn!("System sound '{}': decode failed: {}", name, e),
        }
    });
}

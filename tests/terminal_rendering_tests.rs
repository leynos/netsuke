//! Verify Netsuke terminal rendering through pipes and Unix pseudo-terminals.

#[path = "terminal_rendering/accessibility.rs"]
mod accessibility;
#[cfg(unix)]
#[path = "terminal_rendering/accessibility_pty.rs"]
mod accessibility_pty;
#[path = "terminal_rendering/emoji.rs"]
mod emoji;
#[cfg(unix)]
#[path = "terminal_rendering/emoji_pty.rs"]
mod emoji_pty;
#[path = "terminal_rendering/json.rs"]
mod json;
#[cfg(unix)]
#[path = "terminal_rendering/json_pty.rs"]
mod json_pty;
#[path = "terminal_rendering/progress.rs"]
mod progress;
#[cfg(unix)]
#[path = "terminal_rendering/progress_pty.rs"]
mod progress_pty;
#[path = "terminal_rendering/support.rs"]
mod support;

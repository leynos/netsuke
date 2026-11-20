//! The PowerShell wrapper that surrounds a documented Windows `PATH` fence.
//!
//! The wrapper owns the user environment registry key for the duration of one
//! run: it records the existing value, installs a sentinel, dot-sources the
//! documented script, reports what was stored and its type, and restores the
//! original value whatever the script does. It also repairs the state a killed
//! run leaves, and can reproduce that state for the recovery test.
//!
//! The script lives in `wrapper.ps1` beside this module rather than in a Rust
//! string literal: the file can then be read, linted and parsed as PowerShell,
//! and this module holds only the Rust view of it.

/// Sentinel user `PATH` value every run starts from.
///
/// It is an expandable string, so the example's promise to preserve the value's
/// type is observable: a script that rewrote `PATH` as a plain string would
/// fail the assertion in the test binary even though the directory was added.
pub const SENTINEL_PATH: &str = "%SystemRoot%\\netsuke-path-sentinel";

/// The wrapper that surrounds the documented script.
///
/// It passes `-ExamplePath` to the script, owns the registry key for the
/// duration of the run, and reports the observations the assertions use.
///
/// Its variables carry a `$wrapper` prefix because the documented script is
/// dot-sourced into this scope so that its failures propagate unchanged. A
/// script that declares `$environmentKey` — both documented fences do — would
/// otherwise replace the wrapper's handle, and the `Dispose()` call at the end
/// of the script would close the key the wrapper still needs to read.
///
/// Two runs must not interleave. Each run replaces the user `PATH` with
/// `SENTINEL_PATH`, a fixed value, so a run that started while another was in
/// flight would capture the sentinel as its original and could put it back
/// over the real value. A named mutex serialises them; the backup file, which
/// is written before the sentinel and removed after the restore, lets a later
/// run put the real value back when a wrapper is killed before its `finally`
/// block runs. The file is named by the caller because the recovery has to
/// outlive the temp directory of the run that wrote it. It records the
/// original value, its type, the sentinel, and the directory that run would
/// append, because the recovering run cannot see the killed run's workspace.
/// A run killed after its restore but before the delete leaves the backup
/// recording the value the registry already holds; that state is recognised as
/// a completed restore and the spent backup is removed.
///
/// `-SimulateInterruption` stops the process after it has written the backup
/// and the value a killed run leaves, and before anything restores them, so
/// the next run performs the recovery in a genuinely separate process, against
/// an abandoned mutex, rather than being handed the state directly.
/// `-SimulateSpentBackup` stops it after the restore but before the backup is
/// deleted, which is the other state a killed run can leave behind.
pub const WRAPPER: &str = include_str!("wrapper.ps1");

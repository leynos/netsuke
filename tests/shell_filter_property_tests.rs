//! Property and contract tests for the `shell_quote` and `shell_join` filters.
//!
//! Each obligation in the plan has one property that states it over a generated
//! domain and, beside it, a control that shows the property can fail. A test
//! that only ever passes is not evidence, so most of the work here is arranging
//! for the fixtures to be wrong in a way the assertion must notice: a broken
//! quoter through the real harness, a POSIX-quoted string through the PowerShell
//! decoder, an auto-escaping environment through the verbatim check.
//!
//! The obligations are split across sibling modules, one per group, so a suite
//! can be read on its own; the harness they share lives in [`property_support`].
//! The split keeps each file within `AGENTS.md`'s 400-line cap.
//!
//! | module | obligations |
//! |--------|-------------|
//! | [`round_trip_through_an_oracle`] | OBL-SH-ROUNDTRIP, OBL-PS-ROUNDTRIP |
//! | [`word_and_join`] | OBL-ONE-WORD, OBL-JOIN-SPLIT, OBL-JOIN-QUOTE-AGREE |
//! | [`kind_gate`] | OBL-KIND-GATE |
//! | [`manifest_rendering`] | OBL-NO-ESCAPE, OBL-CONTEXT |

#[path = "shell_filter_property_tests/property_support.rs"]
mod property_support;

#[path = "shell_filter_property_tests/round_trip_through_an_oracle.rs"]
mod round_trip_through_an_oracle;

#[path = "shell_filter_property_tests/word_and_join.rs"]
mod word_and_join;

#[path = "shell_filter_property_tests/kind_gate.rs"]
mod kind_gate;

#[path = "shell_filter_property_tests/manifest_rendering.rs"]
mod manifest_rendering;

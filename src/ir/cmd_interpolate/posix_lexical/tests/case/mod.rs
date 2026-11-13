//! Unit tests for the `case` construct's effect on substitution boundaries.
//!
//! A `)` terminating a `case` pattern is an operator, while the `)` closing a
//! command substitution stays in the surrounding word. These modules pin the
//! boundary the scanner draws between the two, and the tracker state that makes
//! the boundary provable: the reserved words it must recognise, and the
//! comments whose text it must never read.

mod boundary;
mod tracker;

//! The private snapshot tree a `MiniJinja` value is admitted into.
//!
//! [RFC 0006 §6.7] defines canonical equality as byte-identity of RFC 8785
//! canonical JSON. Reaching that form means the emitted bytes must be decided
//! solely by the value's content, never by the disposition of the `MiniJinja`
//! host object behind it: a custom object that reorders its enumerator would
//! otherwise yield two different keys for one value.
//!
//! This tree is the seam that prevents it. A host value is traversed exactly
//! once into a representation that owns its data and keeps the source numeric
//! width, so a wide integer is not narrowed before the canonicalizer sees it.
//! Only this tree is serialized, and a host object is never re-read after
//! admission.
//!
//! The tree is not the canonical form itself. RFC 8785 renders every number as
//! an IEEE-754 binary64, so distinct nodes can still share one key.
//!
//! [RFC 0006 §6.7]: ../../../docs/rfcs/0006-ansible-inspired-template-standard-library.md

use std::collections::BTreeMap;

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json_canonicalizer::to_vec;

use crate::stdlib::canonical_value::error::CanonicalValueError;

/// One admitted value, with its numeric width preserved.
///
/// A mapping is held as a [`BTreeMap`] because RFC 8785 orders object members
/// by property name. The canonicalizer sorts them again in UTF-16 order, so
/// this map's ordering only has to be deterministic, not canonical.
#[derive(Clone, Debug)]
pub(super) enum SnapshotNode {
    /// The JSON `null` literal, from a `MiniJinja` none value.
    Null,
    /// A boolean.
    Bool(bool),
    /// A UTF-8 string.
    String(String),
    /// A signed integer, kept at its source width.
    I128(i128),
    /// An unsigned integer, kept at its source width.
    U128(u128),
    /// A finite binary64 floating-point number.
    F64(f64),
    /// An ordered sequence of admitted values.
    Seq(Vec<Self>),
    /// A string-keyed mapping of admitted values.
    Map(BTreeMap<String, Self>),
}

/// Serialize `snapshot` to RFC 8785 canonical JSON bytes.
pub(super) fn to_canonical_bytes(snapshot: &SnapshotNode) -> Result<Vec<u8>, CanonicalValueError> {
    to_vec(snapshot).map_err(|source| CanonicalValueError::Serialization { source })
}

impl Serialize for SnapshotNode {
    /// Write the snapshot in the JSON shape RFC 8785 canonicalizes.
    ///
    /// Integers are written as integers rather than as floats. The tree has
    /// already refused every non-finite number, and the canonicalizer casts
    /// each integer width to binary64 itself, so casting here would only
    /// discard the fidelity the tree exists to keep.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::String(text) => serializer.serialize_str(text),
            Self::I128(value) => serializer.serialize_i128(*value),
            Self::U128(value) => serializer.serialize_u128(*value),
            Self::F64(value) => serializer.serialize_f64(*value),
            Self::Seq(items) => serialize_sequence(items, serializer),
            Self::Map(members) => serialize_mapping(members, serializer),
        }
    }
}

/// Serialize a sequence snapshot as a JSON array.
fn serialize_sequence<S: Serializer>(
    items: &[SnapshotNode],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_seq(Some(items.len()))?;
    for item in items {
        seq.serialize_element(item)?;
    }
    seq.end()
}

/// Serialize a mapping snapshot as a JSON object.
fn serialize_mapping<S: Serializer>(
    members: &BTreeMap<String, SnapshotNode>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(members.len()))?;
    for (key, value) in members {
        map.serialize_entry(key, value)?;
    }
    map.end()
}

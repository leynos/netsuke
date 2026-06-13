//! Bounded map used by Kani to verify IR production code paths.
//!
//! This module is compiled only under `cfg(kani)`.  It gives the IR graph a
//! small deterministic map with the same methods used by production code, so
//! Kani harnesses can drive the real lowering and cycle-detection logic
//! without depending on `std::collections::HashMap`.

use std::borrow::Borrow;

use camino::{Utf8Path, Utf8PathBuf};

const KANI_IR_HASHMAP_CAPACITY: usize = 4;

/// Map used by the IR graph under Kani.
#[derive(Debug, Clone, PartialEq)]
pub struct IrHashMap<K, V> {
    entries: [Option<(K, V)>; KANI_IR_HASHMAP_CAPACITY],
    len: usize,
}

/// Equality used by the Kani map when replacing existing keys.
pub trait IrMapKeyEq {
    /// Return `true` when two keys identify the same IR map entry.
    fn ir_map_key_eq(&self, other: &Self) -> bool;
}

impl IrMapKeyEq for String {
    fn ir_map_key_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl IrMapKeyEq for Utf8PathBuf {
    fn ir_map_key_eq(&self, other: &Self) -> bool {
        bounded_path_key_eq(self, other)
    }
}

impl IrMapKeyEq for &Utf8Path {
    fn ir_map_key_eq(&self, other: &Self) -> bool {
        bounded_path_eq(self, other)
    }
}

fn bounded_path_key_eq(left: &Utf8PathBuf, right: &Utf8PathBuf) -> bool {
    bounded_path_eq(left.as_path(), right.as_path())
}

fn bounded_path_eq(left: &Utf8Path, right: &Utf8Path) -> bool {
    let left = left.as_str().as_bytes();
    let right = right.as_str().as_bytes();
    left.len() == 1 && right.len() == 1 && left[0] == right[0]
}

impl<K, V> Default for IrHashMap<K, V> {
    fn default() -> Self {
        Self {
            entries: [None, None, None, None],
            len: 0,
        }
    }
}

impl<K, V> IrHashMap<K, V>
where
    K: IrMapKeyEq,
{
    /// Insert `value` at `key`, replacing and returning any previous value.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let mut index = 0;
        while index < self.len {
            if let Some((candidate, stored)) = &mut self.entries[index] {
                if candidate.ir_map_key_eq(&key) {
                    return Some(std::mem::replace(stored, value));
                }
            }
            index += 1;
        }
        assert!(
            self.len < KANI_IR_HASHMAP_CAPACITY,
            "Kani IR map capacity exceeded",
        );
        self.entries[self.len] = Some((key, value));
        self.len += 1;
        None
    }
}

impl<K, V> IrHashMap<K, V> {
    /// Return the value for `key`, if present.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        let mut index = 0;
        while index < self.len {
            if let Some((candidate, value)) = &self.entries[index] {
                if candidate.borrow() == key {
                    return Some(value);
                }
            }
            index += 1;
        }
        None
    }

    /// Return `true` when `key` is present in the map.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.get(key).is_some()
    }

    /// Iterate over keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries[..self.len]
            .iter()
            .filter_map(|entry| entry.as_ref().map(|(key, _)| key))
    }

    /// Return the key at `index`, if present.
    pub fn key_at(&self, index: usize) -> Option<&K> {
        self.entry_at(index).map(|(key, _)| key)
    }

    /// Return the key-value pair at `index`, if present.
    pub fn entry_at(&self, index: usize) -> Option<(&K, &V)> {
        if index < self.len {
            match &self.entries[index] {
                Some((key, value)) => Some((key, value)),
                None => None,
            }
        } else {
            None
        }
    }

    /// Iterate over key-value pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries[..self.len]
            .iter()
            .filter_map(|entry| entry.as_ref().map(|(key, value)| (key, value)))
    }

    /// Iterate over values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries[..self.len]
            .iter()
            .filter_map(|entry| entry.as_ref().map(|(_, value)| value))
    }

    /// Remove every entry from the map.
    pub fn clear(&mut self) {
        let mut index = 0;
        while index < self.len {
            self.entries[index] = None;
            index += 1;
        }
        self.len = 0;
    }

    /// Return the number of entries in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return `true` when the map contains no entries.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

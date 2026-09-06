//! Streams rendered template output through manifest-owned byte accounting.

use super::{ManifestBudget, ManifestBudgetExhaustion, ManifestBudgetKind, ManifestBudgetStage};
use minijinja::{Error, ErrorKind};
use std::{cell::Cell, io};

/// Write rendered bytes into a bounded value buffer.
pub(crate) struct CappedWriter<'a> {
    /// Shares aggregate accounting with the rest of the manifest parse.
    budget: &'a ManifestBudget,
    /// Holds only bytes accepted below both configured ceilings.
    output: Vec<u8>,
    /// Preserves the exact local exhaustion when `MiniJinja` maps I/O to `WriteFailure`.
    exhaustion: Cell<Option<ManifestBudgetExhaustion>>,
}

impl<'a> CappedWriter<'a> {
    /// Construct an empty bounded rendered-value buffer.
    pub(super) const fn new(budget: &'a ManifestBudget) -> Self {
        Self {
            budget,
            output: Vec::new(),
            exhaustion: Cell::new(None),
        }
    }

    /// Return the accepted bytes as UTF-8 after `MiniJinja` finishes rendering.
    pub(crate) fn into_string(self) -> std::result::Result<String, Error> {
        String::from_utf8(self.output).map_err(|_| {
            Error::new(
                ErrorKind::BadSerialization,
                "MiniJinja emitted non-UTF-8 output",
            )
        })
    }

    /// Return the resource exhaustion captured by a failed write.
    pub(crate) const fn exhaustion(&self) -> Option<ManifestBudgetExhaustion> {
        self.exhaustion.get()
    }
}

impl io::Write for CappedWriter<'_> {
    /// Accept a whole `MiniJinja` write only when it fits all byte ceilings.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next_len = self.output.len().saturating_add(bytes.len());
        if next_len > self.budget.limits.rendered_value_bytes {
            self.exhaustion.set(Some(ManifestBudget::exhaustion(
                ManifestBudgetKind::ValueBytes,
                ManifestBudgetStage::Render,
                self.budget.limits.rendered_value_bytes as u64,
            )));
            return Err(io::Error::other("manifest rendered value budget exhausted"));
        }
        if let Err(exhaustion) = self.budget.charge_rendered_bytes(bytes.len()) {
            self.exhaustion.set(Some(exhaustion));
            return Err(io::Error::other("manifest rendered byte budget exhausted"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    /// Flush the in-memory buffer without side effects.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

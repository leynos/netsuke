//! Naming, content-addressing, and emission of one serial edge's staged block.
//!
//! [`super`] owns bundle assembly: it decides which edges need staging, writes
//! the surrounding file, and returns the artefact. This module owns the
//! *format* of a staged block — the identity a gate set is named from, the
//! dyndep document a sidecar carries, how that document is content-addressed,
//! and the three Ninja statements each stage contributes.
//!
//! The boundary is ownership of the emitted staging shape rather than a line
//! count: a change to what a stage looks like lands here, while a change to how
//! bundles are assembled stays in the parent. The reserved-namespace constants
//! stay in the parent because path rejection needs them too.
//!
//! Why a sidecar reveals a relay phony instead of the real dependency is
//! recorded in
//! [ADR-0058](../../../docs/adrs/adr-0058-reveal-a-relay-phony-to-avoid-the-ninja-out-edge-reallocation-crash.md),
//! and summarised in [`super`]'s module documentation.

use super::{DYNDEP_NAMESPACE, GeneratedDyndep, SERIAL_NAMESPACE};
use crate::hex;
use crate::ir::BuildEdge;
use crate::ninja_gen::{NinjaGenError, validated_ninja_path};
use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;

/// Schema tag incorporated into every parent (gate-set) identity.
const PARENT_SCHEMA: &str = "netsuke-serial-v1";
/// Format tag incorporated into every dyndep filename digest.
const DYNDEP_SCHEMA: &str = "netsuke-dyndep-v1";
/// Filename prefix distinguishing a stage's relay phony from its gate.
const RELAY_PREFIX: &str = "relay-";

/// Mutable staging state shared while lowering one serial edge.
///
/// [`super`] threads this through its edge walk and takes the collected
/// sidecars out of [`Self::dyndep_files`] when it builds the artefact.
#[derive(Default)]
pub(super) struct SerialStages {
    /// Dyndep sidecars collected while lowering serial edges.
    pub(super) dyndep_files: Vec<GeneratedDyndep>,
    /// Sidecar paths already staged, deduplicating shared dependencies.
    staged_sidecars: HashSet<Utf8PathBuf>,
}

/// Emit the staged relays, gates, and sidecar-producing phony edges for one
/// serial edge, collecting each sidecar into the bundle and returning the gate
/// paths in dependency order.
///
/// Each stage emits three statements — a relay carrying the real dependency, a
/// phony producing the sidecar, and the gate — so a serial edge with `N`
/// implicit dependencies contributes `1 + 3N` statements to the file.
pub(super) fn render_serial_block(
    edge: &BuildEdge,
    out: &mut String,
    stages: &mut SerialStages,
    gate_paths: &mut Vec<Utf8PathBuf>,
) -> Result<(), NinjaGenError> {
    let parent = parent_identity(edge);
    for (index, dep) in edge.implicit_deps.iter().enumerate() {
        let gate = parent.join(format!("{index:03}"));
        let relay = parent.join(format!("{RELAY_PREFIX}{index:03}"));
        let content = sidecar_content(&gate, &relay)?;
        let digest = sidecar_digest(&content);
        let sidecar = Utf8PathBuf::from(format!("{DYNDEP_NAMESPACE}/{digest}.dd"));

        let sidecar_escaped = validated_ninja_path(sidecar.as_str())?;
        let gate_escaped = validated_ninja_path(gate.as_str())?;
        let relay_escaped = validated_ninja_path(relay.as_str())?;
        let dep_escaped = validated_ninja_path(dep.as_str())?;

        // The relay carries the real dependency and is what the sidecar
        // reveals. It is emitted before the gate so the revealed name is
        // already declared where the file is read top to bottom.
        writeln!(out, "build {relay_escaped}: phony | {dep_escaped}")?;
        // The phony edge that produces (but never rebuilds) the sidecar file.
        // Starting at the second stage it depends on the previous gate, which
        // prevents Ninja from revealing the next sidecar early.
        match gate_paths.last() {
            None => writeln!(out, "build {sidecar_escaped}: phony")?,
            Some(prev) => {
                let prev_escaped = validated_ninja_path(prev.as_str())?;
                writeln!(out, "build {sidecar_escaped}: phony {prev_escaped}")?;
            }
        }
        // The gate edge: order-only depends on the sidecar and declares it as
        // its dyndep file so Ninja loads the revealed relay from the sidecar.
        writeln!(out, "build {gate_escaped}: phony || {sidecar_escaped}")?;
        writeln!(out, "  dyndep = {sidecar_escaped}")?;
        writeln!(out)?;

        if stages.staged_sidecars.insert(sidecar.clone()) {
            stages.dyndep_files.push(GeneratedDyndep {
                relative_path: sidecar,
                content,
            });
        }
        gate_paths.push(gate);
    }
    Ok(())
}

/// Derive the stable parent (gate-set) identity for a serial edge.
///
/// The identity is a SHA-256 over the edge canonical output identity and an
/// explicit schema tag, so renaming the output or changing the staging format
/// yields a fresh namespace without colliding with other serial edges.
fn parent_identity(edge: &BuildEdge) -> Utf8PathBuf {
    let canonical = edge
        .explicit_outputs
        .iter()
        .map(|p| p.as_str())
        .collect::<Vec<_>>()
        .join("\u{0}");
    let mut hasher = Sha256::new();
    hasher.update(PARENT_SCHEMA.as_bytes());
    hasher.update(b"\0");
    hasher.update(canonical.as_bytes());
    let digest = hex::to_lower_hex(&hasher.finalize());
    Utf8PathBuf::from(format!("{SERIAL_NAMESPACE}/{digest}"))
}

/// Render the dyndep document revealing one stage's relay phony.
///
/// The revealed path is the relay rather than the real dependency so that
/// Ninja's `AddOutEdge` targets a node with no other consumer; see the module
/// documentation for the upstream crash this avoids.
fn sidecar_content(gate: &Utf8PathBuf, relay: &Utf8PathBuf) -> Result<String, NinjaGenError> {
    let gate_escaped = validated_ninja_path(gate.as_str())?;
    let relay_escaped = validated_ninja_path(relay.as_str())?;
    Ok(format!(
        "ninja_dyndep_version = 1\nbuild {gate_escaped}: dyndep | {relay_escaped}\n"
    ))
}

/// Content-address a sidecar by its complete bytes and a format tag.
fn sidecar_digest(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DYNDEP_SCHEMA.as_bytes());
    hasher.update(b"\0");
    hasher.update(content.as_bytes());
    hex::to_lower_hex(&hasher.finalize())
}

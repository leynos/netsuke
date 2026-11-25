//! Runner-owned dyndep bundle telemetry and sidecar publication.

pub(in crate::runner) mod generation_telemetry;
mod publication;

pub(in crate::runner) use publication::{
    DyndepPublication, materialize_dyndep_bundle, materialize_dyndep_bundle_in,
    prune_dyndep_bundle, prune_dyndep_bundle_in,
};

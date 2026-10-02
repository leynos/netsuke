//! Own manifest environment reads and their bounded telemetry.

mod reader;
mod telemetry;

pub use reader::{EnvReadError, EnvReader, ManifestEnvironment, process_env_reader};
pub(in crate::manifest) use reader::{disabled_env_reader, env_var_with_default};
pub use telemetry::{ENV_LOOKUP_OUTCOME_VALUES, ENV_LOOKUP_TOTAL};

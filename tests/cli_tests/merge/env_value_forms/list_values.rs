//! Supported and unsupported list environment forms.

use super::*;

/// A scalar `NETSUKE_DEFAULT_TARGETS` is read as a one-element list.
#[test]
fn scalar_default_targets_merge() -> Result<()> {
    let merged = merge_with_environment(&[("NETSUKE_DEFAULT_TARGETS", "hello.txt")], &["netsuke"])?;
    ensure!(
        merged.default_targets == ["hello.txt"],
        "a scalar list value should merge as one target, got {:?}",
        merged.default_targets,
    );
    Ok(())
}

/// A serialized JSON array is read as a list.
#[test]
fn json_array_default_targets_merge() -> Result<()> {
    let merged = merge_with_environment(
        &[("NETSUKE_DEFAULT_TARGETS", r#"["hello.txt","world.txt"]"#)],
        &["netsuke"],
    )?;
    ensure!(
        merged.default_targets == ["hello.txt", "world.txt"],
        "a JSON array should merge as two targets, got {:?}",
        merged.default_targets,
    );
    Ok(())
}

/// Indexed `__N` variables are still rejected as a map, not a list.
#[test]
fn indexed_default_targets_are_rejected_as_a_map() -> Result<()> {
    let error =
        merge_with_environment_error(&[("NETSUKE_DEFAULT_TARGETS__0", "hello.txt")], &["netsuke"])?;
    ensure!(
        error.contains("invalid type: map"),
        "an indexed variable should nest into a map and fail list deserialization, got: {error}",
    );
    Ok(())
}

use super::*;
use rstest::rstest;
use std::path::PathBuf;

#[rstest]
#[case(None, "out.ninja", "out.ninja")]
#[case(Some("work"), "out.ninja", "work/out.ninja")]
#[case(Some("work"), "/tmp/out.ninja", "/tmp/out.ninja")]
fn resolve_output_path_respects_directory(
    #[case] directory: Option<&str>,
    #[case] input: &str,
    #[case] expected: &str,
) {
    let cli = Cli {
        directory: directory.map(PathBuf::from),
        ..Cli::default()
    };
    let resolved = resolve_output_path(&cli, Path::new(input));
    assert_eq!(resolved.as_ref(), Path::new(expected));
}

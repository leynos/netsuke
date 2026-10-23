//! Pin the finite ADR-019 registry independently of the compiling host.

use super::{BuiltInExecutable, BuiltInShell, HostFamily};
use crate::{recipe_shell::RecipeShell, shell_selection::ShellName};
use pretty_assertions::assert_eq;
use rstest::rstest;
use std::fmt::Write;

/// Force a new enum variant to update the independent expected-name table.
const fn expected_name(shell: BuiltInShell) -> &'static str {
    match shell {
        BuiltInShell::Sh => "sh",
        BuiltInShell::Bash => "bash",
        BuiltInShell::Pwsh => "pwsh",
        BuiltInShell::PowerShell => "powershell",
    }
}

#[test]
fn all_lists_every_variant() {
    use BuiltInShell::{Bash, PowerShell, Pwsh, Sh};
    assert_eq!(BuiltInShell::ALL, [Sh, Bash, Pwsh, PowerShell]);
    for shell in BuiltInShell::ALL {
        assert_eq!(shell.name(), expected_name(shell));
    }
}

#[test]
fn built_in_names_are_exactly_the_adr_019_set() {
    assert_eq!(
        BuiltInShell::ALL.map(BuiltInShell::name),
        ["sh", "bash", "pwsh", "powershell"]
    );
}

#[rstest]
fn built_in_names_satisfy_the_grammar(
    #[values(
        BuiltInShell::Sh,
        BuiltInShell::Bash,
        BuiltInShell::Pwsh,
        BuiltInShell::PowerShell
    )]
    shell: BuiltInShell,
) {
    assert_eq!(ShellName::parse(shell.name()), Ok(ShellName::from(shell)));
}

#[rstest]
fn lookup_round_trips(
    #[values(
        BuiltInShell::Sh,
        BuiltInShell::Bash,
        BuiltInShell::Pwsh,
        BuiltInShell::PowerShell
    )]
    shell: BuiltInShell,
) {
    assert_eq!(BuiltInShell::lookup(&ShellName::from(shell)), Some(shell));
    assert_eq!(shell.to_string(), expected_name(shell));
}

#[rstest]
#[case("dash")]
#[case("zsh")]
#[case("cmd")]
#[case("sh2")]
#[case("shh")]
#[case("bashx")]
fn lookup_excludes_other_valid_names(#[case] text: &str) {
    let name = ShellName::parse(text).expect("configured-name witness must parse");
    assert_eq!(BuiltInShell::lookup(&name), None);
}

#[rstest]
#[case(BuiltInShell::Sh, HostFamily::UnixLike, true)]
#[case(BuiltInShell::Sh, HostFamily::Windows, false)]
#[case(BuiltInShell::Bash, HostFamily::UnixLike, true)]
#[case(BuiltInShell::Bash, HostFamily::Windows, true)]
#[case(BuiltInShell::Pwsh, HostFamily::UnixLike, true)]
#[case(BuiltInShell::Pwsh, HostFamily::Windows, true)]
#[case(BuiltInShell::PowerShell, HostFamily::UnixLike, false)]
#[case(BuiltInShell::PowerShell, HostFamily::Windows, true)]
fn support_matrix(#[case] shell: BuiltInShell, #[case] host: HostFamily, #[case] expected: bool) {
    assert_eq!(shell.is_supported_on(host), expected);
}

#[rstest]
#[case(HostFamily::UnixLike, BuiltInShell::Sh)]
#[case(HostFamily::Windows, BuiltInShell::PowerShell)]
fn host_default_matches_the_adr(#[case] host: HostFamily, #[case] expected: BuiltInShell) {
    assert_eq!(BuiltInShell::host_default(host), expected);
    assert!(BuiltInShell::host_default(host).is_supported_on(host));
}

#[test]
fn current_host_agrees_with_legacy_recipe_default() {
    assert_eq!(
        HostFamily::current() == HostFamily::Windows,
        RecipeShell::host_default() == RecipeShell::PowerShell
    );
}

#[rstest]
#[case(BuiltInShell::Sh, BuiltInExecutable::AbsolutePath("/bin/sh"), &["-c"])]
#[case(BuiltInShell::Bash, BuiltInExecutable::BareName("bash"), &["--noprofile", "--norc", "-c"])]
#[case(BuiltInShell::Pwsh, BuiltInExecutable::BareName("pwsh"), &["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])]
#[case(BuiltInShell::PowerShell, BuiltInExecutable::BareName("powershell.exe"), &["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])]
fn fixed_invocations(
    #[case] shell: BuiltInShell,
    #[case] executable: BuiltInExecutable,
    #[case] args: &[&str],
) {
    assert_eq!(shell.executable(), executable);
    assert_eq!(shell.fixed_args(), args);
}

#[test]
fn invocation_shapes_are_well_formed() {
    for shell in BuiltInShell::ALL {
        let args = shell.fixed_args();
        let empty_args: &[&str] = &[];
        assert_ne!(args, empty_args);
        assert!(
            args.iter()
                .all(|arg| !arg.is_empty() && !arg.contains('\0'))
        );
        match shell.executable() {
            BuiltInExecutable::AbsolutePath(path) => assert!(path.starts_with('/')),
            BuiltInExecutable::BareName(name) => assert!(!name.contains(['/', '\\'])),
        }
    }
}

#[test]
fn host_families_are_complete() {
    assert_eq!(HostFamily::ALL, [HostFamily::UnixLike, HostFamily::Windows]);
}

#[test]
fn built_in_registry_table() {
    let mut rendered = String::from("name | unix-like | windows | executable | fixed arguments\n");
    for shell in BuiltInShell::ALL {
        let (kind, executable) = match shell.executable() {
            BuiltInExecutable::AbsolutePath(path) => ("absolute-path", path),
            BuiltInExecutable::BareName(name) => ("bare-name", name),
        };
        writeln!(
            rendered,
            "{} | {} | {} | {kind} {executable} | {}",
            shell.name(),
            shell.is_supported_on(HostFamily::UnixLike),
            shell.is_supported_on(HostFamily::Windows),
            shell.fixed_args().join(" ")
        )
        .expect("registry row must write to a String");
    }
    for (label, host) in [
        ("unix-like", HostFamily::UnixLike),
        ("windows", HostFamily::Windows),
    ] {
        writeln!(
            rendered,
            "{label} default: {}",
            BuiltInShell::host_default(host)
        )
        .expect("default row must write to a String");
    }
    crate::snapshot_test_support::snapshot_settings("shell_selection").bind(|| {
        insta::assert_snapshot!("built_in_registry_table", rendered);
    });
}

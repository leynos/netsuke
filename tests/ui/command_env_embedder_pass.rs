//! Compile-pass fixture: the explicit Ninja environment API as an external
//! embedder sees it.
//!
//! Compiled by `tests/command_env_ui_tests.rs` against the `netsuke` rlib with
//! no special visibility, so it proves `CommandEnv`, the request structs, and
//! `run_ninja_with`/`run_ninja_tool_with` are reachable and composable from
//! outside the crate. Nothing here runs: the harness stops at `--emit=metadata`.

use std::io;
use std::path::Path;

use netsuke::cli::Cli;
use netsuke::runner::{
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaToolRequest, run_ninja_tool_with,
    run_ninja_with,
};

/// The pieces an embedder would hold before building requests.
struct Parts<'a> {
    program: &'a Path,
    cli: &'a Cli,
    build_file: &'a Path,
    targets: &'a BuildTargets<'a>,
    env: &'a CommandEnv,
}

/// An embedder can construct both request bundles around one `CommandEnv`.
fn compose_requests<'a>(parts: &Parts<'a>) -> (NinjaBuildRequest<'a>, NinjaToolRequest<'a>) {
    let build = NinjaBuildRequest {
        program: parts.program,
        cli: parts.cli,
        build_file: parts.build_file,
        targets: parts.targets,
        env: parts.env,
    };
    let tool = NinjaToolRequest {
        program: parts.program,
        cli: parts.cli,
        build_file: parts.build_file,
        tool: "clean",
        env: parts.env,
    };
    (build, tool)
}

fn main() {
    // The builder surface composes without touching the process environment.
    let env = CommandEnv::inherit()
        .with_var("NINJA_STATUS", "[%f/%t] ")
        .with_path("/opt/toolchain/bin");
    assert!(!env.is_empty());
    assert!(env.get("PATH").is_some());

    let cli = Cli::default();
    let targets = BuildTargets::default();
    let parts = Parts {
        program: Path::new("ninja"),
        cli: &cli,
        build_file: Path::new("build.ninja"),
        targets: &targets,
        env: &env,
    };
    let (build, tool) = compose_requests(&parts);

    // Reference the explicit entry points by signature; calling them would
    // spawn a process, which a compile-only fixture must not do.
    let _run: fn(&NinjaBuildRequest<'_>) -> io::Result<()> = run_ninja_with;
    let _run_tool: fn(&NinjaToolRequest<'_>) -> io::Result<()> = run_ninja_tool_with;
    let _ = (build, tool);
}

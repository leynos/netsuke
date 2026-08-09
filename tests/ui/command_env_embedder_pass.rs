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
    BuildTargets, CommandEnv, NinjaBuildRequest, NinjaToolRequest, StderrMode, run_ninja_tool_with,
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
        stderr_mode: StderrMode::from_cli(parts.cli),
    };
    let tool = NinjaToolRequest {
        program: parts.program,
        cli: parts.cli,
        build_file: parts.build_file,
        tool: "clean",
        env: parts.env,
        stderr_mode: StderrMode::from_cli(parts.cli),
    };
    (build, tool)
}

fn main() -> io::Result<()> {
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

    // Genuine calls to the explicit entry points. The harness compiles this
    // fixture with --emit=metadata and never runs it, so no process spawns;
    // what is proven is that an embedder can drive both boundaries with the
    // requests composed above.
    run_ninja_with(&build)?;
    run_ninja_tool_with(&tool)?;
    Ok(())
}

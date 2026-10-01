//! Validate capability-step boundaries and scheduling diagnostics.

use super::*;
use rstest::rstest;

/// Render eight capability steps without depending on the live roadmap.
fn roadmap() -> String {
    let steps = (2..10)
        .map(|step| format!("### 6.{step}. Capability\n- `helper{step}`\n"))
        .collect::<Vec<_>>()
        .concat();
    format!(
        "## 6. Template standard-library expansion\n### 6.1. Shared\n`ignored`\n{steps}### 6.10. Deferred\n`deferred`\n"
    )
}

/// Parse one isolated roadmap without writing tracked documents.
fn read(text: &str) -> Result<Steps> {
    let temporary = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(temporary.path()).context("UTF-8 fixture root")?;
    let dir = cap_std::fs_utf8::Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    dir.create_dir_all("docs")?;
    dir.write(ROADMAP, text)?;
    parse(&Repo::fixture(root)?)
}

#[test]
fn schedules_only_real_capability_step_bodies() -> Result<()> {
    let text = roadmap().replace(
        "- `helper2`",
        "- `helper2`\n```md\n### 6.5. Example\n`ghost`\n```",
    );
    let steps = read(&text)?;
    ensure!(
        steps.order.len() == 8,
        "fixture result differs from expected contract"
    );
    ensure!(
        steps.all_names().len() == 8,
        "fixture result differs from expected contract"
    );
    ensure!(!steps.all_names().contains("ignored"));
    ensure!(!steps.all_names().contains("ghost"));
    ensure!(!steps.all_names().contains("deferred"));
    let names = vec!["helper2".into(), "absent".into()];
    ensure!(
        steps.unscheduled(&names) == vec!["absent"],
        "fixture result differs from expected contract"
    );
    ensure!(
        steps.missing_from_step("6.3", &names)? == names,
        "fixture result differs from expected contract"
    );
    ensure!(
        steps
            .names_for("6.11")
            .err()
            .context("unknown step")?
            .to_string()
            .contains("roadmap step 6.11 does not exist")
    );
    Ok(())
}

#[rstest]
#[case::section("section", "docs/roadmap.md has no section 6")]
#[case::start("start", "no capability steps starting 6.2.")]
#[case::count("count", "yields 7 capability steps")]
#[case::names("names", "no roadmap capability step names a backticked helper")]
#[case::phase("phase", "not numbered under phase 6")]
fn rejects_invalid_capability_structure(#[case] mutation: &str, #[case] diagnostic: &str) {
    let valid = roadmap();
    let text = match mutation {
        "section" => valid.replace("## 6.", "## 5."),
        "start" => valid.replace("### 6.2.", "### 6.1."),
        "count" => valid.replace("### 6.3. Capability\n- `helper3`\n", ""),
        "names" => valid.replace('`', ""),
        "phase" => valid.replace("### 6.3.", "### 7.3."),
        _ => valid,
    };
    let error = read(&text).err().expect("invalid fixture must fail");
    assert!(error.to_string().contains(diagnostic), "{error:#}");
}

#[test]
fn range_latch_opens_and_closes_at_exact_step_boundaries() -> Result<()> {
    let mut latch = false;
    ensure!(
        step_of("6.1. Shared", &mut latch)?.is_none(),
        "fixture result differs from expected contract"
    );
    ensure!(
        step_of("6.2. First", &mut latch)? == Some("6.2".into()),
        "fixture result differs from expected contract"
    );
    ensure!(
        step_of("6.9. Last", &mut latch)? == Some("6.9".into()),
        "fixture result differs from expected contract"
    );
    ensure!(
        step_of("6.10. Deferred", &mut latch)?.is_none(),
        "fixture result differs from expected contract"
    );
    ensure!(!latch);
    Ok(())
}

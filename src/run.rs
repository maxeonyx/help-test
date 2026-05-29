use crate::discover::discover_subcommands;
use crate::parse::{find_short_flag_violations, parse_examples};
use crate::{fixture_for_page, Fixture, HelpTest};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

pub(crate) fn run_help_test(help_test: HelpTest, binary_path: &Path) {
    let mut failures = Vec::new();
    let mut visited = BTreeSet::new();
    visit_page(
        &help_test,
        binary_path,
        &mut failures,
        &mut visited,
        Vec::new(),
    );

    if !failures.is_empty() {
        panic!("help test failed:\n  {}", failures.join("\n  "));
    }
}

fn visit_page(
    help_test: &HelpTest,
    binary_path: &Path,
    failures: &mut Vec<String>,
    visited: &mut BTreeSet<Vec<String>>,
    command_path: Vec<String>,
) {
    if !visited.insert(command_path.clone()) {
        return;
    }

    let fixture = match fixture_for_page(help_test, &command_path) {
        Some(fixture) => fixture,
        None => {
            failures.push(format!(
                "page {}: missing .page() declaration",
                describe_page(&command_path)
            ));
            Fixture::default()
        }
    };

    let help_output = match run_help_command(binary_path, &command_path, &fixture) {
        Ok(output) => output,
        Err(error) => {
            failures.push(format!("page {}: {error}", describe_page(&command_path)));
            return;
        }
    };

    let command_words = help_test
        .display_command
        .iter()
        .cloned()
        .chain(command_path.iter().cloned())
        .collect::<Vec<_>>();

    match parse_examples(&help_output, &command_words) {
        Ok(examples) => {
            for example in examples {
                let short_flag_violations =
                    find_short_flag_violations(&example.args, &help_test.allow_short_flags);
                if !short_flag_violations.is_empty() {
                    failures.push(format!(
                        "page {}: {}: short flags not allowed: {}",
                        describe_page(&command_path),
                        example.line,
                        short_flag_violations.join(", ")
                    ));
                }

                if let Err(error) = run_example(binary_path, &command_path, &fixture, &example.args)
                {
                    failures.push(format!(
                        "page {}: {}: {error}",
                        describe_page(&command_path),
                        example.line
                    ));
                }
            }
        }
        Err(parse_failures) => {
            for failure in parse_failures {
                failures.push(format!("page {}: {failure}", describe_page(&command_path)));
            }
        }
    }

    for subcommand in discover_subcommands(&help_output) {
        let mut next_path = command_path.clone();
        next_path.push(subcommand);
        visit_page(help_test, binary_path, failures, visited, next_path);
    }
}

fn run_help_command(
    binary_path: &Path,
    command_path: &[String],
    fixture: &Fixture,
) -> Result<String, String> {
    let output = run_command(binary_path, command_path, &["--help".to_owned()], fixture)?;
    Ok(output.stdout)
}

fn run_example(
    binary_path: &Path,
    command_path: &[String],
    fixture: &Fixture,
    args: &[String],
) -> Result<(), String> {
    run_command(binary_path, command_path, args, fixture).map(|_| ())
}

fn run_command(
    binary_path: &Path,
    command_path: &[String],
    args: &[String],
    fixture: &Fixture,
) -> Result<CommandOutput, String> {
    let temp_dir = TempDir::new().map_err(|error| format!("failed to create tempdir: {error}"))?;
    materialize_fixture(temp_dir.path(), fixture)?;

    let mut command = Command::new(binary_path);
    command
        .args(command_path)
        .args(args)
        .current_dir(temp_dir.path())
        .envs(&fixture.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn command: {error}"))?;

    if let Some(stdin) = &fixture.stdin {
        child
            .stdin
            .as_mut()
            .expect("stdin should be piped")
            .write_all(stdin)
            .map_err(|error| format!("failed to write stdin: {error}"))?;
    }

    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for command: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        return Err(format!(
            "command failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        ));
    }

    Ok(CommandOutput { stdout })
}

fn materialize_fixture(root: &Path, fixture: &Fixture) -> Result<(), String> {
    for dir in &fixture.dirs {
        fs::create_dir_all(resolve_fixture_path(root, dir)?)
            .map_err(|error| format!("failed to create dir {}: {error}", dir.display()))?;
    }

    for file in &fixture.files {
        let path = resolve_fixture_path(root, &file.path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create parent dir for {}: {error}",
                    file.path.display()
                )
            })?;
        }
        fs::write(&path, &file.content)
            .map_err(|error| format!("failed to write {}: {error}", file.path.display()))?;
    }

    for command in &fixture.commands {
        let status = Command::new(&command.program)
            .args(&command.args)
            .current_dir(root)
            .envs(&fixture.env)
            .status()
            .map_err(|error| {
                format!("failed to run fixture command {}: {error}", command.program)
            })?;

        if !status.success() {
            return Err(format!(
                "fixture command {} {:?} failed with status {}",
                command.program, command.args, status
            ));
        }
    }

    Ok(())
}

fn resolve_fixture_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Err(format!("fixture path must be relative: {}", path.display()));
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "fixture path must not escape tempdir: {}",
            path.display()
        ));
    }

    Ok(root.join(path))
}

fn describe_page(command_path: &[String]) -> String {
    if command_path.is_empty() {
        "<root>".to_owned()
    } else {
        command_path.join(" ")
    }
}

struct CommandOutput {
    stdout: String,
}

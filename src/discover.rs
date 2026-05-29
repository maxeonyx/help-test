pub(crate) fn discover_subcommands(help_output: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_commands_section = false;

    for line in help_output.lines() {
        if is_commands_header(line) {
            in_commands_section = true;
            continue;
        }

        if !in_commands_section {
            continue;
        }

        if line.trim().is_empty() {
            break;
        }

        if !line.starts_with(' ') && line.ends_with(':') {
            break;
        }

        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }

        let Some(name) = trimmed.split_whitespace().next() else {
            continue;
        };

        if name.starts_with('-') {
            continue;
        }

        commands.push(name.to_owned());
    }

    commands
}

fn is_commands_header(line: &str) -> bool {
    matches!(line.trim(), "Commands:" | "Subcommands:")
}

#[cfg(test)]
mod tests {
    use super::discover_subcommands;

    #[test]
    fn reads_commands_section() {
        let commands = discover_subcommands(
            "Usage: demo\n\nCommands:\n  run   Execute work\n  test  Run tests\n\nOptions:\n  -h, --help\n",
        );

        assert_eq!(commands, ["run", "test"]);
    }
}

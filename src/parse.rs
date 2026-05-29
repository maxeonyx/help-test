use std::collections::BTreeSet;

const EXAMPLE_PREFIX: &str = "$";

#[derive(Debug)]
pub(crate) struct Example {
    pub(crate) line: String,
    pub(crate) args: Vec<String>,
}

pub(crate) fn parse_examples(
    help_output: &str,
    command_words: &[String],
) -> Result<Vec<Example>, Vec<String>> {
    let mut examples = Vec::new();
    let mut failures = Vec::new();

    for raw_line in help_output.lines() {
        let Some(example_text) = raw_line.trim_start().strip_prefix(EXAMPLE_PREFIX) else {
            continue;
        };

        let example_text = example_text.trim_start();
        if example_text.is_empty() {
            continue;
        }

        let tool_segment = example_text
            .rsplit('|')
            .next()
            .expect("rsplit always returns at least one segment")
            .trim();

        let tool_segment = strip_shell_comment(tool_segment);

        match split_shell_words(tool_segment) {
            Ok(words) => {
                if words.starts_with(command_words) {
                    examples.push(Example {
                        line: raw_line.trim().to_owned(),
                        args: words[command_words.len()..].to_vec(),
                    });
                }
            }
            Err(error) => failures.push(format!("{}: {error}", raw_line.trim())),
        }
    }

    if failures.is_empty() {
        Ok(examples)
    } else {
        Err(failures)
    }
}

fn strip_shell_comment(input: &str) -> &str {
    let mut chars = input.char_indices().peekable();
    let mut quote = None;

    while let Some((idx, ch)) = chars.next() {
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                }
            }
            Some('"') => match ch {
                '"' => quote = None,
                '\\' => {
                    let _ = chars.next();
                }
                _ => {}
            },
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '#' => {
                    let starts_comment =
                        idx == 0 || input[..idx].chars().last().is_some_and(char::is_whitespace);
                    if starts_comment {
                        return input[..idx].trim_end();
                    }
                }
                '\\' => {
                    let _ = chars.next();
                }
                _ => {}
            },
            Some(other) => unreachable!("unexpected quote state: {other}"),
        }
    }

    input
}

pub(crate) fn find_short_flag_violations(
    args: &[String],
    allowlist: &BTreeSet<String>,
) -> Vec<String> {
    let mut violations = Vec::new();

    for arg in args {
        if arg == "--" {
            break;
        }

        if is_short_flag(arg) {
            let flag = arg.trim_start_matches('-');
            if !allowlist.contains(flag) {
                violations.push(arg.clone());
            }
        }
    }

    violations
}

fn is_short_flag(arg: &str) -> bool {
    let mut chars = arg.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some('-'), Some(letter), None) if letter.is_ascii_alphabetic()
    )
}

fn split_shell_words(input: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some('\'') => match ch {
                '\'' => quote = None,
                _ => current.push(ch),
            },
            Some('"') => match ch {
                '"' => quote = None,
                '\\' => match chars.next() {
                    Some(escaped @ ('"' | '\\' | '$' | '`')) => current.push(escaped),
                    Some('\n') => {}
                    Some(other) => {
                        current.push('\\');
                        current.push(other);
                    }
                    None => current.push('\\'),
                },
                _ => current.push(ch),
            },
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '\\' => match chars.next() {
                    Some('\n') => {}
                    Some(other) => current.push(other),
                    None => return Err("trailing backslash".to_owned()),
                },
                ch if ch.is_whitespace() => {
                    if !current.is_empty() {
                        words.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(ch),
            },
            Some(other) => unreachable!("unexpected quote state: {other}"),
        }
    }

    if let Some(quote) = quote {
        return Err(format!("unterminated {quote} quote"));
    }

    if !current.is_empty() {
        words.push(current);
    }

    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::{find_short_flag_violations, parse_examples};
    use std::collections::BTreeSet;

    #[test]
    fn extracts_examples_for_display_command_after_pipe() {
        let examples = parse_examples(
            "Examples:\n  $ seq 1 10 | cargo ratchet --json\n  $ other-tool --help\n",
            &["cargo".to_owned(), "ratchet".to_owned()],
        )
        .unwrap();

        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].args, ["--json"]);
    }

    #[test]
    fn rejects_unallowlisted_short_flags_until_double_dash() {
        let violations = find_short_flag_violations(
            &[
                "-f".to_owned(),
                "--first".to_owned(),
                "--".to_owned(),
                "-x".to_owned(),
            ],
            &BTreeSet::from(["h".to_owned()]),
        );

        assert_eq!(violations, ["-f"]);
    }

    #[test]
    fn strips_subcommand_prefix_for_subcommand_pages() {
        let examples = parse_examples(
            "Examples:\n  $ cargo ratchet run --json\n",
            &["cargo".to_owned(), "ratchet".to_owned(), "run".to_owned()],
        )
        .unwrap();

        assert_eq!(examples[0].args, ["--json"]);
    }

    #[test]
    fn strips_unquoted_inline_shell_comments() {
        let examples = parse_examples(
            "Examples:\n  $ cargo ratchet --init  # initialize from current state\n",
            &["cargo".to_owned(), "ratchet".to_owned()],
        )
        .unwrap();

        assert_eq!(examples[0].args, ["--init"]);
    }

    #[test]
    fn preserves_hash_inside_quotes() {
        let examples = parse_examples(
            "Examples:\n  $ cargo ratchet --message '# still an arg'\n",
            &["cargo".to_owned(), "ratchet".to_owned()],
        )
        .unwrap();

        assert_eq!(examples[0].args, ["--message", "# still an arg"]);
    }
}

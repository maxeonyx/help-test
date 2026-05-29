mod discover;
mod parse;
mod run;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

type PageSetup = Box<dyn Fn(&mut Fixture)>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ExampleKey {
    command_path: Vec<String>,
    args: Vec<String>,
}

pub struct HelpTest {
    binary_name: String,
    display_command: Vec<String>,
    allow_short_flags: BTreeSet<String>,
    pages: BTreeMap<Vec<String>, PageSetup>,
    examples: BTreeMap<ExampleKey, PageSetup>,
}

impl HelpTest {
    pub fn new(binary_name: &str) -> Self {
        Self {
            binary_name: binary_name.to_owned(),
            display_command: vec![binary_name.to_owned()],
            allow_short_flags: BTreeSet::new(),
            pages: BTreeMap::new(),
            examples: BTreeMap::new(),
        }
    }

    pub fn display_command(mut self, cmd: &[&str]) -> Self {
        assert!(
            !cmd.is_empty(),
            "display command must contain at least one word"
        );

        self.display_command = cmd.iter().map(|part| (*part).to_owned()).collect();
        self
    }

    pub fn allow_short_flags(mut self, flags: &[&str]) -> Self {
        self.allow_short_flags = flags.iter().map(|flag| (*flag).to_owned()).collect();
        self
    }

    pub fn page(mut self, command_path: &[&str], setup: impl Fn(&mut Fixture) + 'static) -> Self {
        let key = command_path
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect::<Vec<_>>();

        let previous = self.pages.insert(key.clone(), Box::new(setup));
        assert!(previous.is_none(), "duplicate page declaration: {key:?}");
        self
    }

    pub fn example(
        mut self,
        command_path: &[&str],
        args: &[&str],
        setup: impl Fn(&mut Fixture) + 'static,
    ) -> Self {
        let key = ExampleKey {
            command_path: command_path
                .iter()
                .map(|segment| (*segment).to_owned())
                .collect::<Vec<_>>(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
        };

        let previous = self.examples.insert(key.clone(), Box::new(setup));
        assert!(
            previous.is_none(),
            "duplicate example declaration: {:?} {:?}",
            key.command_path,
            key.args
        );
        self
    }

    pub fn run(self) {
        let binary_path = resolve_binary_path(&self.binary_name);
        run::run_help_test(self, &binary_path);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Fixture {
    pub(crate) stdin: Option<Vec<u8>>,
    pub(crate) files: Vec<FileFixture>,
    pub(crate) dirs: Vec<PathBuf>,
    pub(crate) env: BTreeMap<String, String>,
    pub(crate) commands: Vec<CommandFixture>,
}

impl Fixture {
    pub fn stdin(&mut self, content: impl Into<Vec<u8>>) {
        self.stdin = Some(content.into());
    }

    pub fn file(&mut self, path: impl AsRef<Path>, content: impl AsRef<[u8]>) {
        self.files.push(FileFixture {
            path: path.as_ref().to_path_buf(),
            content: content.as_ref().to_vec(),
        });
    }

    pub fn dir(&mut self, path: impl AsRef<Path>) {
        self.dirs.push(path.as_ref().to_path_buf());
    }

    pub fn env(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.env.insert(key.into(), value.into());
    }

    pub fn command(&mut self, program: impl Into<String>, args: &[&str]) {
        self.commands.push(CommandFixture {
            program: program.into(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        });
    }

    fn extend(&mut self, other: Fixture) {
        if let Some(stdin) = other.stdin {
            self.stdin = Some(stdin);
        }
        self.files.extend(other.files);
        self.dirs.extend(other.dirs);
        self.env.extend(other.env);
        self.commands.extend(other.commands);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FileFixture {
    pub(crate) path: PathBuf,
    pub(crate) content: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct CommandFixture {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

pub(crate) fn fixture_for_page(help_test: &HelpTest, command_path: &[String]) -> Option<Fixture> {
    help_test.pages.get(command_path).map(|setup| {
        let mut fixture = Fixture::default();
        setup(&mut fixture);
        fixture
    })
}

pub(crate) fn fixture_for_example(
    help_test: &HelpTest,
    command_path: &[String],
    args: &[String],
) -> Option<Fixture> {
    let mut fixture = fixture_for_page(help_test, command_path)?;

    if let Some(setup) = help_test.examples.get(&ExampleKey {
        command_path: command_path.to_vec(),
        args: args.to_vec(),
    }) {
        let mut example_fixture = Fixture::default();
        setup(&mut example_fixture);
        fixture.extend(example_fixture);
    }

    Some(fixture)
}

#[cfg(test)]
mod tests {
    use super::{fixture_for_example, fixture_for_page, HelpTest};

    #[test]
    fn example_fixture_extends_page_fixture() {
        let help_test = HelpTest::new("demo")
            .page(&["run"], |fixture| {
                fixture.env("PAGE_ONLY", "1");
                fixture.env("OVERRIDE_ME", "page");
                fixture.command("setup-page", &["alpha"]);
            })
            .example(&["run"], &["--flag"], |fixture| {
                fixture.env("OVERRIDE_ME", "example");
                fixture.command("setup-example", &["beta"]);
            });

        let fixture = fixture_for_example(&help_test, &["run".to_owned()], &["--flag".to_owned()])
            .expect("fixture should exist");

        assert_eq!(fixture.env.get("PAGE_ONLY").map(String::as_str), Some("1"));
        assert_eq!(
            fixture.env.get("OVERRIDE_ME").map(String::as_str),
            Some("example")
        );
        assert_eq!(fixture.commands.len(), 2);
        assert_eq!(fixture.commands[0].program, "setup-page");
        assert_eq!(fixture.commands[1].program, "setup-example");
    }

    #[test]
    fn page_fixture_is_still_available_without_example_override() {
        let help_test = HelpTest::new("demo").page(&[], |fixture| {
            fixture.env("HOME", "/tmp/demo");
        });

        let page_fixture = fixture_for_page(&help_test, &[]).expect("page fixture should exist");
        let example_fixture = fixture_for_example(&help_test, &[], &[])
            .expect("example lookup should reuse page fixture");

        assert_eq!(page_fixture.env, example_fixture.env);
    }
}

fn resolve_binary_path(binary_name: &str) -> PathBuf {
    let env_key = format!("CARGO_BIN_EXE_{binary_name}");
    if let Some(path) = std::env::var_os(&env_key) {
        return PathBuf::from(path);
    }

    let manifest_dir = std::env::current_dir().unwrap_or_else(|error| {
        panic!("failed to read current directory while locating {binary_name}: {error}")
    });

    let status = std::process::Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--bin")
        .arg(binary_name)
        .current_dir(&manifest_dir)
        .status()
        .unwrap_or_else(|error| panic!("failed to run cargo build for {binary_name}: {error}"));

    assert!(
        status.success(),
        "cargo build --bin {binary_name} failed with status {status}"
    );

    let mut search_dir = Some(manifest_dir.as_path());
    while let Some(dir) = search_dir {
        let candidate = dir.join("target").join("debug").join(binary_name);
        if candidate.is_file() {
            return candidate;
        }

        #[cfg(windows)]
        {
            let candidate = dir
                .join("target")
                .join("debug")
                .join(format!("{binary_name}.exe"));
            if candidate.is_file() {
                return candidate;
            }
        }

        search_dir = dir.parent();
    }

    panic!(
        "built {binary_name}, but could not find target/debug binary from {}",
        manifest_dir.display()
    );
}

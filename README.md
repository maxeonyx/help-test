# help-test

`help-test` exercises the examples and command pages exposed by a CLI's real `--help` interface. Fixtures can supply stdin, files, directories, environment variables, and fake commands without coupling tests to product internals.

The crate is intentionally consumed from a pinned Git release:

```toml
[dev-dependencies]
help-test = { git = "https://github.com/maxeonyx/help-test", tag = "v0.1.0" }
```

Development works from an ordinary standalone clone. Run `cargo ratchet`, `cargo fmt --check`, and `cargo clippy -- -D warnings` before proposing changes.

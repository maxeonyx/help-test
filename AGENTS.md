# help-test — Agent Instructions

This repository is self-contained for development. A standalone clone must
build, test, and release without an `agent-tools` checkout.

## TDD ratchet — read before testing

Run `cargo ratchet`, not plain `cargo test`. A new test must be red when first
introduced and committed as `pending`; that expected red test keeps CI green.
A new test must not pass when first introduced. Implement only after the red
commit, then rerun the ratchet and commit the promotion to `passing`.

## Integration workflow

Run `devenv test` before committing and pushing; it includes `actionlint`, so
workflow syntax is checked offline. Source CI does not run on push. Open a pull
request, merge current `main` into the feature branch, then explicitly dispatch:

```bash
gh workflow run ci.yml --ref <feature-branch> -f pr_number=<number>
```

The repository-serialized run records the required `Ready` check, builds the
release artifacts, auto-merges the pull request, publishes those same artifacts,
and records `integrated-ci` on the exact merge commit.

## Purpose

`help-test` drives a compiled CLI through its public help surface. Keep the API
small and product-agnostic. Fixtures may arrange external inputs, but assertions
must come from invoking the real binary and observing its output and effects.

## Checks

```bash
cargo ratchet
cargo fmt --check
cargo clippy -- -D warnings
```

Prefer merge commits. Do not rebase or force-push by default. History
replacement is exceptional and requires an explicit decision.

{ pkgs, ... }:

{
  packages = [
    pkgs.actionlint
    pkgs.cargo
    pkgs.cargo-nextest
    pkgs.clippy
    pkgs.git
    pkgs.rustc
    pkgs.rustfmt
  ];

  enterTest = ''
    actionlint
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo ratchet
  '';
}

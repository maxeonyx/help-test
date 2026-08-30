#[test]
fn tdd_ratchet_gatekeeper() {
    assert!(
        std::env::var_os("TDD_RATCHET").is_some(),
        "Run tdd-ratchet instead of cargo test."
    );
}

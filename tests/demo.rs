use amex_together::{compat, model::Route};

#[test]
fn published_demo_matches_its_documented_balances() {
    let ledger = compat::decode(
        include_bytes!("../examples/demo-ledger.json"),
        std::path::Path::new("unused-local-profile.json"),
    )
    .unwrap();
    let account = ledger.account_summary();
    assert_eq!(account.summary.remaining, [24488, 36712]);
    assert_eq!(account.summary.contributions, [40000, 15000]);
    assert_eq!(account.summary.bank_remaining, 61200);
    assert!(account.summary.complete());
    assert!(ledger.statements.iter().all(|s| s.source.is_none()));
    assert!(ledger.transactions.iter().all(|t| t.receipt.is_none()));
    assert_eq!(
        ledger
            .payments
            .iter()
            .filter(|p| p.route == Route::Amex)
            .count(),
        3
    );
}

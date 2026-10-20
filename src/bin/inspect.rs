fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("Pass a PDF or CSV path");
    let p = amex_together::import::read(std::path::Path::new(&path))?;
    println!(
        "{} to {}: {} records; purchases {}, refunds {}, payments {}; closing {}",
        p.start,
        p.end,
        p.records.len(),
        amex_together::model::money(p.debits),
        amex_together::model::money(p.refunds()),
        amex_together::model::money(p.payments()),
        amex_together::model::money(p.closing)
    );
    println!("Warnings: {:?}\nErrors: {:?}", p.warnings, p.errors);
    if !p.errors.is_empty() {
        anyhow::bail!("Statement did not reconcile");
    }
    Ok(())
}

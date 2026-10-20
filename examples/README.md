# Demo statement

`demo-statement.csv` contains fictional purchases, one refund and one repayment. Alex and Sam are example card members. None of the rows come from a personal statement.

Import the CSV to try the preview and allocation controls. Check the date range and use an opening balance of £0.00 before confirming the import. The sample contains £1,214.50 in purchases, a £52.50 refund and a £300.00 repayment.

For the ready-made example shown in the main README, run the app with `--demo`. That workspace already has spending assigned to Alex, Sam or Joint.

`demo-ledger.json` contains the complete example shown in the screenshots. It includes the allocated transactions, the statement repayment and two later partial payments: £100.00 from Alex and £150.00 from Sam. It contains no attachments or local file paths.

Open it with `cargo run --release --bin amex-together -- --demo --restore examples/demo-ledger.json`. The app restores it into a new demo folder and keeps your existing ledger.

use crate::{import::Preview, model::*};
use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub struct Store {
    connection: Connection,
    pub root: PathBuf,
    pub demo: bool,
}

impl Store {
    pub fn open(root: PathBuf, demo: bool) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        let connection = Connection::open(root.join(if demo {
            "demo.sqlite3"
        } else {
            "ledger.sqlite3"
        }))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS workspace (id INTEGER PRIMARY KEY CHECK(id=1), content TEXT NOT NULL);")?;
        Ok(Self {
            connection,
            root,
            demo,
        })
    }
    pub fn load(&self) -> Result<Ledger> {
        let mut stmt = self
            .connection
            .prepare("SELECT content FROM workspace WHERE id=1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let data: String = row.get(0)?;
            let ledger =
                crate::compat::decode(data.as_bytes(), &self.root.join("legacy-profile.json"))
                    .context("Your saved ledger could not be read. It has not been overwritten.")?;
            if serde_json::from_str::<serde_json::Value>(&data)?["version"] == 1 {
                let folder = self.root.join(if self.demo {
                    "demo-recovery"
                } else {
                    "recovery"
                });
                std::fs::create_dir_all(&folder)?;
                std::fs::write(
                    folder.join(format!(
                        "before-format-upgrade-{}.json",
                        chrono::Local::now().format("%Y%m%d-%H%M%S-%f")
                    )),
                    data,
                )?;
            }
            Ok(ledger)
        } else {
            Ok(Ledger::default())
        }
    }
    pub fn save(&self, ledger: &Ledger) -> Result<()> {
        let data = serde_json::to_string(ledger)?;
        self.connection.execute("INSERT INTO workspace(id,content) VALUES (1,?1) ON CONFLICT(id) DO UPDATE SET content=excluded.content", [data])?;
        Ok(())
    }
    pub fn save_recovery(&self, ledger: &Ledger) -> Result<PathBuf> {
        let folder = self.root.join(if self.demo {
            "demo-recovery"
        } else {
            "recovery"
        });
        std::fs::create_dir_all(&folder)?;
        let path = folder.join(format!(
            "ledger-{}.json",
            chrono::Local::now().format("%Y%m%d-%H%M%S-%f")
        ));
        std::fs::write(&path, serde_json::to_vec_pretty(ledger)?)?;
        Ok(path)
    }
    pub fn copy_attachment(&self, source: &Path, category: &str, key: &str) -> Result<String> {
        let folder = self
            .root
            .join(if self.demo { "demo-files" } else { "files" })
            .join(category);
        std::fs::create_dir_all(&folder)?;
        let ext = source.extension().and_then(|s| s.to_str()).unwrap_or("bin");
        let dest = folder.join(format!("{key}.{ext}"));
        if source != dest {
            std::fs::copy(source, &dest)
                .context("Could not save a local copy of the attachment")?;
        }
        Ok(dest.to_string_lossy().into_owned())
    }
    pub fn import(&self, ledger: &mut Ledger, p: &Preview) -> Result<u64> {
        if !p.errors.is_empty() {
            bail!("Import is blocked because the statement totals do not reconcile");
        }
        if let Some(s) = ledger.statements.iter().find(|s| s.digest == p.digest) {
            return Ok(s.id);
        }
        if ledger
            .removed_statements
            .iter()
            .any(|s| s.statement.digest == p.digest)
        {
            bail!(
                "This statement was removed from the workspace. Restore it under Manage statements to keep its saved allocations."
            );
        }
        if ledger
            .statements
            .iter()
            .any(|s| s.start == p.start && s.end == p.end)
        {
            bail!(
                "This statement period already exists. Re-import the original file to open it; importing a different export for the same period could double-count transactions."
            );
        }
        if ledger
            .statements
            .iter()
            .any(|s| s.start <= p.end && p.start <= s.end)
        {
            bail!(
                "This date range overlaps an imported statement. Use separate, non-overlapping statement periods so spending is only counted once."
            );
        }
        let mut next = ledger.clone();
        let id = next.id();
        let source = self.copy_attachment(&p.path, "statements", &p.digest)?;
        next.statements.push(Statement {
            id,
            label: format!("{} to {}", p.start, p.end),
            start: p.start.clone(),
            end: p.end.clone(),
            due: p.due.clone(),
            previous: p.previous,
            new_debits: p.debits,
            new_credits: p.credits,
            closing: p.closing,
            source: Some(source),
            digest: p.digest.clone(),
            warnings: p.warnings.clone(),
        });
        if p.previous != 0 {
            let tid = next.id();
            next.transactions.push(Transaction { id: tid, statement_id: id, fingerprint: format!("opening-{id}"), date: p.start.clone(), processed: p.start.clone(), merchant: "Opening balance brought forward".into(), amount: p.previous, cardholder: "Opening balance".into(), kind: Kind::Opening, split: Split::Unassigned, custom_person_two: 0, note: "Assign the balance brought forward from the previous statement. Use a custom split for an exact share.".into(), receipt: None, expense: false });
        }
        for r in &p.records {
            let rid = next.id();
            if r.payment {
                next.payments.push(Payment {
                    id: rid,
                    statement_id: id,
                    fingerprint: Some(r.fingerprint.clone()),
                    date: r.date.clone(),
                    amount: -r.amount,
                    split: Split::Unassigned,
                    custom_person_two: 0,
                    route: Route::Amex,
                    note: "Repayment found on your statement. Attribute the contribution below."
                        .into(),
                    imported: true,
                });
            } else {
                next.transactions.push(Transaction {
                    id: rid,
                    statement_id: id,
                    fingerprint: r.fingerprint.clone(),
                    date: r.date.clone(),
                    processed: r.processed.clone(),
                    merchant: r.merchant.clone(),
                    amount: r.amount,
                    cardholder: r.cardholder.clone(),
                    kind: Preview::kind(r),
                    split: Split::Unassigned,
                    custom_person_two: 0,
                    note: String::new(),
                    receipt: None,
                    expense: false,
                });
            }
        }
        self.save(&next)?;
        *ledger = next;
        Ok(id)
    }
}

pub fn seed_demo(ledger: &mut Ledger) {
    ledger.people = [
        Person {
            name: "Alex".into(),
            cardholder_names: vec!["ALEX SAMPLE".into()],
        },
        Person {
            name: "Sam".into(),
            cardholder_names: vec!["SAM SAMPLE".into()],
        },
    ];
    let id = ledger.id();
    ledger.statements.push(Statement {
        id,
        label: "Demo · September 2026".into(),
        start: "2026-09-06".into(),
        end: "2026-10-05".into(),
        due: Some("2026-10-30".into()),
        previous: 0,
        new_debits: 121450,
        new_credits: 35250,
        closing: 86200,
        source: None,
        digest: "demo".into(),
        warnings: vec![],
    });
    for (on, name, amount, owner, split) in [
        (
            "2026-09-08",
            "Waitrose · weekly shop",
            8540,
            "SAM SAMPLE",
            Split::Joint,
        ),
        (
            "2026-09-10",
            "The Italian Kitchen",
            7260,
            "ALEX SAMPLE",
            Split::Joint,
        ),
        (
            "2026-09-12",
            "Trainline",
            4250,
            "ALEX SAMPLE",
            Split::PersonOne,
        ),
        ("2026-09-14", "Boots", 2875, "SAM SAMPLE", Split::PersonTwo),
        (
            "2026-09-16",
            "British Airways",
            64000,
            "ALEX SAMPLE",
            Split::Joint,
        ),
        (
            "2026-09-20",
            "John Lewis · return",
            -5250,
            "SAM SAMPLE",
            Split::Joint,
        ),
        (
            "2026-09-21",
            "Apple",
            14900,
            "ALEX SAMPLE",
            Split::PersonOne,
        ),
        (
            "2026-09-24",
            "Sainsbury's",
            6835,
            "SAM SAMPLE",
            Split::Joint,
        ),
        ("2026-09-26", "Dishoom", 9290, "ALEX SAMPLE", Split::Joint),
        (
            "2026-09-28",
            "Coffee & Co",
            3500,
            "SAM SAMPLE",
            Split::PersonTwo,
        ),
    ] {
        let tid = ledger.id();
        ledger.transactions.push(Transaction {
            id: tid,
            statement_id: id,
            fingerprint: format!("demo-{tid}"),
            date: on.into(),
            processed: on.into(),
            merchant: name.into(),
            amount,
            cardholder: owner.into(),
            kind: if amount < 0 {
                Kind::Refund
            } else {
                Kind::Purchase
            },
            split,
            custom_person_two: 0,
            note: String::new(),
            receipt: None,
            expense: name == "Trainline",
        });
    }
    let pid = ledger.id();
    ledger.payments.push(Payment {
        id: pid,
        statement_id: id,
        fingerprint: None,
        date: "2026-09-25".into(),
        amount: 30000,
        split: Split::PersonOne,
        custom_person_two: 0,
        route: Route::Amex,
        note: "Statement payment".into(),
        imported: true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_round_trip_is_atomic_and_durable() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path().to_owned(), false).unwrap();
        let mut ledger = Ledger::default();
        seed_demo(&mut ledger);
        store.save(&ledger).unwrap();
        drop(store);
        let store = Store::open(tmp.path().to_owned(), false).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.transactions.len(), 10);
        assert_eq!(loaded.summary(1).bank_paid, 30000);
    }
    #[test]
    fn equal_payments_remain_distinct_after_reopening() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path().to_owned(), false).unwrap();
        let mut ledger = Ledger::default();
        seed_demo(&mut ledger);
        for split in [Split::PersonOne, Split::PersonTwo] {
            let id = ledger.id();
            ledger.payments.push(Payment {
                id,
                statement_id: 1,
                fingerprint: None,
                date: "2026-10-19".into(),
                amount: 2500,
                split,
                custom_person_two: 0,
                route: Route::Amex,
                note: "A separate payment".into(),
                imported: false,
            });
        }
        store.save(&ledger).unwrap();
        drop(store);
        let store = Store::open(tmp.path().to_owned(), false).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.payments.len(), 3);
        assert_ne!(loaded.payments[1].id, loaded.payments[2].id);
        assert_eq!(loaded.summary(1).contributions, [32500, 2500]);
        assert_eq!(loaded.account_summary().summary.bank_remaining, 81200);
    }
    #[test]
    fn demo_and_real_ledgers_are_separate() {
        let tmp = tempfile::tempdir().unwrap();
        let demo = Store::open(tmp.path().to_owned(), true).unwrap();
        let mut l = Ledger::default();
        seed_demo(&mut l);
        demo.save(&l).unwrap();
        assert!(
            Store::open(tmp.path().to_owned(), false)
                .unwrap()
                .load()
                .unwrap()
                .statements
                .is_empty()
        );
    }
    #[test]
    fn older_ledgers_load_and_recovery_keeps_removed_statements() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path().to_owned(), false).unwrap();
        let mut ledger = Ledger::default();
        seed_demo(&mut ledger);
        let mut old = serde_json::to_value(&ledger).unwrap();
        old.as_object_mut().unwrap().remove("removed_statements");
        let mut loaded: Ledger = serde_json::from_value(old).unwrap();
        assert!(loaded.removed_statements.is_empty());
        let recovery = store.save_recovery(&loaded).unwrap();
        loaded.remove_statement(1).unwrap();
        store.save(&loaded).unwrap();
        let saved = store.load().unwrap();
        assert!(saved.statements.is_empty());
        assert_eq!(saved.removed_statements[0].transactions.len(), 10);
        let original: Ledger = serde_json::from_slice(&std::fs::read(recovery).unwrap()).unwrap();
        assert_eq!(original.statements.len(), 1);
        assert_eq!(original.payments.len(), 1);
        let mut restored = saved;
        restored.restore_statement(1).unwrap();
        assert_eq!(restored.summary(1).remaining, original.summary(1).remaining);
    }
    #[test]
    fn repeated_import_preserves_classifications_and_never_drops_repeated_purchases() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("statement.csv");
        std::fs::write(&input, "Date,Description,Amount,Card Member\n01/10/2026,CAFE,22.00,CARD MEMBER ONE\n01/10/2026,CAFE,22.00,CARD MEMBER ONE\n02/10/2026,PAYMENT RECEIVED,-10.00,\n03/10/2026,REFUND,-5.00,CARD MEMBER ONE\n").unwrap();
        let p = crate::import::read(&input).unwrap();
        let store = Store::open(tmp.path().join("data"), false).unwrap();
        let mut l = Ledger::default();
        let id = store.import(&mut l, &p).unwrap();
        l.transactions[0].split = Split::Joint;
        l.payments[0].split = Split::PersonTwo;
        store.save(&l).unwrap();
        assert_eq!(store.import(&mut l, &p).unwrap(), id);
        assert_eq!(l.transactions.len(), 3);
        assert_eq!(l.payments.len(), 1);
        assert_eq!(l.transactions[0].split, Split::Joint);
        assert_eq!(l.payments[0].split, Split::PersonTwo);
        assert_ne!(l.transactions[0].fingerprint, l.transactions[1].fingerprint);
        l.remove_statement(id).unwrap();
        store.save(&l).unwrap();
        assert!(
            store
                .import(&mut l, &p)
                .unwrap_err()
                .to_string()
                .contains("Restore")
        );
        l.restore_statement(id).unwrap();
        assert_eq!(l.transactions[0].split, Split::Joint);
        assert_eq!(l.payments[0].split, Split::PersonTwo);
    }
    #[test]
    fn overlapping_period_is_rejected_without_changing_saved_data() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("statement.csv");
        std::fs::write(
            &input,
            "Date,Description,Amount\n01/10/2026,CAFE,22.00\n05/10/2026,SHOP,10.00\n",
        )
        .unwrap();
        let preview = crate::import::read(&input).unwrap();
        let store = Store::open(tmp.path().join("data"), false).unwrap();
        let mut ledger = Ledger::default();
        store.import(&mut ledger, &preview).unwrap();
        let before = serde_json::to_string(&ledger).unwrap();
        let mut overlapping = preview.clone();
        overlapping.digest = "different-file".into();
        overlapping.start = "2026-10-03".into();
        overlapping.end = "2026-10-10".into();
        assert!(
            store
                .import(&mut ledger, &overlapping)
                .unwrap_err()
                .to_string()
                .contains("overlaps")
        );
        assert_eq!(serde_json::to_string(&ledger).unwrap(), before);
        assert_eq!(
            serde_json::to_string(&store.load().unwrap()).unwrap(),
            before
        );
    }
}

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub type Pence = i64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    #[serde(default)]
    pub cardholder_names: Vec<String>,
}

pub fn default_people() -> [Person; 2] {
    std::array::from_fn(|i| Person {
        name: format!("Person {}", i + 1),
        cardholder_names: vec![],
    })
}

pub fn validate_people(people: &[Person; 2]) -> Result<()> {
    for person in people {
        if person.name.trim().is_empty() || person.name.chars().count() > 32 {
            bail!("Enter a name of 1 to 32 characters for each person");
        }
    }
    if people[0].name.trim().to_lowercase() == people[1].name.trim().to_lowercase() {
        bail!("Use different display names so you can tell the two people apart");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Split {
    #[default]
    Unassigned,
    PersonOne,
    PersonTwo,
    Joint,
    Custom,
}

impl Split {
    pub const ALL: [Self; 5] = [
        Self::Unassigned,
        Self::PersonOne,
        Self::PersonTwo,
        Self::Joint,
        Self::Custom,
    ];
    pub fn label(self, people: &[Person; 2]) -> &str {
        match self {
            Self::Unassigned => "Unassigned",
            Self::PersonOne => &people[0].name,
            Self::PersonTwo => &people[1].name,
            Self::Joint => "Joint · 50/50",
            Self::Custom => "Custom split",
        }
    }
    /// The odd penny goes to the first person, consistently, for both charges and refunds.
    pub fn shares(self, amount: Pence, custom_person_two: Pence) -> Option<[Pence; 2]> {
        match self {
            Self::Unassigned => None,
            Self::PersonOne => Some([amount, 0]),
            Self::PersonTwo => Some([0, amount]),
            Self::Joint => Some([amount - amount / 2, amount / 2]),
            Self::Custom => Some([amount - custom_person_two, custom_person_two]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Purchase,
    Refund,
    Opening,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: u64,
    pub statement_id: u64,
    pub fingerprint: String,
    pub date: String,
    pub processed: String,
    pub merchant: String,
    pub amount: Pence,
    pub cardholder: String,
    pub kind: Kind,
    pub split: Split,
    pub custom_person_two: Pence,
    pub note: String,
    pub receipt: Option<String>,
    pub expense: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    Amex,
    PersonTwoToPersonOne,
    PersonOneToPersonTwo,
}

impl Route {
    pub const ALL: [Self; 3] = [
        Self::Amex,
        Self::PersonTwoToPersonOne,
        Self::PersonOneToPersonTwo,
    ];
    pub fn label(self, people: &[Person; 2]) -> String {
        match self {
            Self::Amex => "Payment to Amex".into(),
            Self::PersonTwoToPersonOne => format!("{} → {}", people[1].name, people[0].name),
            Self::PersonOneToPersonTwo => format!("{} → {}", people[0].name, people[1].name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub id: u64,
    pub statement_id: u64,
    pub fingerprint: Option<String>,
    pub date: String,
    pub amount: Pence,
    pub split: Split,
    pub custom_person_two: Pence,
    pub route: Route,
    pub note: String,
    pub imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statement {
    pub id: u64,
    pub label: String,
    pub start: String,
    pub end: String,
    pub due: Option<String>,
    pub previous: Pence,
    pub new_debits: Pence,
    pub new_credits: Pence,
    pub closing: Pence,
    pub source: Option<String>,
    pub digest: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedStatement {
    pub statement: Statement,
    pub transactions: Vec<Transaction>,
    pub payments: Vec<Payment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    pub version: u32,
    #[serde(default = "default_people")]
    pub people: [Person; 2],
    pub next_id: u64,
    pub statements: Vec<Statement>,
    pub transactions: Vec<Transaction>,
    pub payments: Vec<Payment>,
    #[serde(default)]
    pub removed_statements: Vec<RemovedStatement>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            version: 2,
            people: default_people(),
            next_id: 1,
            statements: vec![],
            transactions: vec![],
            payments: vec![],
            removed_statements: vec![],
        }
    }
}

impl Ledger {
    pub fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn statement(&self, id: u64) -> Option<&Statement> {
        self.statements.iter().find(|s| s.id == id)
    }
    pub fn latest_statement(&self) -> Option<&Statement> {
        self.statements.iter().max_by_key(|s| (&s.end, &s.start))
    }
    pub fn remove_statement(&mut self, id: u64) -> Result<()> {
        let index = self
            .statements
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| anyhow::anyhow!("This statement is no longer in the workspace"))?;
        let transactions = self
            .transactions
            .iter()
            .filter(|t| t.statement_id == id)
            .cloned()
            .collect();
        let payments = self
            .payments
            .iter()
            .filter(|p| p.statement_id == id)
            .cloned()
            .collect();
        self.transactions.retain(|t| t.statement_id != id);
        self.payments.retain(|p| p.statement_id != id);
        self.removed_statements.push(RemovedStatement {
            statement: self.statements.remove(index),
            transactions,
            payments,
        });
        Ok(())
    }
    pub fn restore_statement(&mut self, id: u64) -> Result<()> {
        let index = self
            .removed_statements
            .iter()
            .position(|s| s.statement.id == id)
            .ok_or_else(|| anyhow::anyhow!("This statement is no longer in removed statements"))?;
        let restoring = &self.removed_statements[index].statement;
        if self
            .statements
            .iter()
            .any(|s| s.id == id || (s.start <= restoring.end && restoring.start <= s.end))
        {
            bail!(
                "This statement overlaps an active statement. Remove that copy before restoring the original."
            );
        }
        let removed = self.removed_statements.remove(index);
        self.statements.push(removed.statement);
        self.transactions.extend(removed.transactions);
        self.payments.extend(removed.payments);
        Ok(())
    }
    pub fn account_transactions(&self) -> impl Iterator<Item = &Transaction> {
        let first = self.statements.iter().min_by_key(|s| (&s.start, &s.end));
        self.transactions.iter().filter(move |t| {
            t.kind != Kind::Opening || first.is_some_and(|s| t.statement_id == s.id)
        })
    }
    /// A continuous account history: keep the first opening balance, then only
    /// new activity. Subsequent openings repeat debt already in that history.
    pub fn account_summary(&self) -> AccountSummary {
        let mut account = AccountSummary::default();
        for t in self.account_transactions() {
            account.summary.add_transaction(t);
        }
        let mut statements: Vec<_> = self.statements.iter().collect();
        statements.sort_by_key(|s| (&s.start, &s.end));
        for pair in statements.windows(2) {
            let [previous, next] = pair else {
                unreachable!()
            };
            let adjacent = chrono::NaiveDate::parse_from_str(&previous.end, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.succ_opt())
                .is_some_and(|date| date.to_string() == next.start);
            let difference = next.previous - previous.closing;
            if next.start <= previous.end {
                account.warnings.push("Some imported statement periods overlap. Review them before relying on the combined balance.".into());
            } else if !adjacent || difference != 0 {
                // Missing activity must not be silently assigned using the next
                // statement's full opening split. Preserve it as unallocated.
                account.summary.total += difference;
                account.summary.unassigned_spend += difference;
                account.summary.unassigned_count += 1;
                account.balance_adjustment += difference;
                account.warnings.push(if !adjacent {
                    format!("Statement history has a gap between {} and {}. Import the missing statement to allocate the carried balance correctly.", previous.end, next.start)
                } else {
                    format!("The opening balance on {} differs from the previous closing balance by {}. Check the statements before finalising your shares.", next.start, money(difference))
                });
            }
        }

        let mut imported: Vec<_> = self
            .payments
            .iter()
            .filter(|p| p.imported)
            .cloned()
            .collect();
        let mut matched = vec![false; imported.len()];
        for manual in self.payments.iter().filter(|p| !p.imported) {
            let match_index = (manual.route == Route::Amex)
                .then(|| {
                    imported.iter().enumerate().position(|(i, p)| {
                        !matched[i]
                            && p.route == Route::Amex
                            // Entries deliberately recorded against the same
                            // statement are distinct, even on an identical date.
                            && p.statement_id != manual.statement_id
                            && p.date == manual.date
                            && p.amount == manual.amount
                    })
                })
                .flatten();
            if let Some(i) = match_index {
                matched[i] = true;
                account.matched_payments += 1;
                let payment = &mut imported[i];
                if payment.split == Split::Unassigned {
                    payment.split = manual.split;
                    payment.custom_person_two = manual.custom_person_two;
                } else if manual.split != Split::Unassigned
                    && payment
                        .split
                        .shares(payment.amount, payment.custom_person_two)
                        != manual.split.shares(manual.amount, manual.custom_person_two)
                {
                    account.warnings.push(format!("The recorded payment of {} on {} has a different attribution from its statement entry. The combined balance uses the statement attribution; check both records.", money(manual.amount), manual.date));
                }
            } else {
                account.summary.add_payment(manual);
                if manual.route == Route::Amex {
                    account.extra_bank_paid += manual.amount;
                }
            }
        }
        for payment in &imported {
            account.summary.add_payment(payment);
        }
        account.summary.review_required = !account.warnings.is_empty();
        account.summary.finish();
        account
    }
    pub fn summary(&self, statement_id: u64) -> Summary {
        let mut s = Summary::default();
        for t in self
            .transactions
            .iter()
            .filter(|t| t.statement_id == statement_id)
        {
            s.add_transaction(t);
        }
        for p in self
            .payments
            .iter()
            .filter(|p| p.statement_id == statement_id)
        {
            s.add_payment(p);
        }
        s.finish();
        s
    }
}

#[derive(Debug, Default)]
pub struct AccountSummary {
    pub summary: Summary,
    pub warnings: Vec<String>,
    pub balance_adjustment: Pence,
    pub extra_bank_paid: Pence,
    pub matched_payments: usize,
}

#[derive(Debug, Default, Clone)]
pub struct Summary {
    pub shares: [Pence; 2],
    pub contributions: [Pence; 2],
    pub remaining: [Pence; 2],
    pub total: Pence,
    pub purchases: Pence,
    pub refunds: Pence,
    pub refund_count: usize,
    pub opening: Pence,
    pub bank_paid: Pence,
    pub bank_remaining: Pence,
    pub unassigned_spend: Pence,
    pub unassigned_count: usize,
    pub unassigned_payments: Pence,
    pub unassigned_payment_count: usize,
    pub review_required: bool,
}

impl Summary {
    fn add_transaction(&mut self, t: &Transaction) {
        self.total += t.amount;
        match t.kind {
            Kind::Purchase => self.purchases += t.amount,
            Kind::Refund => {
                self.refunds -= t.amount;
                self.refund_count += 1;
            }
            Kind::Opening => self.opening += t.amount,
        }
        if let Some(shares) = t.split.shares(t.amount, t.custom_person_two) {
            for (i, share) in shares.iter().enumerate() {
                self.shares[i] += share;
            }
        } else {
            self.unassigned_spend += t.amount;
            self.unassigned_count += 1;
        }
    }
    fn add_payment(&mut self, p: &Payment) {
        match p.route {
            Route::Amex => {
                self.bank_paid += p.amount;
                if let Some(shares) = p.split.shares(p.amount, p.custom_person_two) {
                    for (i, share) in shares.iter().enumerate() {
                        self.contributions[i] += share;
                    }
                } else {
                    self.unassigned_payments += p.amount;
                    self.unassigned_payment_count += 1;
                }
            }
            Route::PersonTwoToPersonOne => {
                self.contributions[0] -= p.amount;
                self.contributions[1] += p.amount;
            }
            Route::PersonOneToPersonTwo => {
                self.contributions[0] += p.amount;
                self.contributions[1] -= p.amount;
            }
        }
    }
    fn finish(&mut self) {
        self.remaining = [
            self.shares[0] - self.contributions[0],
            self.shares[1] - self.contributions[1],
        ];
        self.bank_remaining = self.total - self.bank_paid;
    }
    pub fn complete(&self) -> bool {
        self.unassigned_count == 0 && self.unassigned_payment_count == 0 && !self.review_required
    }
}

pub fn money(amount: Pence) -> String {
    let abs = amount.unsigned_abs();
    let digits = (abs / 100).to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!(
        "{}£{}.{:02}",
        if amount < 0 { "−" } else { "" },
        grouped,
        abs % 100
    )
}

pub fn parse_money(input: &str) -> Result<Pence> {
    let upper = input.trim().to_uppercase().replace('\u{a0}', " ");
    let mut s = upper.trim();
    let credit = s.ends_with("CR");
    if credit {
        s = s.strip_suffix("CR").unwrap().trim();
    }
    let parentheses = s.starts_with('(') && s.ends_with(')');
    if parentheses {
        s = s
            .strip_prefix('(')
            .unwrap()
            .strip_suffix(')')
            .unwrap()
            .trim();
    }
    s = s
        .strip_prefix("GBP")
        .or_else(|| s.strip_prefix('£'))
        .unwrap_or(s)
        .trim();
    let negative = credit || parentheses || s.starts_with(['-', '−']);
    s = s.strip_prefix(['-', '−', '+']).unwrap_or(s).trim();
    s = s.strip_prefix('£').unwrap_or(s).trim();
    if s.chars()
        .any(|c| !c.is_ascii_digit() && c != ',' && c != '.')
    {
        bail!("Enter an amount such as 42.50");
    }
    let whole_part = s.split('.').next().unwrap_or("");
    if whole_part.contains(',') {
        let groups: Vec<_> = whole_part.split(',').collect();
        if groups[0].is_empty()
            || groups[0].len() > 3
            || groups.iter().skip(1).any(|s| s.len() != 3)
        {
            bail!("Check the thousands separators in this amount");
        }
    }
    let s = s.replace(',', "");
    let parts: Vec<_> = s.split('.').collect();
    if parts.is_empty()
        || parts.len() > 2
        || parts[0].is_empty()
        || !parts[0].chars().all(|c| c.is_ascii_digit())
    {
        bail!("Enter an amount such as 42.50");
    }
    let fraction = parts.get(1).copied().unwrap_or("");
    if fraction.len() > 2 || !fraction.chars().all(|c| c.is_ascii_digit()) {
        bail!("Use no more than two decimal places");
    }
    let whole: i64 = parts[0].parse()?;
    if whole > 1_000_000_000 {
        bail!("Amount is too large");
    }
    let fraction: i64 = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>()? * 10,
        _ => fraction.parse()?,
    };
    Ok((whole * 100 + fraction) * if negative { -1 } else { 1 })
}

pub fn validate_custom(amount: Pence, second_share: Pence) -> Result<()> {
    if (amount >= 0 && !(0..=amount).contains(&second_share))
        || (amount < 0 && !(amount..=0).contains(&second_share))
    {
        bail!(
            "The custom share must be between zero and the transaction amount (use a negative share for a refund)"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transaction(amount: i64, split: Split) -> Transaction {
        Transaction {
            id: 1,
            statement_id: 1,
            fingerprint: "a".into(),
            date: "2026-10-01".into(),
            processed: "2026-10-01".into(),
            merchant: "Test".into(),
            amount,
            cardholder: "Person 1".into(),
            kind: if amount < 0 {
                Kind::Refund
            } else {
                Kind::Purchase
            },
            split,
            custom_person_two: 0,
            note: String::new(),
            receipt: None,
            expense: false,
        }
    }
    fn payment(amount: i64, split: Split, route: Route) -> Payment {
        Payment {
            id: 1,
            statement_id: 1,
            fingerprint: None,
            date: "2026-10-02".into(),
            amount,
            split,
            custom_person_two: 0,
            route,
            note: String::new(),
            imported: false,
        }
    }
    fn statement(id: u64, start: &str, end: &str, previous: Pence, closing: Pence) -> Statement {
        Statement {
            id,
            label: format!("{start} to {end}"),
            start: start.into(),
            end: end.into(),
            due: None,
            previous,
            closing,
            new_debits: 0,
            new_credits: 0,
            source: None,
            digest: id.to_string(),
            warnings: vec![],
        }
    }
    fn two_months() -> Ledger {
        let mut opening = transaction(10000, Split::PersonOne);
        opening.kind = Kind::Opening;
        let mut carried = transaction(22000, Split::Unassigned);
        carried.kind = Kind::Opening;
        carried.statement_id = 2;
        let mut purchase = transaction(5000, Split::PersonTwo);
        purchase.statement_id = 2;
        let mut refund = transaction(-3000, Split::PersonOne);
        refund.statement_id = 2;
        let mut repayment = payment(2000, Split::Joint, Route::Amex);
        repayment.statement_id = 2;
        repayment.imported = true;
        let mut earlier = payment(8000, Split::PersonOne, Route::Amex);
        earlier.imported = true;
        Ledger {
            statements: vec![
                statement(1, "2026-09-01", "2026-09-30", 10000, 22000),
                statement(2, "2026-10-01", "2026-10-31", 22000, 22000),
            ],
            transactions: vec![
                opening,
                transaction(20000, Split::Joint),
                carried,
                purchase,
                refund,
            ],
            payments: vec![earlier, repayment],
            ..Ledger::default()
        }
    }
    #[test]
    fn combined_history_counts_carryover_once_and_keeps_refunds_and_payments() {
        let mut ledger = two_months();
        let account = ledger.account_summary();
        assert_eq!(account.summary.bank_remaining, 22000);
        assert_eq!(account.summary.remaining, [8000, 14000]);
        assert_eq!(account.summary.opening, 10000);
        assert_eq!(account.summary.refunds, 3000);
        assert!(account.summary.complete());
        assert_eq!(ledger.summary(2).unassigned_count, 1); // The individual snapshot is unchanged.
        ledger.statements.reverse(); // Import order must not choose the starting balance.
        assert_eq!(ledger.account_summary().summary.remaining, [8000, 14000]);
        assert_eq!(ledger.latest_statement().unwrap().id, 2);
        let mut early = payment(4000, Split::PersonOne, Route::Amex);
        early.date = "2026-11-01".into();
        ledger.payments.push(early);
        ledger
            .payments
            .push(payment(1000, Split::PersonTwo, Route::PersonTwoToPersonOne));
        let account = ledger.account_summary();
        assert_eq!(account.summary.bank_remaining, 18000);
        assert_eq!(account.summary.remaining, [5000, 13000]);
        assert_eq!(account.extra_bank_paid, 4000);
    }
    #[test]
    fn removing_and_restoring_preserves_allocation_payments_and_receipts() {
        let mut ledger = two_months();
        ledger.transactions[0].receipt = Some("/saved/receipt.pdf".into());
        ledger.transactions[0].note = "Keep this allocation".into();
        let expected = ledger.account_summary().summary.remaining;
        let next_id = ledger.next_id;
        ledger.remove_statement(1).unwrap();
        assert_eq!(ledger.statements.len(), 1);
        assert!(ledger.transactions.iter().all(|t| t.statement_id == 2));
        assert!(ledger.payments.iter().all(|p| p.statement_id == 2));
        assert_eq!(ledger.account_summary().summary.bank_remaining, 22000);
        assert_eq!(ledger.account_summary().summary.unassigned_count, 1);
        let removed = &ledger.removed_statements[0];
        assert_eq!(removed.transactions.len(), 2);
        assert_eq!(removed.payments.len(), 1);
        assert_eq!(
            removed.transactions[0].receipt.as_deref(),
            Some("/saved/receipt.pdf")
        );
        ledger.restore_statement(1).unwrap();
        assert!(ledger.removed_statements.is_empty());
        assert_eq!(ledger.account_summary().summary.remaining, expected);
        assert_eq!(ledger.account_summary().summary.bank_remaining, 22000);
        assert_eq!(ledger.next_id, next_id);
        assert!(
            ledger
                .transactions
                .iter()
                .any(|t| t.note == "Keep this allocation" && t.split == Split::PersonOne)
        );
        let before = serde_json::to_string(&ledger).unwrap();
        assert!(ledger.remove_statement(999).is_err());
        assert!(ledger.restore_statement(999).is_err());
        assert_eq!(serde_json::to_string(&ledger).unwrap(), before);
    }
    #[test]
    fn restoring_an_overlapping_statement_does_not_mutate_the_ledger() {
        let mut ledger = two_months();
        ledger.remove_statement(1).unwrap();
        ledger
            .statements
            .push(statement(3, "2026-09-01", "2026-09-30", 0, 0));
        let before = serde_json::to_string(&ledger).unwrap();
        assert!(ledger.restore_statement(1).is_err());
        assert_eq!(serde_json::to_string(&ledger).unwrap(), before);
    }
    #[test]
    fn recorded_payment_later_on_statement_is_counted_once_with_its_attribution() {
        let mut ledger = two_months();
        ledger.payments[0].split = Split::Unassigned;
        let mut manual = payment(8000, Split::PersonOne, Route::Amex);
        manual.statement_id = 2; // A matching date/amount can belong to another snapshot.
        ledger.payments.push(manual);
        let before = serde_json::to_string(&ledger).unwrap();
        let account = ledger.account_summary();
        assert_eq!(account.matched_payments, 1);
        assert_eq!(account.extra_bank_paid, 0);
        assert_eq!(account.summary.bank_remaining, 22000);
        assert_eq!(account.summary.remaining, [8000, 14000]);
        assert!(account.summary.complete());
        assert_eq!(serde_json::to_string(&ledger).unwrap(), before);
        // A second genuine statement payment with the same date/amount survives.
        ledger.payments.push(ledger.payments[0].clone());
        assert_eq!(ledger.account_summary().summary.bank_paid, 18000);
        assert_eq!(ledger.account_summary().summary.unassigned_payment_count, 1);
    }
    #[test]
    fn conflicting_payment_attributions_require_review_without_double_payment() {
        let mut ledger = two_months();
        let mut manual = payment(8000, Split::PersonTwo, Route::Amex);
        manual.statement_id = 2;
        ledger.payments.push(manual);
        let account = ledger.account_summary();
        assert_eq!(account.summary.bank_remaining, 22000);
        assert_eq!(account.summary.remaining, [8000, 14000]);
        assert!(!account.summary.complete());
        assert_eq!(account.warnings.len(), 1);
    }
    #[test]
    fn separate_manual_payments_with_same_amount_and_day_both_count() {
        let ledger = Ledger {
            statements: vec![statement(1, "2026-10-01", "2026-10-31", 0, 10000)],
            transactions: vec![transaction(10000, Split::Joint)],
            payments: vec![
                payment(2500, Split::PersonOne, Route::Amex),
                payment(2500, Split::PersonTwo, Route::Amex),
            ],
            ..Ledger::default()
        };
        let statement = ledger.summary(1);
        let account = ledger.account_summary();
        assert_eq!(statement.contributions, [2500, 2500]);
        assert_eq!(statement.remaining, [2500, 2500]);
        assert_eq!(account.summary.remaining, statement.remaining);
        assert_eq!(account.summary.bank_remaining, 5000);
        assert_eq!(account.extra_bank_paid, 5000);
        assert_eq!(account.matched_payments, 0);
    }
    #[test]
    fn new_payment_matching_an_entry_on_same_statement_is_separate() {
        let mut imported = payment(2500, Split::PersonTwo, Route::Amex);
        imported.imported = true;
        let ledger = Ledger {
            statements: vec![statement(1, "2026-10-01", "2026-10-31", 0, 7500)],
            transactions: vec![transaction(10000, Split::Joint)],
            payments: vec![imported, payment(2500, Split::PersonOne, Route::Amex)],
            ..Ledger::default()
        };
        let account = ledger.account_summary();
        assert_eq!(account.summary.contributions, [2500, 2500]);
        assert_eq!(account.summary.bank_remaining, 5000);
        assert_eq!(account.extra_bank_paid, 2500);
        assert_eq!(account.matched_payments, 0);
        assert!(account.warnings.is_empty());
        assert_eq!(account.summary.remaining, ledger.summary(1).remaining);
    }
    #[test]
    fn missing_history_is_provisional_and_balance_difference_stays_unallocated() {
        let mut ledger = two_months();
        ledger.statements[1].start = "2026-11-01".into();
        ledger.statements[1].end = "2026-11-30".into();
        ledger.statements[1].previous = 35000;
        let account = ledger.account_summary();
        assert_eq!(account.summary.bank_remaining, 35000);
        assert_eq!(account.summary.remaining, [8000, 14000]);
        assert_eq!(account.summary.unassigned_spend, 13000);
        assert_eq!(account.balance_adjustment, 13000);
        assert!(!account.summary.complete());
        assert_eq!(account.warnings.len(), 1);
        // A gap still needs review when its net balance change happens to be zero.
        ledger.statements[1].previous = 22000;
        assert!(!ledger.account_summary().summary.complete());
        ledger.statements[1].start = "2026-10-01".into();
        ledger.statements[1].previous = 21000;
        assert_eq!(ledger.account_summary().balance_adjustment, -1000);
        assert!(!ledger.account_summary().summary.complete());
    }
    #[test]
    fn refunds_reverse_joint_spend_including_odd_pennies() {
        let l = Ledger {
            transactions: vec![
                transaction(10001, Split::Joint),
                transaction(-10001, Split::Joint),
            ],
            ..Ledger::default()
        };
        let s = l.summary(1);
        assert_eq!(s.shares, [0, 0]);
        assert_eq!(s.refunds, 10001);
    }
    #[test]
    fn transfer_is_not_a_second_bank_payment() {
        let l = Ledger {
            transactions: vec![transaction(80000, Split::Joint)],
            payments: vec![
                payment(80000, Split::PersonOne, Route::Amex),
                payment(40000, Split::PersonTwo, Route::PersonTwoToPersonOne),
            ],
            ..Ledger::default()
        };
        let s = l.summary(1);
        assert_eq!(s.bank_remaining, 0);
        assert_eq!(s.remaining, [0, 0]);
        assert_eq!(s.bank_paid, 80000);
    }
    #[test]
    fn unattributed_payment_reduces_bank_balance_but_does_not_guess_payer() {
        let l = Ledger {
            transactions: vec![transaction(10000, Split::Joint)],
            payments: vec![payment(3000, Split::Unassigned, Route::Amex)],
            ..Ledger::default()
        };
        let s = l.summary(1);
        assert_eq!(s.bank_remaining, 7000);
        assert_eq!(s.remaining, [5000, 5000]);
        assert!(!s.complete());
    }
    #[test]
    fn partial_and_overpayments_are_preserved_as_credit() {
        let l = Ledger {
            transactions: vec![transaction(10000, Split::Joint)],
            payments: vec![
                payment(6000, Split::PersonOne, Route::Amex),
                payment(1000, Split::PersonTwo, Route::Amex),
            ],
            ..Ledger::default()
        };
        assert_eq!(l.summary(1).remaining, [-1000, 4000]);
    }
    #[test]
    fn exact_currency_parsing() {
        assert_eq!(parse_money("£1,234.56 CR").unwrap(), -123456);
        assert_eq!(parse_money("(0.51)").unwrap(), -51);
        assert_eq!(parse_money("2.1").unwrap(), 210);
        assert!(parse_money("2.001").is_err());
        assert!(parse_money("NaN").is_err());
        assert!(parse_money("1-2.34").is_err());
        assert!(parse_money("1,23.45").is_err());
        assert_eq!(money(-123456), "−£1,234.56");
    }
    #[test]
    fn custom_refund_must_be_within_signed_amount() {
        assert!(validate_custom(-5000, -2000).is_ok());
        assert!(validate_custom(-5000, 2000).is_err());
    }
}

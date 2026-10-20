use crate::model::{Kind, Pence, parse_money};
use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDate};
use pdf_extract::{MediaBox, OutputDev, OutputError, Transform};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub date: String,
    pub processed: String,
    pub merchant: String,
    pub amount: Pence,
    pub cardholder: String,
    pub payment: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct Preview {
    pub path: PathBuf,
    pub digest: String,
    pub start: String,
    pub end: String,
    pub due: Option<String>,
    pub previous: Pence,
    pub debits: Pence,
    pub credits: Pence,
    pub closing: Pence,
    pub records: Vec<Draft>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub verified: bool,
}

impl Preview {
    pub fn net_spend(&self) -> Pence {
        self.records
            .iter()
            .filter(|r| !r.payment)
            .map(|r| r.amount)
            .sum()
    }
    pub fn payments(&self) -> Pence {
        self.records
            .iter()
            .filter(|r| r.payment)
            .map(|r| -r.amount)
            .sum()
    }
    pub fn refunds(&self) -> Pence {
        self.records
            .iter()
            .filter(|r| !r.payment && r.amount < 0)
            .map(|r| -r.amount)
            .sum()
    }
    pub fn kind(r: &Draft) -> Kind {
        if r.amount < 0 {
            Kind::Refund
        } else {
            Kind::Purchase
        }
    }
}

#[derive(Default)]
struct LayoutText {
    pages: Vec<String>,
    glyphs: Vec<Glyph>,
}
struct Glyph {
    x: f64,
    y: f64,
    end: f64,
    text: String,
}

impl OutputDev for LayoutText {
    fn begin_page(
        &mut self,
        _: u32,
        _: &MediaBox,
        _: Option<(f64, f64, f64, f64)>,
    ) -> std::result::Result<(), OutputError> {
        self.glyphs.clear();
        Ok(())
    }
    fn end_page(&mut self) -> std::result::Result<(), OutputError> {
        self.glyphs.sort_by(|a, b| b.y.total_cmp(&a.y));
        let mut rows: Vec<(f64, Vec<&Glyph>)> = vec![];
        for g in &self.glyphs {
            if let Some((y, row)) = rows.last_mut().filter(|(y, _)| (g.y - *y).abs() < 2.0) {
                let _ = y;
                row.push(g);
            } else {
                rows.push((g.y, vec![g]));
            }
        }
        let mut page = String::new();
        for (_, mut row) in rows {
            row.sort_by(|a, b| a.x.total_cmp(&b.x));
            let mut end = 0.0;
            for g in row {
                if !page.ends_with('\n') && !page.is_empty() && g.x > end + 2.0 {
                    page.push_str(
                        &" ".repeat(((g.x - end) / 4.0).round().clamp(1.0, 80.0) as usize),
                    );
                }
                page.push_str(&g.text);
                end = g.end;
            }
            page.push('\n');
        }
        self.pages.push(page);
        Ok(())
    }
    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _: f64,
        font_size: f64,
        text: &str,
    ) -> std::result::Result<(), OutputError> {
        self.glyphs.push(Glyph {
            x: trm.m31,
            y: trm.m32,
            end: trm.m31 + width * font_size * trm.m11.abs(),
            text: text.into(),
        });
        Ok(())
    }
    fn begin_word(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }
}

pub fn pdf_text(path: &Path) -> Result<String> {
    let doc = pdf_extract::Document::load(path).context(
        "Could not read this PDF. Use the original downloaded statement, rather than a scan.",
    )?;
    if doc.is_encrypted() {
        bail!("This PDF is password protected. Save an unlocked copy and import that instead.");
    }
    let mut output = LayoutText::default();
    pdf_extract::output_doc(&doc, &mut output).context("Could not extract statement text")?;
    Ok(output.pages.join("\n"))
}

pub fn read(path: &Path) -> Result<Preview> {
    let bytes = std::fs::read(path).context("Could not open the selected file")?;
    if bytes.len() > 30 * 1024 * 1024 {
        bail!("Please choose a statement smaller than 30 MB");
    }
    let digest = format!("{:x}", Sha256::digest(&bytes));
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "pdf" => parse_pdf_text(&pdf_text(path)?, path.to_owned(), digest),
        "csv" => parse_csv(&bytes, path.to_owned(), digest),
        _ => bail!("Choose an Amex PDF statement or a CSV transaction export"),
    }
}

fn re(s: &str) -> Regex {
    Regex::new(s).expect("static regular expression")
}
fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn month(s: &str) -> u32 {
    match &s.to_ascii_lowercase()[..3] {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => 0,
    }
}
fn statement_date(m: &str, day: &str, end: NaiveDate) -> Result<String> {
    let m = month(m);
    let year = if m > end.month() {
        end.year() - 1
    } else {
        end.year()
    };
    Ok(NaiveDate::from_ymd_opt(year, m, day.parse()?)
        .context("Invalid transaction date in the statement")?
        .to_string())
}
pub fn date(s: &str) -> Result<String> {
    let formats = if re(r"^\d{2}/\d{2}/\d{2}$").is_match(s.trim()) {
        vec!["%d/%m/%y"]
    } else {
        vec!["%Y-%m-%d", "%d/%m/%Y", "%d %b %Y", "%d %B %Y"]
    };
    for fmt in formats {
        if let Ok(d) = NaiveDate::parse_from_str(s.trim(), fmt) {
            return Ok(d.to_string());
        }
    }
    bail!("Use a date in YYYY-MM-DD or DD/MM/YYYY format")
}

pub fn parse_pdf_text(text: &str, path: PathBuf, digest: String) -> Result<Preview> {
    let flat = normalize(text);
    let end_s = re(r"\b\d{2}/\d{2}/\d{2,4}\b").find(&flat).context("Could not find the statement date. This importer supports downloaded UK Amex statements.")?.as_str();
    let end = NaiveDate::parse_from_str(&date(end_s)?, "%Y-%m-%d")?;
    let period =
        re(r"(?i)From\s*(\d{1,2})\s*([A-Za-z]+)\s*to\s*(\d{1,2})\s*([A-Za-z]+)\s*(20\d{2})");
    let start = if let Some(c) = period.captures(&flat) {
        statement_date(&c[2], &c[1], end)?
    } else {
        end.with_day(1).unwrap().to_string()
    };
    let header = flat
        .find("Previous Closing Balance")
        .context("Could not find the Amex account summary")?;
    let summary = &flat[header..flat.len().min(header + 500)];
    let amounts: Vec<_> = re(r"\d[\d,]*\.\d{2}")
        .find_iter(summary)
        .take(4)
        .map(|m| parse_money(m.as_str()))
        .collect::<Result<_>>()?;
    if amounts.len() != 4 {
        bail!("Could not read all four account summary totals. Nothing has been imported.");
    }
    let mut p = Preview {
        path,
        digest,
        start,
        end: end.to_string(),
        due: None,
        previous: amounts[0],
        debits: amounts[2],
        credits: amounts[1],
        closing: amounts[3],
        records: vec![],
        warnings: vec![],
        errors: vec![],
        verified: true,
    };
    if let Some(pos) = flat.find("Payment Due Date") {
        let due = re(
            r"(\d{1,2})\s*(January|February|March|April|May|June|July|August|September|October|November|December)\s*(20\d{2})",
        );
        if let Some(c) = due.captures(&flat[pos..flat.len().min(pos + 180)]) {
            p.due = NaiveDate::from_ymd_opt(c[3].parse()?, month(&c[2]), c[1].parse()?)
                .map(|d| d.to_string());
        }
    }
    let row_re = re(
        r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s*(\d{1,2})\s+(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s*(\d{1,2})\s+(.+?)\s+([\d,]+\.\d{2})(?:\s*(CR))?$",
    );
    let dated = re(r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s*\d{1,2}\b");
    let card_total =
        re(r"^Total new spend transactions for\s+(.+?)\s+(-?[\d,]+\.\d{2})(?:\s*(CR))?$");
    let mut chunk_start = 0;
    let mut card_totals = 0;
    for raw in text.lines() {
        let line = normalize(raw);
        if let Some(c) = row_re.captures(&line) {
            let merchant = normalize(&c[5]);
            let payment = merchant.to_uppercase().starts_with("PAYMENT RECEIVED");
            let amount = parse_money(&c[6])? * if payment || c.get(7).is_some() { -1 } else { 1 };
            p.records.push(Draft {
                date: statement_date(&c[1], &c[2], end)?,
                processed: statement_date(&c[3], &c[4], end)?,
                merchant,
                amount,
                cardholder: String::new(),
                payment,
                fingerprint: String::new(),
            });
        } else if line == "CR" {
            if let Some(r) = p.records.last_mut() {
                r.amount = -r.amount.abs();
            }
        } else if let Some(c) = card_total.captures(&line) {
            let name = normalize(&c[1]);
            let expected = parse_money(&c[2])? * if c.get(3).is_some() { -1 } else { 1 };
            let mut actual = 0;
            for row in &mut p.records[chunk_start..] {
                if !row.payment {
                    row.cardholder = name.clone();
                    actual += row.amount;
                }
            }
            if actual != expected {
                p.errors.push(format!(
                    "{} card subtotal differs by {}",
                    name,
                    crate::model::money(actual - expected)
                ));
            }
            chunk_start = p.records.len();
            card_totals += 1;
        } else if dated.is_match(&line) {
            p.errors
                .push(format!("Could not read a dated transaction row: {line}"));
        }
    }
    if p.records.is_empty() {
        bail!("No readable transactions found. Scanned PDFs need a text-based PDF or CSV export.");
    }
    if card_totals == 0 {
        p.warnings.push(
            "Cardholder subtotals were not found. Check the cardholder column before importing."
                .into(),
        );
    }
    if p.records
        .iter()
        .any(|r| !r.payment && r.cardholder.is_empty())
    {
        p.warnings
            .push("Some transactions have no recognised cardholder.".into());
    }
    let actual_debits: i64 = p
        .records
        .iter()
        .filter(|r| r.amount > 0)
        .map(|r| r.amount)
        .sum();
    let actual_credits: i64 = p
        .records
        .iter()
        .filter(|r| r.amount < 0)
        .map(|r| -r.amount)
        .sum();
    if actual_debits != p.debits {
        p.errors.push(format!(
            "Purchases do not match New Debits: parsed {}, statement {}",
            crate::model::money(actual_debits),
            crate::model::money(p.debits)
        ));
    }
    if actual_credits != p.credits {
        p.errors.push(format!(
            "Refunds + repayments do not match New Credits: parsed {}, statement {}",
            crate::model::money(actual_credits),
            crate::model::money(p.credits)
        ));
    }
    if p.previous + p.debits - p.credits != p.closing {
        p.errors.push(
            "The account summary does not balance. Check whether the PDF was extracted correctly."
                .into(),
        );
    }
    fingerprint_rows(&mut p.records);
    Ok(p)
}

fn fingerprint_rows(rows: &mut [Draft]) {
    let mut occurrences = HashMap::<String, usize>::new();
    for r in rows {
        let base = format!(
            "{}|{}|{}|{}|{}|{}",
            r.date, r.processed, r.merchant, r.amount, r.cardholder, r.payment
        );
        let n = occurrences.entry(base.clone()).or_default();
        r.fingerprint = format!("{:x}", Sha256::digest(format!("{base}|{n}")));
        *n += 1;
    }
}

fn parse_csv(bytes: &[u8], path: PathBuf, digest: String) -> Result<Preview> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let delimiter = if text.lines().next().unwrap_or("").matches(';').count()
        > text.lines().next().unwrap_or("").matches(',').count()
    {
        b';'
    } else {
        b','
    };
    let mut csv = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let headers = csv.headers()?.clone();
    let find = |names: &[&str]| {
        headers
            .iter()
            .position(|h| names.iter().any(|n| h.eq_ignore_ascii_case(n)))
    };
    let d = find(&["Date", "Transaction Date"]).context("CSV needs a Date column")?;
    let m = find(&["Description", "Transaction Details", "Merchant", "Payee"])
        .context("CSV needs a Description column")?;
    let a = find(&["Amount", "Amount GBP", "Amount £"])
        .context("CSV needs an Amount column (GBP, charges positive and credits negative)")?;
    let owner = find(&[
        "Card Member",
        "Cardmember",
        "Cardholder",
        "Card Member Name",
    ]);
    let process = find(&["Process Date", "Processed Date", "Posting Date"]);
    let mut records = vec![];
    for (i, row) in csv.records().enumerate() {
        let row = row.with_context(|| format!("CSV row {} is malformed", i + 2))?;
        let on = date(row.get(d).unwrap_or(""))
            .with_context(|| format!("CSV row {} has an invalid date", i + 2))?;
        let processed = match process.and_then(|j| row.get(j)).filter(|s| !s.is_empty()) {
            Some(s) => date(s)?,
            None => on.clone(),
        };
        let merchant = row.get(m).unwrap_or("").to_owned();
        let payment = merchant.to_uppercase().starts_with("PAYMENT RECEIVED")
            || merchant.to_uppercase().starts_with("PAYMENT THANK");
        let mut amount = parse_money(row.get(a).unwrap_or(""))
            .with_context(|| format!("CSV row {} has an invalid amount", i + 2))?;
        if payment {
            amount = -amount.abs();
        }
        if merchant.is_empty() {
            bail!("CSV row {} has no description", i + 2);
        }
        records.push(Draft {
            date: on,
            processed,
            merchant,
            amount,
            cardholder: owner.and_then(|j| row.get(j)).unwrap_or("").into(),
            payment,
            fingerprint: String::new(),
        });
    }
    if records.is_empty() {
        bail!("This CSV contains no transactions");
    }
    fingerprint_rows(&mut records);
    let start = records.iter().map(|r| r.date.clone()).min().unwrap();
    let end = records.iter().map(|r| r.date.clone()).max().unwrap();
    let debits = records
        .iter()
        .filter(|r| r.amount > 0)
        .map(|r| r.amount)
        .sum();
    let credits = records
        .iter()
        .filter(|r| r.amount < 0)
        .map(|r| -r.amount)
        .sum();
    Ok(Preview { path, digest, start, end, due: None, previous: 0, debits, credits, closing: debits - credits, records, warnings: vec!["CSV amounts could not be checked against a PDF statement. Charges must be positive; refunds negative. Choose the correct statement dates and opening balance below.".into()], errors: vec![], verified: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = "Date 05/10/26\nPrevious Closing Balance New Credits New Debits Closing Balance\n£100.00 £80.00 £60.00 £80.00\nStatement Period From 6 September to 5 October 2026\nSep10 Sep10 PAYMENT RECEIVED - THANK YOU 50.00\nCR\nSep12 Sep12 CAFE LONDON 30.00\nSep12 Sep12 CAFE LONDON 30.00\nSep13 Sep13 CAFE LONDON 30.00\nCR\nTotal new spend transactions for CARD MEMBER ONE 30.00\n";
    #[test]
    fn signed_refunds_payments_and_repeated_real_purchases() {
        let p = parse_pdf_text(SAMPLE, "sample.pdf".into(), "a".into()).unwrap();
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.records.len(), 4);
        assert_eq!(p.refunds(), 3000);
        assert_eq!(p.payments(), 5000);
        assert_ne!(p.records[1].fingerprint, p.records[2].fingerprint);
        assert_eq!(p.records[3].cardholder, "CARD MEMBER ONE");
    }
    #[test]
    fn missing_row_blocks_import_instead_of_silently_losing_money() {
        let p = parse_pdf_text(
            &SAMPLE.replace("Sep13 Sep13 CAFE LONDON 30.00\nCR\n", ""),
            "x.pdf".into(),
            "a".into(),
        )
        .unwrap();
        assert!(!p.errors.is_empty());
    }
    #[test]
    fn december_transactions_on_january_statement_use_previous_year() {
        let end = NaiveDate::from_ymd_opt(2027, 1, 5).unwrap();
        assert_eq!(statement_date("Dec", "28", end).unwrap(), "2026-12-28");
    }
    #[test]
    fn amex_two_digit_year_is_2026_not_0026() {
        assert_eq!(date("05/10/26").unwrap(), "2026-10-05");
        assert_eq!(date("05/10/2026").unwrap(), "2026-10-05");
    }
    #[test]
    fn csv_accepts_bom_and_quoted_money_but_rejects_bad_rows() {
        let csv = "\u{feff}Date,Description,Amount,Card Member\n01/10/2026,SHOP,\"1,234.56\",CARD MEMBER TWO\n02/10/2026,REFUND,-30.00,CARD MEMBER TWO\n";
        let p = parse_csv(csv.as_bytes(), "x.csv".into(), "a".into()).unwrap();
        assert_eq!(p.net_spend(), 120456);
        assert!(
            parse_csv(
                b"Date,Description,Amount\nbad,SHOP,10.00",
                "x.csv".into(),
                "a".into()
            )
            .is_err()
        );
    }
}

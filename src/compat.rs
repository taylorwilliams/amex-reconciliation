//! Optional, local mappings for older saved formats. No personal names belong here.
use crate::model::*;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Deserialize)]
pub struct LegacyProfile {
    pub people: [Person; 2],
    pub split_aliases: BTreeMap<String, Split>,
    pub route_aliases: BTreeMap<String, Route>,
    pub custom_share_field: String,
}

pub fn decode(data: &[u8], profile_path: &Path) -> Result<Ledger> {
    let mut value: Value = serde_json::from_slice(data)?;
    match value.get("version").and_then(Value::as_u64) {
        Some(1) => {
            let profile: LegacyProfile = serde_json::from_slice(&std::fs::read(profile_path)
                .context("This older ledger needs its local legacy-profile.json migration file. Your data has not been changed.")?)?;
            validate_people(&profile.people)?;
            migrate(&mut value, &profile)?;
            value["version"] = Value::from(2);
            value["people"] = serde_json::to_value(profile.people)?;
        }
        Some(2) => {}
        _ => bail!(
            "This ledger needs a different version of Amex Together. Your data has not been changed."
        ),
    }
    let ledger: Ledger = serde_json::from_value(value)?;
    validate_people(&ledger.people)?;
    Ok(ledger)
}

fn migrate(value: &mut Value, profile: &LegacyProfile) -> Result<()> {
    match value {
        Value::Object(object) => {
            if let Some(old) = object.remove(&profile.custom_share_field) {
                if object.contains_key("custom_person_two") {
                    bail!("The legacy ledger contains conflicting custom-share fields");
                }
                object.insert("custom_person_two".into(), old);
            }
            if let Some(Value::String(name)) = object.get_mut("split")
                && let Some(split) = profile.split_aliases.get(name)
            {
                *name = serde_json::to_value(split)?.as_str().unwrap().into();
            }
            if let Some(Value::String(name)) = object.get_mut("route")
                && let Some(route) = profile.route_aliases.get(name)
            {
                *name = serde_json::to_value(route)?.as_str().unwrap().into();
            }
            for value in object.values_mut() {
                migrate(value, profile)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                migrate(value, profile)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_fixture() -> (Value, Value) {
        let mut ledger = Ledger::default();
        crate::store::seed_demo(&mut ledger);
        ledger.transactions[0].split = Split::Custom;
        ledger.transactions[0].custom_person_two = 1234;
        ledger.transactions[0].receipt = Some("receipts/example.pdf".into());
        ledger.payments[0].route = Route::PersonTwoToPersonOne;
        ledger.remove_statement(1).unwrap();
        let current = serde_json::to_value(&ledger).unwrap();
        let mut legacy = current.clone();
        fn convert(value: &mut Value) {
            match value {
                Value::Object(object) => {
                    if let Some(share) = object.remove("custom_person_two") {
                        object.insert("old_second_share".into(), share);
                    }
                    if let Some(Value::String(split)) = object.get_mut("split") {
                        *split = match split.as_str() {
                            "PersonOne" => "OldFirst",
                            "PersonTwo" => "OldSecond",
                            other => other,
                        }
                        .into();
                    }
                    if let Some(Value::String(route)) = object.get_mut("route") {
                        *route = match route.as_str() {
                            "PersonTwoToPersonOne" => "OldSecondToFirst",
                            "PersonOneToPersonTwo" => "OldFirstToSecond",
                            other => other,
                        }
                        .into();
                    }
                    for value in object.values_mut() {
                        convert(value);
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        convert(value);
                    }
                }
                _ => {}
            }
        }
        convert(&mut legacy);
        legacy["version"] = Value::from(1);
        legacy.as_object_mut().unwrap().remove("people");
        (legacy, current)
    }

    fn profile() -> Value {
        serde_json::json!({
            "people": [{"name":"Alex", "cardholder_names":["ALEX SAMPLE"]}, {"name":"Sam", "cardholder_names":["SAM SAMPLE"]}],
            "split_aliases": {"OldFirst":"PersonOne", "OldSecond":"PersonTwo"},
            "route_aliases": {"OldSecondToFirst":"PersonTwoToPersonOne", "OldFirstToSecond":"PersonOneToPersonTwo"},
            "custom_share_field": "old_second_share"
        })
    }

    #[test]
    fn migration_preserves_removed_statements_custom_shares_payments_and_receipts() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("legacy-profile.json");
        std::fs::write(&path, serde_json::to_vec(&profile()).unwrap()).unwrap();
        let (legacy, expected) = legacy_fixture();
        let migrated = decode(&serde_json::to_vec(&legacy).unwrap(), &path).unwrap();
        assert_eq!(serde_json::to_value(&migrated).unwrap(), expected);
        // New backups no longer need the legacy profile.
        std::fs::remove_file(&path).unwrap();
        let reopened = decode(&serde_json::to_vec(&migrated).unwrap(), &path).unwrap();
        assert_eq!(serde_json::to_value(reopened).unwrap(), expected);
    }

    #[test]
    fn legacy_without_profile_fails_without_changing_the_input() {
        let folder = tempfile::tempdir().unwrap();
        let (legacy, _) = legacy_fixture();
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let before = bytes.clone();
        assert!(decode(&bytes, &folder.path().join("missing.json")).is_err());
        assert_eq!(bytes, before);
    }

    #[test]
    fn changing_names_preserves_balances_and_person_ids() {
        let mut ledger = Ledger::default();
        crate::store::seed_demo(&mut ledger);
        let before = ledger.summary(1);
        let split = ledger.transactions[1].split;
        ledger.people[0].name = "Casey".into();
        ledger.people[1].name = "Jordan".into();
        validate_people(&ledger.people).unwrap();
        assert_eq!(ledger.summary(1).remaining, before.remaining);
        assert_eq!(ledger.summary(1).contributions, before.contributions);
        assert_eq!(ledger.transactions[1].split, split);
        assert_eq!(Split::PersonOne.label(&ledger.people), "Casey");
        assert_eq!(
            Route::PersonTwoToPersonOne.label(&ledger.people),
            "Jordan → Casey"
        );
        assert!(
            serde_json::to_string(&ledger)
                .unwrap()
                .contains("PersonOne")
        );
    }
}

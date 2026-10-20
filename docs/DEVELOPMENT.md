# Development notes

## Project structure

- `src/model.rs` calculates shares, contributions and balances in integer pence.
- `src/import.rs` reads statement PDFs and CSV files and checks their totals.
- `src/store.rs` saves the ledger and copies attachments into the workspace.
- `src/compat.rs` upgrades older ledgers using a local mapping file.
- `src/ui.rs` provides the desktop screens and settings.
- `src/main.rs` opens the app, selects the workspace and restores backups.

## Names and allocations

Each ledger stores two people, with a display name and optional card member names. Allocations use `PersonOne` and `PersonTwo`. Transfers use `PersonOneToPersonTwo` and `PersonTwoToPersonOne`. Custom allocations store the second person's exact share in `custom_person_two`.

Names can change without moving any spending or payments between people. Card member names change display labels only. They do not assign transactions automatically.

The supported statement format uses GBP. Multi-currency conversion and bank API access are outside the current scope.

## Upgrade an older ledger

Version 2 replaces name-based allocation identifiers with stable person IDs. Keep the data folder in place when updating an existing installation.

A version 1 ledger needs a `legacy-profile.json` file in its local data folder. This file contains the old identifiers and the names to display after the upgrade. It must stay out of Git.

Use this structure, replacing the example identifiers with those used by the old installation:

```json
{
  "people": [
    {"name": "Alex", "cardholder_names": ["ALEX SAMPLE"]},
    {"name": "Sam", "cardholder_names": ["SAM SAMPLE"]}
  ],
  "split_aliases": {"OldFirst": "PersonOne", "OldSecond": "PersonTwo"},
  "route_aliases": {
    "OldFirstToSecond": "PersonOneToPersonTwo",
    "OldSecondToFirst": "PersonTwoToPersonOne"
  },
  "custom_share_field": "old_second_share"
}
```

The upgrade covers active statements, removed statements, payments, custom splits, notes and attachment paths. It saves the original JSON under `recovery/` before the updated ledger is saved. If the profile is missing or invalid, loading fails and the existing ledger is kept.

New version 2 backups include the display names and stable IDs. They do not need a migration profile. Restoring a version 1 backup uses the local profile from the selected data folder.

## Build the app icon

The default icon is original vector artwork in `packaging/amex-app-icon.svg`. On macOS, regenerate the PNG and icon set with:

```sh
swift scripts/render-icon.swift
```

To use your own icon in a local build, set `AMEX_ICON_DIR` to a folder containing `amex-icon.png` and `AmexTogether.icns` when running `scripts/package-macos.sh`. This changes the app icon and sidebar image. Keep personal or third-party artwork outside the repository.

## Check changes

Run formatting, Clippy and the test suite before pushing. Tests cover signed refunds, odd pennies, partial payments, repeated payments, statement removal and restoration, import checks and older-ledger upgrades.

Use the built-in demo or the sample CSV for screenshots and bug reports. Never commit a real statement, receipt, database, recovery copy, migration profile or screenshot of personal data.

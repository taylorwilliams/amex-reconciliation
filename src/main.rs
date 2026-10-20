mod ui;
use amex_together::{
    model::Ledger,
    store::{Store, seed_demo},
};
use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        rfd::MessageDialog::new()
            .set_title("Amex Together")
            .set_description(format!("{e:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    let demo = args.iter().any(|s| s == "--demo");
    let mut root = arg("--data-dir").unwrap_or(
        directories::BaseDirs::new()
            .context("Could not find your app data folder")?
            .data_dir()
            .join("Amex Together"),
    );
    let mut restore: Option<Ledger> = None;
    if let Some(path) = arg("--restore") {
        let mut ledger = amex_together::compat::decode(
            &std::fs::read(&path)?,
            &root.join("legacy-profile.json"),
        )?;
        let base = path.parent().context("Invalid backup path")?;
        for s in ledger.statements.iter_mut().chain(
            ledger
                .removed_statements
                .iter_mut()
                .map(|removed| &mut removed.statement),
        ) {
            if let Some(p) = &s.source {
                s.source = Some(base.join(p).to_string_lossy().into_owned());
            }
        }
        for t in ledger.transactions.iter_mut().chain(
            ledger
                .removed_statements
                .iter_mut()
                .flat_map(|removed| &mut removed.transactions),
        ) {
            if let Some(p) = &t.receipt {
                t.receipt = Some(base.join(p).to_string_lossy().into_owned());
            }
        }
        root = root.join(format!("restored-{}", chrono::Local::now().timestamp()));
        restore = Some(ledger);
    }
    let store = Store::open(root, demo)?;
    let mut ledger = if let Some(mut restored) = restore {
        if restored.version != 2 {
            anyhow::bail!("This backup needs a newer app version");
        }
        for s in restored.statements.iter_mut().chain(
            restored
                .removed_statements
                .iter_mut()
                .map(|removed| &mut removed.statement),
        ) {
            if let Some(path) = &s.source {
                s.source =
                    Some(store.copy_attachment(&PathBuf::from(path), "statements", &s.digest)?);
            }
        }
        for t in restored.transactions.iter_mut().chain(
            restored
                .removed_statements
                .iter_mut()
                .flat_map(|removed| &mut removed.transactions),
        ) {
            if let Some(path) = &t.receipt {
                t.receipt = Some(store.copy_attachment(
                    &PathBuf::from(path),
                    "receipts",
                    &format!("restored-{}", t.id),
                )?);
            }
        }
        restored
    } else {
        store.load()?
    };
    if demo && ledger.statements.is_empty() {
        seed_demo(&mut ledger);
    }
    store.save(&ledger)?;
    let file = arg("--import");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(if demo {
                "Amex Together · Demo"
            } else {
                "Amex Together"
            })
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([1100.0, 720.0])
            .with_icon(app_icon()?)
            .with_app_id("org.amextogether.desktop"),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Amex Together",
        options,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, store, ledger, file)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
}

fn app_icon() -> Result<eframe::egui::IconData> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(contents) = exe.parent().and_then(|path| path.parent())
    {
        let path = contents.join("Resources/AppIcon.png");
        if path.is_file() {
            return Ok(eframe::icon_data::from_png_bytes(&std::fs::read(path)?)?);
        }
    }
    Ok(eframe::icon_data::from_png_bytes(include_bytes!(
        "../packaging/amex-icon.png"
    ))?)
}

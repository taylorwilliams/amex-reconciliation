use amex_together::{
    import::{self, Preview},
    model::*,
    store::{Store, seed_demo},
};
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use eframe::egui::{self, Color32, CornerRadius, FontId, Frame, Margin, RichText, Stroke, Vec2};
use egui_extras::{Column, TableBuilder};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

const BG: Color32 = Color32::from_rgb(9, 18, 33);
const NAV: Color32 = Color32::from_rgb(6, 13, 26);
const SURFACE: Color32 = Color32::from_rgb(16, 30, 51);
const ELEVATED: Color32 = Color32::from_rgb(23, 42, 68);
const INK: Color32 = Color32::from_rgb(241, 246, 255);
const MUTED: Color32 = Color32::from_rgb(146, 168, 199);
const BLUE: Color32 = Color32::from_rgb(35, 125, 242);
const PALE_BLUE: Color32 = Color32::from_rgb(151, 204, 255);
const GREEN: Color32 = Color32::from_rgb(112, 222, 188);
const LINE: Color32 = Color32::from_rgb(34, 55, 85);
const AMBER: Color32 = Color32::from_rgb(241, 195, 113);
const RED: Color32 = Color32::from_rgb(255, 141, 151);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Transactions,
    Payments,
    Import,
    Statements,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Transactions => "Transactions",
            Self::Payments => "Payments",
            Self::Import => "Import statement",
            Self::Statements => "Manage statements",
        }
    }
}

#[derive(Clone)]
struct Editor {
    id: u64,
    split: Split,
    second_share: String,
    note: String,
    expense: bool,
}

struct PaymentForm {
    id: Option<u64>,
    statement_id: Option<u64>,
    amount: String,
    date: String,
    split: Split,
    second_share: String,
    route: Route,
    note: String,
    error: String,
}

impl Default for PaymentForm {
    fn default() -> Self {
        Self {
            id: None,
            statement_id: None,
            amount: String::new(),
            date: chrono::Local::now().date_naive().to_string(),
            split: Split::PersonOne,
            second_share: String::new(),
            route: Route::Amex,
            note: String::new(),
            error: String::new(),
        }
    }
}

pub struct App {
    context: egui::Context,
    store: Store,
    ledger: Ledger,
    statement: Option<u64>,
    overview_statement: Option<u64>,
    logo: egui::TextureHandle,
    page: Page,
    search: String,
    filter: usize,
    card_filter: String,
    selected: BTreeSet<u64>,
    editor: Option<Editor>,
    payment_form: Option<PaymentForm>,
    preview: Option<Preview>,
    loading: Option<mpsc::Receiver<std::result::Result<Preview, String>>>,
    import_opening: String,
    import_start: String,
    import_end: String,
    csv_confirmed: bool,
    message: Option<(String, bool, Instant)>,
    undo: Option<Ledger>,
    delete_payment: Option<u64>,
    people_form: Option<[Person; 2]>,
    people_error: String,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        store: Store,
        ledger: Ledger,
        file: Option<PathBuf>,
    ) -> Self {
        let mut style = egui::Style {
            visuals: egui::Visuals::dark(),
            ..Default::default()
        };
        style.visuals.override_text_color = Some(INK);
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = SURFACE;
        style.visuals.window_stroke = Stroke::new(1.0, LINE);
        style.visuals.window_corner_radius = CornerRadius::same(16);
        style.visuals.extreme_bg_color = NAV;
        style.visuals.faint_bg_color = Color32::from_rgb(14, 26, 44);
        style.visuals.text_edit_bg_color = Some(SURFACE);
        style.visuals.weak_text_color = Some(MUTED);
        style.visuals.hyperlink_color = PALE_BLUE;
        style.visuals.selection.bg_fill = Color32::from_rgb(25, 67, 116);
        style.visuals.selection.stroke = Stroke::new(1.0, PALE_BLUE);
        style.visuals.widgets.noninteractive.bg_fill = ELEVATED;
        style.visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, INK);
        style.visuals.widgets.inactive.bg_fill = ELEVATED;
        style.visuals.widgets.inactive.weak_bg_fill = ELEVATED;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
        style.visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(31, 64, 105);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(31, 64, 105);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BLUE);
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, INK);
        style.visuals.widgets.active.bg_fill = BLUE;
        style.visuals.widgets.active.weak_bg_fill = BLUE;
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
        style.spacing.item_spacing = Vec2::new(10.0, 9.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.spacing.interact_size = Vec2::new(40.0, 32.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(30.0));
        // Use the Mac's system font at runtime, with bundled fonts as a fallback.
        if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("system-ui".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "system-ui".into());
            cc.egui_ctx.set_fonts(fonts);
        }
        cc.egui_ctx.set_global_style(style);
        let icon = crate::app_icon().expect("The bundled app icon is a valid PNG");
        let logo = cc.egui_ctx.load_texture(
            "amex-logo",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        let statement = ledger.latest_statement().map(|s| s.id);
        let mut app = Self {
            context: cc.egui_ctx.clone(),
            store,
            ledger,
            statement,
            overview_statement: None,
            logo,
            page: Page::Overview,
            search: String::new(),
            filter: 0,
            card_filter: "All cards".into(),
            selected: BTreeSet::new(),
            editor: None,
            payment_form: None,
            preview: None,
            loading: None,
            import_opening: String::new(),
            import_start: String::new(),
            import_end: String::new(),
            csv_confirmed: false,
            message: None,
            undo: None,
            delete_payment: None,
            people_form: None,
            people_error: String::new(),
        };
        if let Some(file) = file {
            app.start_import(file);
        }
        app
    }

    fn tell(&mut self, message: impl Into<String>, error: bool) {
        self.message = Some((message.into(), error, Instant::now()));
    }
    fn change(&mut self, f: impl FnOnce(&mut Ledger)) {
        let before = self.ledger.clone();
        f(&mut self.ledger);
        match self.store.save(&self.ledger) {
            Ok(()) => {
                self.undo = Some(before);
                self.tell("Saved on this laptop", false);
            }
            Err(e) => {
                self.ledger = before;
                self.tell(format!("Could not save: {e}"), true);
            }
        }
    }
    fn set_split(&mut self, ids: &[u64], split: Split) {
        self.change(|l| {
            for t in &mut l.transactions {
                if ids.contains(&t.id) {
                    t.split = split;
                }
            }
        });
        self.selected.clear();
        self.editor = None;
    }
    fn edit_history(&mut self, edit: impl FnOnce(&mut Ledger) -> Result<()>) -> bool {
        let mut next = self.ledger.clone();
        let result = (|| -> Result<()> {
            edit(&mut next)?;
            self.store.save_recovery(&self.ledger)?;
            self.store.save(&next)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.undo = Some(std::mem::replace(&mut self.ledger, next));
                if self
                    .statement
                    .is_none_or(|id| self.ledger.statement(id).is_none())
                {
                    self.statement = self.ledger.latest_statement().map(|s| s.id);
                }
                if self
                    .overview_statement
                    .is_some_and(|id| self.ledger.statement(id).is_none())
                {
                    self.overview_statement = None;
                }
                self.editor = None;
                self.payment_form = None;
                self.selected.clear();
                self.tell(
                    "Saved. A recovery copy of your previous ledger is kept on this laptop.",
                    false,
                );
                true
            }
            Err(error) => {
                self.tell(error.to_string(), true);
                false
            }
        }
    }
    fn open_editor(&mut self, id: u64) {
        if let Some(t) = self.ledger.transactions.iter().find(|t| t.id == id) {
            self.editor = Some(Editor {
                id,
                split: t.split,
                second_share: decimal(if t.split == Split::Custom {
                    t.custom_person_two
                } else {
                    t.amount / 2
                }),
                note: t.note.clone(),
                expense: t.expense,
            });
        }
    }
    fn start_import(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        let context = self.context.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| import::read(&path))
                .map_err(|_| "The PDF reader could not process this file. Try an original downloaded PDF or CSV.".to_owned())
                .and_then(|r| r.map_err(|e| format!("{e:#}")));
            let _ = tx.send(result);
            context.request_repaint();
        });
        self.loading = Some(rx);
        self.preview = None;
        self.page = Page::Import;
        self.editor = None;
    }
    fn pick_import(&mut self) {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("Amex statement", &["pdf", "csv"])
            .pick_file()
        {
            self.start_import(file);
        }
    }
    fn switch_workspace(&mut self, demo: bool) -> Result<()> {
        let store = Store::open(self.store.root.clone(), demo)?;
        let mut ledger = store.load()?;
        if demo && ledger.statements.is_empty() {
            seed_demo(&mut ledger);
            store.save(&ledger)?;
        }
        self.statement = ledger.latest_statement().map(|s| s.id);
        self.overview_statement = None;
        self.store = store;
        self.ledger = ledger;
        self.page = Page::Overview;
        self.editor = None;
        self.selected.clear();
        self.undo = None;
        self.preview = None;
        self.search.clear();
        self.filter = 0;
        self.card_filter = "All cards".into();
        Ok(())
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("navigation")
            .exact_size(210.0)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(NAV)
                    .inner_margin(Margin::symmetric(20, 26)),
            )
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(40.0)))
                        .on_hover_text("American Express");
                    ui.label(RichText::new("together").size(23.0).strong().color(INK));
                });
                ui.add_space(7.0);
                ui.label(RichText::new("YOUR AMEX COMPANION").size(9.0).color(MUTED));
                ui.add_space(32.0);
                eyebrow(ui, "WORKSPACE");
                ui.add_space(7.0);
                for page in [
                    Page::Overview,
                    Page::Transactions,
                    Page::Payments,
                    Page::Import,
                    Page::Statements,
                ] {
                    let label = match page {
                        Page::Overview => "Overview",
                        Page::Transactions => "Transactions",
                        Page::Payments => "Payments & transfers",
                        Page::Import => "Import statement",
                        Page::Statements => "Manage statements",
                    };
                    if navigation_button(ui, page, label, self.page == page).clicked() {
                        self.page = page;
                        self.editor = None;
                    }
                }
                ui.add_space(30.0);
                Frame::new()
                    .fill(SURFACE)
                    .corner_radius(10)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_width(146.0);
                        ui.horizontal(|ui| {
                            let (dot, _) =
                                ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                dot.center(),
                                3.0,
                                if self.store.demo { AMBER } else { GREEN },
                            );
                            ui.label(
                                RichText::new(if self.store.demo {
                                    "DEMO WORKSPACE"
                                } else {
                                    "PRIVATE WORKSPACE"
                                })
                                .size(9.0)
                                .strong()
                                .color(if self.store.demo { AMBER } else { PALE_BLUE }),
                            );
                        });
                        ui.add_space(5.0);
                        ui.label(
                            RichText::new(if self.store.demo {
                                "Sample data, separate from your real statement."
                            } else {
                                "Stored on this computer. Shared spending, kept private."
                            })
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        let (avatar, _) =
                            ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::hover());
                        ui.painter().circle_filled(avatar.center(), 15.0, ELEVATED);
                        ui.painter().text(
                            avatar.center(),
                            egui::Align2::CENTER_CENTER,
                            format!(
                                "{}+{}",
                                self.ledger.people[0].name.chars().next().unwrap_or('1'),
                                self.ledger.people[1].name.chars().next().unwrap_or('2')
                            ),
                            FontId::proportional(10.0),
                            PALE_BLUE,
                        );
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "{} & {}",
                                        self.ledger.people[0].name, self.ledger.people[1].name
                                    ))
                                    .size(12.0)
                                    .strong(),
                                )
                                .truncate(),
                            );
                            ui.label(RichText::new("Shared account").size(10.0).color(MUTED));
                        });
                    });
                    ui.add_space(13.0);
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Settings").size(11.0).color(MUTED))
                                .frame(false),
                        )
                        .clicked()
                    {
                        self.people_form = Some(self.ledger.people.clone());
                        self.people_error.clear();
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(if self.store.demo {
                                    "Open my real ledger"
                                } else {
                                    "Try the demo workspace"
                                })
                                .size(11.0)
                                .color(MUTED),
                            )
                            .frame(false),
                        )
                        .clicked()
                        && let Err(e) = self.switch_workspace(!self.store.demo)
                    {
                        self.tell(e.to_string(), true);
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Show saved files").size(11.0).color(MUTED),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        open_path(&self.store.root);
                    }
                    if self.undo.is_some()
                        && ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Undo last change")
                                        .size(11.0)
                                        .color(PALE_BLUE),
                                )
                                .frame(false),
                            )
                            .clicked()
                    {
                        self.undo_change();
                    }
                });
            });
    }
    fn undo_change(&mut self) {
        if let Some(previous) = self.undo.take() {
            match self.store.save(&previous) {
                Ok(()) => {
                    self.ledger = previous;
                    if self
                        .statement
                        .is_none_or(|id| self.ledger.statement(id).is_none())
                    {
                        self.statement = self.ledger.latest_statement().map(|s| s.id);
                    }
                    if self
                        .overview_statement
                        .is_some_and(|id| self.ledger.statement(id).is_none())
                    {
                        self.overview_statement = None;
                    }
                    self.editor = None;
                    self.tell("Last change undone", false);
                }
                Err(e) => self.tell(e.to_string(), true),
            }
        }
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(310.0, 62.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.heading(RichText::new(self.page.title()).strong());
                    ui.label(
                        RichText::new(
                            if self.page == Page::Overview && self.overview_statement.is_none() {
                                "Your full account, in one place."
                            } else {
                                "Your shared account. A clear balance."
                            },
                        )
                        .size(13.0)
                        .color(MUTED),
                    );
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary(ui, "+ Import statement").clicked() {
                    self.pick_import();
                }
                if self.page != Page::Statements && !self.ledger.statements.is_empty() {
                    let overview = self.page == Page::Overview;
                    let scope = if overview {
                        self.overview_statement
                    } else {
                        self.statement
                    };
                    let label = scope
                        .and_then(|id| self.ledger.statement(id))
                        .map(|s| format!("{} – {}", short_date(&s.start), short_date(&s.end)))
                        .unwrap_or_else(|| {
                            if overview {
                                "All statements".into()
                            } else {
                                "Choose statement".into()
                            }
                        });
                    let mut selected = scope;
                    egui::ComboBox::from_id_salt(("statement", overview))
                        .selected_text(label)
                        .width(195.0)
                        .show_ui(ui, |ui| {
                            if overview {
                                ui.selectable_value(&mut selected, None, "All statements");
                                ui.separator();
                            }
                            for s in &self.ledger.statements {
                                ui.selectable_value(
                                    &mut selected,
                                    Some(s.id),
                                    format!("{} – {}", short_date(&s.start), short_date(&s.end)),
                                );
                            }
                        });
                    if selected != scope {
                        if overview {
                            self.overview_statement = selected;
                        }
                        if selected.is_some() {
                            self.statement = selected;
                        }
                        self.selected.clear();
                        self.editor = None;
                    }
                }
            });
        });
        ui.add_space(18.0);
    }

    fn status_footer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("save_status")
            .exact_size(34.0)
            .resizable(false)
            .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(28, 7)))
            .show(ui, |ui| {
                let (text, color) = match &self.message {
                    Some((text, true, _)) => (text.as_str(), RED),
                    Some((text, false, time)) if time.elapsed() < Duration::from_secs(5) => {
                        ui.ctx().request_repaint_after(
                            Duration::from_secs(5).saturating_sub(time.elapsed()),
                        );
                        (text.as_str(), GREEN)
                    }
                    _ => ("Saved on this laptop", MUTED),
                };
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.0, color);
                    ui.add(
                        egui::Label::new(RichText::new(text).size(11.0).color(color)).truncate(),
                    )
                    .on_hover_text(text);
                });
            });
    }

    fn manage_statements(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Remove a statement from reconciliation, or restore one with its saved allocations.").color(MUTED));
        ui.add_space(12.0);
        let mut active = self.ledger.statements.clone();
        active.sort_by(|a, b| a.start.cmp(&b.start));
        let mut removed = self.ledger.removed_statements.clone();
        removed.sort_by(|a, b| a.statement.start.cmp(&b.statement.start));
        let mut action = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            eyebrow(ui, "IN YOUR WORKSPACE");
            if active.is_empty() { ui.label("No active statements. Import a statement or restore one below."); }
            for statement in &active {
                ui.push_id(("active_statement", statement.id), |ui| {
                    card().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(format!("{} – {}", short_date(&statement.start), short_date(&statement.end))).size(18.0).strong());
                                let transactions = self.ledger.transactions.iter().filter(|t| t.statement_id == statement.id).count();
                                let payments = self.ledger.payments.iter().filter(|p| p.statement_id == statement.id).count();
                                ui.label(RichText::new(format!("{transactions} spending/balance items · {payments} payments")).size(12.0).color(MUTED));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Remove").on_hover_text("Exclude this statement and its payments. Restore it below whenever you need it.").clicked() { action = Some((statement.id, false)); }
                                if ui.button("View").clicked() { self.statement = Some(statement.id); self.overview_statement = Some(statement.id); self.page = Page::Overview; }
                            });
                        });
                    });
                });
                ui.add_space(10.0);
            }
            ui.add_space(16.0);
            eyebrow(ui, "REMOVED STATEMENTS");
            ui.label(RichText::new("These do not affect your balance. Their original files, receipts and allocations are kept for restoration.").color(MUTED).size(12.0));
            if removed.is_empty() { ui.label("No removed statements."); }
            for entry in &removed {
                let statement = &entry.statement;
                ui.push_id(("removed_statement", statement.id), |ui| {
                    card().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} – {}", short_date(&statement.start), short_date(&statement.end))).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Restore").clicked() { action = Some((statement.id, true)); }
                            });
                        });
                    });
                });
                ui.add_space(10.0);
            }
        });
        if let Some((id, restore)) = action {
            self.edit_history(|ledger| {
                if restore {
                    ledger.restore_statement(id)
                } else {
                    ledger.remove_statement(id)
                }
            });
        }
    }

    fn overview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Manage statements").clicked() {
                self.page = Page::Statements;
            }
        });
        ui.add_space(10.0);
        let latest = self.ledger.latest_statement().cloned();
        if latest.is_none() {
            self.empty(ui);
            return;
        }
        let scope = self.overview_statement;
        let account = self.ledger.account_summary();
        let names = self
            .ledger
            .people
            .each_ref()
            .map(|person| person.name.clone());
        let s = scope
            .map(|id| self.ledger.summary(id))
            .unwrap_or_else(|| account.summary.clone());
        let statement = scope
            .and_then(|id| self.ledger.statement(id))
            .cloned()
            .or_else(|| latest.clone());
        let mut statements = self.ledger.statements.clone();
        statements.sort_by(|a, b| a.start.cmp(&b.start));
        egui::ScrollArea::vertical().id_salt(("overview", scope)).show(ui, |ui| {
            if scope.is_none() {
                let caption = format!("All statements · {} – {} · {} imported", short_date(&statements[0].start), short_date(&latest.as_ref().unwrap().end), statements.len());
                ui.label(RichText::new(caption).color(MUTED).size(12.0));
                ui.add_space(10.0);
                for warning in &account.warnings {
                    banner(ui, "Account history needs a check", warning, AMBER);
                }
            }
            if s.unassigned_count > 0 || s.unassigned_payment_count > 0 {
                banner(ui, "Finish assigning to see your final shares", &format!("{} spending/balance items and {} repayments need attribution. The figures below are provisional.", s.unassigned_count, s.unassigned_payment_count), AMBER);
                ui.horizontal(|ui| {
                    if ui.add_enabled(s.unassigned_count > 0, egui::Button::new("Review unassigned spending")).clicked() {
                        let next = if let Some(id) = scope { Some(id) } else { self.ledger.account_transactions().find(|t| t.split == Split::Unassigned).map(|t| t.statement_id) };
                        if let Some(id) = next { self.statement = Some(id); self.page = Page::Transactions; self.filter = 1; self.search.clear(); self.card_filter = "All cards".into(); self.selected.clear(); }
                    }
                    if ui.add_enabled(s.unassigned_payment_count > 0, egui::Button::new("Attribute repayments")).clicked() {
                        self.statement = scope.or_else(|| self.ledger.payments.iter().find(|p| p.route == Route::Amex && p.split == Split::Unassigned).map(|p| p.statement_id));
                        self.page = Page::Payments;
                    }
                }); ui.add_space(16.0);
            }
            ui.columns(2, |cols| {
                self.person_card(&mut cols[0], &names[0], 0, &s);
                self.person_card(&mut cols[1], &names[1], 1, &s);
            });
            ui.add_space(18.0);
            card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| { eyebrow(ui, "AMEX ACCOUNT"); ui.label(RichText::new(if s.bank_remaining < 0 { "Account credit" } else { "Remaining balance" }).size(18.0).strong()); });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(RichText::new(money(s.bank_remaining.abs())).size(30.0).strong().color(GREEN)); });
                });
                ui.add_space(12.0); ui.separator(); ui.add_space(8.0);
                ui.columns(4, |cols| {
                    mini(&mut cols[0], if scope.is_none() { "Starting balance" } else { "Brought forward" }, &money(s.opening));
                    mini(&mut cols[1], "New purchases", &money(s.purchases));
                    mini(&mut cols[2], "Less refunds", &money(s.refunds));
                    mini(&mut cols[3], "Paid to Amex", &money(s.bank_paid));
                });
                if scope.is_none() && account.balance_adjustment != 0 {
                    kv(ui, "Unallocated activity between statements", &money(account.balance_adjustment));
                }
                ui.add_space(12.0);
                ui.label(RichText::new("Transfers between you settle your contributions. They don't reduce the Amex balance a second time.").size(12.0).color(MUTED));
            });
            ui.add_space(18.0);
            ui.columns(2, |cols| {
                card().show(&mut cols[0], |ui| {
                    ui.set_width(ui.available_width()); eyebrow(ui, if scope.is_none() { "ACCOUNT HISTORY" } else { "STATEMENT CHECK" });
                    if let Some(statement) = &statement {
                        kv(ui, if scope.is_none() { "Latest statement closing balance" } else { "Statement closing balance" }, &money(statement.closing));
                        kv(ui, if scope.is_none() { "Latest payment due" } else { "Payment due" }, &statement.due.as_deref().map(short_date).unwrap_or_else(|| "Not provided".into()));
                        if let Some(path) = &statement.source && ui.button(if scope.is_none() { "Open latest statement" } else { "Open original statement" }).clicked() { open_path(&PathBuf::from(path)); }
                    } else { ui.label(RichText::new("No active statements. Import your next statement when it arrives.").color(MUTED)); }
                    let manual: i64 = scope.map(|id| self.ledger.payments.iter().filter(|p| p.statement_id == id && !p.imported && p.route == Route::Amex).map(|p| p.amount).sum()).unwrap_or(account.extra_bank_paid);
                    kv(ui, "Additional payments recorded", &money(manual));
                    ui.add_space(8.0);
                    ui.label(RichText::new(if scope.is_some() { "Historical statement view: includes its own opening and repayments. Choose All statements for your current balance." } else { "Includes the first opening balance and all later spending, refunds and payments. Later brought-forward balances are counted once." }).size(12.0).color(MUTED));
                    if scope.is_none() && account.matched_payments > 0 {
                        ui.label(RichText::new(format!("{} recorded payments matched to statement entries and counted once.", account.matched_payments)).size(12.0).color(GREEN));
                    }
                });
                card().show(&mut cols[1], |ui| {
                    ui.set_width(ui.available_width()); eyebrow(ui, "RECEIPTS & EXPENSES");
                    let receipts = self.ledger.transactions.iter().filter(|t| scope.is_none_or(|id| t.statement_id == id) && t.receipt.is_some()).count();
                    let expense = self.ledger.transactions.iter().filter(|t| scope.is_none_or(|id| t.statement_id == id) && t.expense).count();
                    kv(ui, "Saved receipts", &receipts.to_string()); kv(ui, "Flagged to expense", &expense.to_string());
                    ui.add_space(8.0);
                    ui.label(RichText::new("Receipts are optional. Open any transaction to attach a photo or PDF and leave a note.").color(MUTED).size(12.0));
                    if ui.button("Export transactions as CSV").clicked() { self.export_csv(); }
                    if ui.button("Back up all data").clicked() { self.export_backup(); }
                });
            });
            if scope.is_none() && statements.len() > 1 {
                ui.add_space(18.0);
                card().show(ui, |ui| {
                    ui.set_width(ui.available_width()); eyebrow(ui, "IMPORTED STATEMENTS");
                    for statement in &statements {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} – {}", short_date(&statement.start), short_date(&statement.end)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("View statement").clicked() { self.overview_statement = Some(statement.id); self.statement = Some(statement.id); }
                            });
                        });
                    }
                });
            }
        });
    }
    fn person_card(&mut self, ui: &mut egui::Ui, name: &str, n: usize, s: &Summary) {
        let background = ui.painter().add(egui::Shape::Noop);
        let response = Frame::new()
            .stroke(Stroke::new(1.0, Color32::from_rgb(48, 87, 135)))
            .corner_radius(18)
            .inner_margin(22)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(name)
                            .size(23.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        badge(
                            ui,
                            if !s.complete() {
                                "IN PROGRESS"
                            } else if s.remaining[n] <= 0 {
                                "SETTLED"
                            } else {
                                "TO CONTRIBUTE"
                            },
                            if !s.complete() { AMBER } else { GREEN },
                        );
                    });
                });
                ui.add_space(22.0);
                ui.label(
                    RichText::new(if s.remaining[n] < 0 {
                        "Credit / owed back"
                    } else {
                        "Remaining contribution"
                    })
                    .size(12.0)
                    .color(PALE_BLUE),
                );
                ui.label(
                    RichText::new(money(s.remaining[n].abs()))
                        .size(45.0)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(18.0);
                ui.columns(2, |cols| {
                    cols[0].label(RichText::new("Allocated share").size(11.0).color(PALE_BLUE));
                    cols[0].label(
                        RichText::new(money(s.shares[n]))
                            .size(17.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    cols[1].label(
                        RichText::new("Already contributed")
                            .size(11.0)
                            .color(PALE_BLUE),
                    );
                    cols[1].label(
                        RichText::new(money(s.contributions[n]))
                            .size(17.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                });
                ui.add_space(20.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 36.0],
                        egui::Button::new(
                            RichText::new(format!("Record {name}'s payment"))
                                .size(12.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgba_unmultiplied(83, 151, 235, 36))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(65, 112, 166)))
                        .corner_radius(8),
                    )
                    .clicked()
                {
                    self.payment_form = Some(PaymentForm {
                        statement_id: self
                            .overview_statement
                            .or_else(|| self.ledger.latest_statement().map(|s| s.id)),
                        split: if n == 0 {
                            Split::PersonOne
                        } else {
                            Split::PersonTwo
                        },
                        amount: if s.remaining[n] > 0 {
                            decimal(s.remaining[n])
                        } else {
                            String::new()
                        },
                        ..Default::default()
                    });
                }
            });
        ui.painter().set(
            background,
            balance_card_background(response.response.rect, n),
        );
    }
    fn empty(&mut self, ui: &mut egui::Ui) {
        ui.add_space(65.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width()); ui.add_space(20.0);
            ui.label(RichText::new("Make sense of your next statement.").size(28.0).strong());
            ui.label(RichText::new("Import an Amex PDF, assign spending and refunds, then tell us whose repayments are whose.").color(MUTED));
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                if primary(ui, "Choose a statement").clicked() { self.pick_import(); }
                if ui.button("Explore with demo data").clicked() && let Err(e) = self.switch_workspace(true) { self.tell(e.to_string(), true); }
            });
            ui.add_space(20.0); ui.label(RichText::new("PDF + CSV import   ·   Optional receipts   ·   Partial payments   ·   No account or internet needed").color(MUTED).size(12.0)); ui.add_space(20.0);
        });
    }

    fn transactions(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.statement else {
            self.empty(ui);
            return;
        };
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search merchant, cardholder or note…")
                    .desired_width(265.0),
            );
            let filters = [
                "All items".into(),
                "Unassigned".into(),
                "Joint".into(),
                format!("{} only", self.ledger.people[0].name),
                format!("{} only", self.ledger.people[1].name),
                "Refunds".into(),
                "To expense".into(),
                "With receipt".into(),
            ];
            egui::ComboBox::from_id_salt("filter")
                .selected_text(&filters[self.filter])
                .show_ui(ui, |ui| {
                    for (i, label) in filters.iter().enumerate() {
                        ui.selectable_value(&mut self.filter, i, label);
                    }
                });
            let owners: BTreeSet<_> = self
                .ledger
                .transactions
                .iter()
                .filter(|t| t.statement_id == id)
                .map(|t| t.cardholder.clone())
                .collect();
            egui::ComboBox::from_id_salt("card")
                .selected_text(&self.card_filter)
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.card_filter, "All cards".into(), "All cards");
                    for owner in owners {
                        ui.selectable_value(
                            &mut self.card_filter,
                            owner.clone(),
                            owner_label(&owner, &self.ledger.people),
                        );
                    }
                });
        });
        ui.add_space(8.0);
        let query = self.search.to_lowercase();
        let rows: Vec<_> = self
            .ledger
            .transactions
            .iter()
            .filter(|t| t.statement_id == id)
            .filter(|t| {
                query.is_empty()
                    || format!("{} {} {}", t.merchant, t.cardholder, t.note)
                        .to_lowercase()
                        .contains(&query)
            })
            .filter(|t| self.card_filter == "All cards" || t.cardholder == self.card_filter)
            .filter(|t| match self.filter {
                1 => t.split == Split::Unassigned,
                2 => t.split == Split::Joint,
                3 => t.split == Split::PersonOne,
                4 => t.split == Split::PersonTwo,
                5 => t.kind == Kind::Refund,
                6 => t.expense,
                7 => t.receipt.is_some(),
                _ => true,
            })
            .cloned()
            .collect();
        let visible_total: i64 = rows.iter().map(|t| t.amount).sum();
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            ui.label(
                RichText::new(format!(
                    "{} items · {} net",
                    rows.len(),
                    money(visible_total)
                ))
                .color(MUTED)
                .size(12.0),
            );
            if ui.small_button("Select shown").clicked() {
                self.selected.extend(rows.iter().map(|t| t.id));
            }
            if !self.selected.is_empty() {
                ui.label(format!("{} selected", self.selected.len()));
                for split in [
                    Split::PersonOne,
                    Split::PersonTwo,
                    Split::Joint,
                    Split::Unassigned,
                ] {
                    if ui.small_button(split.label(&self.ledger.people)).clicked() {
                        let ids = self.selected.iter().copied().collect::<Vec<_>>();
                        self.set_split(&ids, split);
                    }
                }
                if ui.small_button("Clear selection").clicked() {
                    self.selected.clear();
                }
            }
        });
        ui.add_space(8.0);
        let mut action: Option<(u64, Split)> = None;
        let mut open = None;
        TableBuilder::new(ui)
            .id_salt(("transactions", id))
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(25.0).clip(true))
            .column(Column::exact(65.0).clip(true))
            .column(Column::remainder().at_least(160.0).clip(true))
            .column(Column::exact(75.0).clip(true))
            .column(Column::exact(95.0).clip(true))
            .column(Column::exact(208.0).clip(true))
            .header(30.0, |mut row| {
                for name in ["", "DATE", "TRANSACTION", "CARD", "AMOUNT", "WHOSE SHARE?"] {
                    row.col(|ui| {
                        ui.label(RichText::new(name).size(10.0).color(MUTED).strong());
                    });
                }
            })
            .body(|body| {
                body.rows(60.0, rows.len(), |mut row| {
                    let t = &rows[row.index()];
                    row.col(|ui| {
                        let mut selected = self.selected.contains(&t.id);
                        if ui.checkbox(&mut selected, "").changed() {
                            if selected {
                                self.selected.insert(t.id);
                            } else {
                                self.selected.remove(&t.id);
                            }
                        }
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(compact_date(&t.date)).size(12.0))
                            .on_hover_text(&t.date);
                    });
                    row.col(|ui| {
                        ui.vertical(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(&t.merchant).size(13.0).strong(),
                                    )
                                    .frame(false)
                                    .truncate(),
                                )
                                .on_hover_text(format!(
                                    "{}\nOpen details, receipt and custom split",
                                    t.merchant
                                ))
                                .clicked()
                            {
                                open = Some(t.id);
                            }
                            let mut labels = vec![];
                            if t.kind == Kind::Refund {
                                labels.push("REFUND");
                            }
                            if t.kind == Kind::Opening {
                                labels.push("BALANCE BROUGHT FORWARD");
                            }
                            if t.receipt.is_some() {
                                labels.push("RECEIPT");
                            }
                            if t.expense {
                                labels.push("TO EXPENSE");
                            }
                            if !labels.is_empty() {
                                ui.label(
                                    RichText::new(labels.join("  ·  "))
                                        .size(9.0)
                                        .color(if t.kind == Kind::Refund { GREEN } else { MUTED }),
                                );
                            }
                        });
                    });
                    row.col(|ui| {
                        ui.label(
                            RichText::new(owner_label(&t.cardholder, &self.ledger.people))
                                .size(12.0)
                                .color(MUTED),
                        )
                        .on_hover_text(&t.cardholder);
                    });
                    row.col(|ui| {
                        ui.label(
                            RichText::new(money(t.amount))
                                .strong()
                                .color(if t.amount < 0 { GREEN } else { INK }),
                        );
                    });
                    row.col(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.spacing_mut().button_padding = Vec2::new(7.0, 5.0);
                        ui.horizontal(|ui| {
                            for (split, label) in [
                                (Split::PersonOne, self.ledger.people[0].name.as_str()),
                                (Split::PersonTwo, self.ledger.people[1].name.as_str()),
                                (Split::Joint, "Joint"),
                            ] {
                                let active = t.split == split;
                                if ui
                                    .add_sized(
                                        [46.0, 28.0],
                                        egui::Button::new(
                                            RichText::new(label).size(11.0).color(if active {
                                                Color32::WHITE
                                            } else {
                                                INK
                                            }),
                                        )
                                        .fill(if active { BLUE } else { ELEVATED })
                                        .min_size(Vec2::new(46.0, 28.0))
                                        .truncate(),
                                    )
                                    .on_hover_text(if active {
                                        "Click again to leave unassigned"
                                    } else {
                                        "Assign this transaction"
                                    })
                                    .clicked()
                                {
                                    action = Some((
                                        t.id,
                                        if active { Split::Unassigned } else { split },
                                    ));
                                }
                            }
                            if ui
                                .small_button("…")
                                .on_hover_text(t.split.label(&self.ledger.people))
                                .clicked()
                            {
                                open = Some(t.id);
                            }
                        });
                    });
                });
            });
        if let Some((id, split)) = action {
            self.set_split(&[id], split);
        }
        if let Some(id) = open {
            self.open_editor(id);
        }
    }

    fn details(&mut self, ui: &mut egui::Ui) {
        let Some(mut editor) = self.editor.clone() else {
            return;
        };
        let Some(t) = self
            .ledger
            .transactions
            .iter()
            .find(|t| t.id == editor.id)
            .cloned()
        else {
            self.editor = None;
            return;
        };
        let mut close = false;
        egui::Window::new("Transaction details").anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).default_width(340.0).resizable(false).collapsible(false).frame(card()).show(ui.ctx(), |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| { eyebrow(ui, "TRANSACTION DETAILS"); if ui.small_button("Close").clicked() { close = true; } });
                ui.add_space(12.0); ui.label(RichText::new(&t.merchant).size(19.0).strong());
                ui.label(RichText::new(money(t.amount)).size(28.0).color(if t.amount < 0 { GREEN } else { INK }));
                ui.label(RichText::new(format!("{} · {}", short_date(&t.date), owner_label(&t.cardholder, &self.ledger.people))).color(MUTED));
                ui.add_space(16.0); ui.separator();
                if t.kind == Kind::Refund { ui.label(RichText::new("This refund reduces the assigned share. For a custom split, enter a negative amount for the second person's credit.").color(GREEN)); }
                ui.label("Allocate to"); split_combo(ui, "detail_split", &mut editor.split, &self.ledger.people);
                if editor.split == Split::Custom { ui.label(format!("{}'s share (£)", self.ledger.people[1].name)); ui.text_edit_singleline(&mut editor.second_share); ui.label(RichText::new(format!("{} receives the remainder.", self.ledger.people[0].name)).size(12.0).color(MUTED)); }
                ui.add_space(10.0); ui.checkbox(&mut editor.expense, "To expense later");
                ui.label("Note"); ui.add(egui::TextEdit::multiline(&mut editor.note).desired_rows(3).desired_width(f32::INFINITY));
                if primary(ui, "Save details").clicked() {
                    let custom = if editor.split == Split::Custom { parse_money(&editor.second_share).and_then(|a| { validate_custom(t.amount, a)?; Ok(a) }) } else { Ok(0) };
                    match custom { Ok(second_share) => {
                        let e = editor.clone(); self.change(|l| if let Some(t) = l.transactions.iter_mut().find(|t| t.id == e.id) { t.split = e.split; t.custom_person_two = second_share; t.note = e.note; t.expense = e.expense; }); close = true;
                    }, Err(e) => self.tell(e.to_string(), true) }
                }
                ui.add_space(16.0); ui.separator(); ui.label(RichText::new("Receipt").strong());
                if let Some(path) = &t.receipt {
                    ui.label(RichText::new("Saved with this transaction").color(GREEN));
                    if ui.button("Open receipt").clicked() { open_path(&PathBuf::from(path)); }
                    if ui.button("Save a copy…").clicked() && let Some(dest) = rfd::FileDialog::new().set_file_name(PathBuf::from(path).file_name().unwrap().to_string_lossy()).save_file()
                        && let Err(e) = std::fs::copy(path, dest) { self.tell(e.to_string(), true); }
                } else { ui.label(RichText::new("No receipt attached. That's fine.").size(12.0).color(MUTED)); }
                if ui.button(if t.receipt.is_some() { "Replace receipt…" } else { "Attach photo or PDF…" }).clicked()
                    && let Some(file) = rfd::FileDialog::new().add_filter("Receipt", &["jpg", "jpeg", "png", "heic", "pdf", "webp"]).pick_file() {
                        let key = format!("{}-{}", t.id, chrono::Local::now().timestamp_millis());
                        match self.store.copy_attachment(&file, "receipts", &key) {
                            Ok(path) => self.change(|l| if let Some(t) = l.transactions.iter_mut().find(|x| x.id == t.id) { t.receipt = Some(path); }),
                            Err(e) => self.tell(e.to_string(), true),
                        }
                }
            });
        });
        self.editor = if close { None } else { Some(editor) };
    }

    fn payments(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.statement else {
            self.empty(ui);
            return;
        };
        let s = self.ledger.summary(id);
        let rows: Vec<_> = self
            .ledger
            .payments
            .iter()
            .filter(|p| p.statement_id == id)
            .cloned()
            .collect();
        if s.unassigned_payment_count > 0 {
            banner(
                ui,
                "Whose repayments were these?",
                &format!(
                    "{} payments totalling {} were found on your statement. Choose who contributed each amount. These already reduce the bank balance.",
                    s.unassigned_payment_count,
                    money(s.unassigned_payments)
                ),
                AMBER,
            );
        }
        ui.horizontal(|ui| {
            if primary(ui, "+ Record payment / transfer").clicked() {
                self.payment_form = Some(PaymentForm {
                    statement_id: Some(id),
                    ..Default::default()
                });
            }
            ui.label(
                RichText::new("Partial and early payments are welcome.")
                    .color(MUTED)
                    .size(12.0),
            );
        });
        ui.add_space(12.0);
        let mut assign = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for p in &rows {
                card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(money(p.amount)).size(21.0).strong());
                            ui.label(
                                RichText::new(format!(
                                    "{} · {}",
                                    short_date(&p.date),
                                    p.route.label(&self.ledger.people)
                                ))
                                .color(MUTED)
                                .size(12.0),
                            );
                            ui.label(
                                RichText::new(if p.imported {
                                    "From statement · already included"
                                } else {
                                    "Recorded in this app"
                                })
                                .color(MUTED)
                                .size(10.0),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !p.imported && ui.small_button("Delete").clicked() {
                                self.delete_payment = Some(p.id);
                            }
                            if ui.button("Edit / custom split").clicked() {
                                self.payment_form = Some(PaymentForm {
                                    id: Some(p.id),
                                    statement_id: Some(p.statement_id),
                                    amount: decimal(p.amount),
                                    date: p.date.clone(),
                                    split: p.split,
                                    second_share: decimal(p.custom_person_two),
                                    route: p.route,
                                    note: p.note.clone(),
                                    error: String::new(),
                                });
                            }
                            if p.route == Route::Amex {
                                let mut split = p.split;
                                egui::ComboBox::from_id_salt(("payment_split", p.id))
                                    .selected_text(p.split.label(&self.ledger.people))
                                    .show_ui(ui, |ui| {
                                        for option in [
                                            Split::Unassigned,
                                            Split::PersonOne,
                                            Split::PersonTwo,
                                            Split::Joint,
                                        ] {
                                            ui.selectable_value(
                                                &mut split,
                                                option,
                                                option.label(&self.ledger.people),
                                            );
                                        }
                                    });
                                if split != p.split {
                                    assign = Some((p.id, split));
                                }
                                ui.label(RichText::new("Contribution:").color(MUTED));
                            }
                        });
                    });
                    if !p.note.is_empty() {
                        ui.label(RichText::new(&p.note).size(12.0).color(MUTED));
                    }
                });
                ui.add_space(8.0);
            }
            if rows.is_empty() {
                ui.label("No payments recorded for this statement yet.");
            }
        });
        if let Some((id, split)) = assign {
            self.change(|l| {
                if let Some(p) = l.payments.iter_mut().find(|p| p.id == id) {
                    p.split = split;
                }
            });
        }
    }

    fn payment_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.payment_form.take() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let existing =
            f.id.and_then(|id| self.ledger.payments.iter().find(|p| p.id == id))
                .cloned();
        let imported = existing.as_ref().is_some_and(|p| p.imported);
        if f.statement_id.is_none() {
            f.statement_id = existing.as_ref().map(|p| p.statement_id).or(self.statement);
        }
        egui::Window::new(if imported { "Attribute statement repayment" } else { "Record a contribution" }).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).open(&mut open).collapsible(false).resizable(false).default_width(420.0).show(ctx, |ui| {
            if imported { ui.label(RichText::new("This payment already appears on the statement. Only its attribution and note can change.").color(MUTED)); }
            ui.add_enabled_ui(!imported, |ui| {
                ui.label("Record against");
                let label = f.statement_id.and_then(|id| self.ledger.statement(id)).map(|s| format!("{} – {}", short_date(&s.start), short_date(&s.end))).unwrap_or_else(|| "Choose statement".into());
                egui::ComboBox::from_id_salt("payment_statement").selected_text(label).show_ui(ui, |ui| {
                    for statement in &self.ledger.statements { ui.selectable_value(&mut f.statement_id, Some(statement.id), format!("{} – {}", short_date(&statement.start), short_date(&statement.end))); }
                });
                ui.label("Payment or transfer");
                egui::ComboBox::from_id_salt("route").selected_text(f.route.label(&self.ledger.people)).show_ui(ui, |ui| { for r in Route::ALL { ui.selectable_value(&mut f.route, r, r.label(&self.ledger.people)); } });
                ui.horizontal(|ui| { ui.vertical(|ui| { ui.label("Amount (£)"); ui.add(egui::TextEdit::singleline(&mut f.amount).desired_width(170.0)); }); ui.vertical(|ui| { ui.label("Date (YYYY-MM-DD)"); ui.add(egui::TextEdit::singleline(&mut f.date).desired_width(170.0)); }); });
            });
            if f.route == Route::Amex {
                ui.label("Whose contribution?"); split_combo(ui, "form_split", &mut f.split, &self.ledger.people);
                if f.split == Split::Custom { ui.label(format!("{} contributed (£)", self.ledger.people[1].name)); ui.text_edit_singleline(&mut f.second_share); ui.label(RichText::new(format!("The remainder is attributed to {}.", self.ledger.people[0].name)).size(12.0).color(MUTED)); }
            } else { ui.label(RichText::new("A transfer settles what you owe each other. It does not count as another payment to Amex.").size(12.0).color(MUTED)); }
            ui.label("Note (optional)"); ui.add(egui::TextEdit::multiline(&mut f.note).desired_rows(2).desired_width(f32::INFINITY));
            if f.id.is_none() && let (Ok(amount), Ok(date)) = (parse_money(&f.amount), import::date(&f.date))
                && self.ledger.payments.iter().any(|p| Some(p.statement_id) == f.statement_id && p.route == f.route && p.date == date && p.amount == amount) {
                ui.label(RichText::new("Another payment has this date and amount. This will be saved as a separate payment.").size(12.0).color(MUTED));
            }
            if !f.error.is_empty() { ui.colored_label(RED, &f.error); }
            if primary(ui, if imported { "Save attribution" } else { "Save contribution" }).clicked() { save = true; }
        });
        if save {
            let result = (|| -> Result<()> {
                let amount = parse_money(&f.amount)?;
                if amount <= 0 {
                    bail!("Enter a payment greater than zero");
                }
                let date = import::date(&f.date)?;
                let second_share = if f.split == Split::Custom {
                    let a = parse_money(&f.second_share)?;
                    validate_custom(amount, a)?;
                    a
                } else {
                    0
                };
                let sid = f.statement_id.context("Choose a statement first")?;
                self.change(|l| {
                    if let Some(id) = f.id {
                        if let Some(p) = l.payments.iter_mut().find(|p| p.id == id) {
                            p.split = f.split;
                            p.custom_person_two = second_share;
                            p.note = f.note.clone();
                            if !p.imported {
                                p.statement_id = sid;
                                p.amount = amount;
                                p.date = date.clone();
                                p.route = f.route;
                            }
                        }
                    } else {
                        let id = l.id();
                        l.payments.push(Payment {
                            id,
                            statement_id: sid,
                            fingerprint: None,
                            date,
                            amount,
                            split: f.split,
                            custom_person_two: second_share,
                            route: f.route,
                            note: f.note.clone(),
                            imported: false,
                        });
                    }
                });
                Ok(())
            })();
            match result {
                Ok(()) => open = false,
                Err(e) => f.error = e.to_string(),
            }
        }
        if open {
            self.payment_form = Some(f);
        }
    }

    fn people_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut people) = self.people_form.take() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        egui::Window::new("Settings").anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .open(&mut open).resizable(false).collapsible(false).default_width(520.0).show(ctx, |ui| {
                ui.label("Set the names used on balances, buttons, transfers and exports.");
                ui.label(RichText::new("Changing names keeps every existing allocation and payment attached to the same person.").color(MUTED).size(12.0));
                ui.add_space(12.0);
                for (index, person) in people.iter_mut().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.label(RichText::new(format!("Person {}", index + 1)).strong());
                        ui.label("Display name");
                        ui.add(egui::TextEdit::singleline(&mut person.name).desired_width(f32::INFINITY));
                        ui.label("Card member names on statements (optional, one per line)");
                        let mut aliases = person.cardholder_names.join("\n");
                        if ui.add(egui::TextEdit::multiline(&mut aliases).desired_rows(2).desired_width(f32::INFINITY)).changed() {
                            person.cardholder_names = aliases.lines().map(str::to_owned).collect();
                        }
                    });
                    ui.add_space(12.0);
                }
                ui.label(RichText::new("Card member names shorten the Card column. They do not assign spending automatically. Keyboard shortcuts: 1, 2 and J for joint.").size(12.0).color(MUTED));
                if !self.people_error.is_empty() { ui.colored_label(RED, &self.people_error); }
                if primary(ui, "Save settings").clicked() { save = true; }
            });
        if save {
            for person in &mut people {
                person.name = person.name.trim().to_owned();
                person.cardholder_names = person
                    .cardholder_names
                    .iter()
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty())
                    .collect();
            }
            match validate_people(&people) {
                Ok(()) => {
                    let updated = people.clone();
                    self.change(|ledger| ledger.people = updated);
                    open = false;
                }
                Err(error) => self.people_error = error.to_string(),
            }
        }
        if open {
            self.people_form = Some(people);
        }
    }

    fn import_page(&mut self, ui: &mut egui::Ui) {
        if self.loading.is_some() {
            ui.add_space(60.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Reading your statement and checking every total…");
            });
            return;
        }
        let Some(p) = self.preview.clone() else {
            card().show(ui, |ui| {
                ui.set_width(ui.available_width()); ui.add_space(25.0);
                ui.label(RichText::new("Drop in your Amex statement.").size(25.0).strong());
                ui.label(RichText::new("Drag a downloaded UK Amex PDF into this window, or choose a PDF / CSV below.").color(MUTED));
                ui.add_space(20.0); if primary(ui, "Choose PDF or CSV").clicked() { self.pick_import(); }
                ui.add_space(20.0); ui.separator(); ui.add_space(10.0);
                ui.label("What happens next");
                for text in ["1. Preview the purchases, refunds, cardholders and repayments.", "2. Confirm that the extracted amounts match the statement.", "3. Import, then allocate spending and attribute repayments."] { ui.label(RichText::new(text).color(MUTED)); }
                ui.add_space(15.0); ui.label(RichText::new("Your original file is copied into the local app folder. Re-importing the same file opens the existing statement and preserves your edits.").size(12.0).color(MUTED));
            });
            return;
        };
        let duplicate = self
            .ledger
            .statements
            .iter()
            .find(|s| s.digest == p.digest)
            .map(|s| s.id);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(p.path.file_name().unwrap_or_default().to_string_lossy()).strong(),
            );
            if ui.button("Choose another").clicked() {
                self.pick_import();
            }
            if !p.errors.is_empty() {
                badge(ui, "TOTALS DO NOT MATCH", RED);
            } else {
                badge(
                    ui,
                    if p.verified {
                        "ALL TOTALS VERIFIED"
                    } else {
                        "REVIEW CSV TOTALS"
                    },
                    if p.verified { GREEN } else { AMBER },
                );
            }
        });
        ui.add_space(12.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.columns(4, |cols| {
                mini(&mut cols[0], "Purchases", &money(p.debits));
                mini(&mut cols[1], "Refunds", &money(p.refunds()));
                mini(&mut cols[2], "Repayments", &money(p.payments()));
                mini(&mut cols[3], "Closing balance", &money(p.closing));
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "{} – {} · {} transactions · Opening balance {}",
                    short_date(&p.start),
                    short_date(&p.end),
                    p.records.len(),
                    money(p.previous)
                ))
                .color(MUTED),
            );
        });
        for error in &p.errors {
            ui.colored_label(RED, error);
        }
        for warning in &p.warnings {
            ui.colored_label(AMBER, warning);
        }
        if !p.verified {
            ui.horizontal(|ui| {
                ui.label("Period start");
                ui.add(egui::TextEdit::singleline(&mut self.import_start).desired_width(100.0));
                ui.label("Period end");
                ui.add(egui::TextEdit::singleline(&mut self.import_end).desired_width(100.0));
                ui.label("Opening balance (£)");
                ui.add(egui::TextEdit::singleline(&mut self.import_opening).desired_width(100.0));
            });
            ui.checkbox(
                &mut self.csv_confirmed,
                "I checked the date range, opening balance and signed amounts",
            );
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let ready = p.errors.is_empty() && (p.verified || self.csv_confirmed);
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new(
                        RichText::new(if duplicate.is_some() {
                            "Open existing statement"
                        } else {
                            "Import & attribute repayments"
                        })
                        .color(Color32::WHITE),
                    )
                    .fill(BLUE),
                )
                .clicked()
            {
                if let Some(id) = duplicate {
                    self.statement = Some(id);
                    self.overview_statement = None;
                    self.page = Page::Overview;
                    self.preview = None;
                } else {
                    let mut p = p.clone();
                    let result = (|| -> Result<u64> {
                        if !p.verified {
                            p.start = import::date(&self.import_start)?;
                            p.end = import::date(&self.import_end)?;
                            if p.start > p.end {
                                bail!("Period end must be on or after its start");
                            }
                            p.previous = parse_money(&self.import_opening)?;
                            p.closing = p.previous + p.debits - p.credits;
                        }
                        self.store.import(&mut self.ledger, &p)
                    })();
                    match result {
                        Ok(id) => {
                            self.statement = Some(id);
                            self.overview_statement = None;
                            self.page = if p.records.iter().any(|r| r.payment) {
                                Page::Payments
                            } else {
                                Page::Transactions
                            };
                            self.preview = None;
                            self.filter = 0;
                            self.card_filter = "All cards".into();
                            self.search.clear();
                            self.undo = None;
                            self.tell("Statement imported. Your original is saved locally.", false);
                        }
                        Err(e) => self.tell(e.to_string(), true),
                    }
                }
            }
            ui.label(
                RichText::new("New items start unassigned. No personal/joint guesses.")
                    .color(MUTED)
                    .size(12.0),
            );
        });
        ui.add_space(10.0);
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::exact(70.0).clip(true))
            .column(Column::remainder().clip(true))
            .column(Column::exact(90.0).clip(true))
            .column(Column::exact(100.0).clip(true))
            .column(Column::exact(90.0).clip(true))
            .header(28.0, |mut row| {
                for h in ["DATE", "DESCRIPTION", "CARD", "AMOUNT", "TYPE"] {
                    row.col(|ui| {
                        eyebrow(ui, h);
                    });
                }
            })
            .body(|body| {
                body.rows(30.0, p.records.len(), |mut row| {
                    let r = &p.records[row.index()];
                    row.col(|ui| {
                        ui.label(compact_date(&r.date)).on_hover_text(&r.date);
                    });
                    row.col(|ui| {
                        ui.label(&r.merchant);
                    });
                    row.col(|ui| {
                        ui.label(owner_label(&r.cardholder, &self.ledger.people));
                    });
                    row.col(|ui| {
                        ui.colored_label(if r.amount < 0 { GREEN } else { INK }, money(r.amount));
                    });
                    row.col(|ui| {
                        ui.label(if r.payment {
                            "Repayment"
                        } else if r.amount < 0 {
                            "Refund"
                        } else {
                            "Purchase"
                        });
                    });
                });
            });
    }

    fn export_csv(&mut self) {
        let scope = if self.page == Page::Overview {
            self.overview_statement
        } else {
            self.statement
        };
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("amex-reconciliation.csv")
            .add_filter("CSV", &["csv"])
            .save_file()
        {
            let result = (|| -> Result<()> {
                let mut csv = csv::Writer::from_path(path)?;
                csv.write_record([
                    "Date",
                    "Statement period",
                    "Description",
                    "Cardholder",
                    "Amount GBP",
                    "Allocation",
                    &format!("{} share GBP", self.ledger.people[0].name),
                    &format!("{} share GBP", self.ledger.people[1].name),
                    "To expense",
                    "Receipt",
                    "Note",
                ])?;
                for t in self
                    .ledger
                    .transactions
                    .iter()
                    .filter(|t| scope.is_none_or(|id| t.statement_id == id))
                {
                    let shares = t.split.shares(t.amount, t.custom_person_two);
                    csv.write_record([
                        t.date.clone(),
                        self.ledger
                            .statement(t.statement_id)
                            .map(|s| s.label.clone())
                            .unwrap_or_default(),
                        safe_csv(&t.merchant),
                        safe_csv(&t.cardholder),
                        decimal(t.amount),
                        t.split.label(&self.ledger.people).into(),
                        shares.map(|v| decimal(v[0])).unwrap_or_default(),
                        shares.map(|v| decimal(v[1])).unwrap_or_default(),
                        t.expense.to_string(),
                        t.receipt.clone().unwrap_or_default(),
                        safe_csv(&t.note),
                    ])?;
                }
                csv.flush()?;
                Ok(())
            })();
            match result {
                Ok(()) => self.tell("Reconciliation CSV exported", false),
                Err(e) => self.tell(e.to_string(), true),
            }
        }
    }
    fn export_backup(&mut self) {
        if let Some(folder) = rfd::FileDialog::new()
            .set_title("Choose a folder for a complete backup")
            .pick_folder()
        {
            let result = (|| -> Result<()> {
                let root = folder.join(format!(
                    "Amex Together backup {}",
                    chrono::Local::now().format("%Y-%m-%d-%H%M%S")
                ));
                std::fs::create_dir_all(&root)?;
                let mut l = self.ledger.clone();
                for s in l.statements.iter_mut().chain(
                    l.removed_statements
                        .iter_mut()
                        .map(|removed| &mut removed.statement),
                ) {
                    if let Some(path) = &s.source {
                        s.source = Some(backup_file(&root, path)?);
                    }
                }
                for t in l.transactions.iter_mut().chain(
                    l.removed_statements
                        .iter_mut()
                        .flat_map(|removed| &mut removed.transactions),
                ) {
                    if let Some(path) = &t.receipt {
                        t.receipt = Some(backup_file(&root, path)?);
                    }
                }
                std::fs::write(root.join("ledger.json"), serde_json::to_vec_pretty(&l)?)?;
                std::fs::write(
                    root.join("RESTORE.txt"),
                    "This backup contains all ledger data and attachments. Keep the folder together. To restore, run Amex Together with --restore followed by the path to ledger.json; it restores into a new local folder and does not overwrite your existing ledger.\n",
                )?;
                open_path(&root);
                Ok(())
            })();
            match result {
                Ok(()) => self.tell("Complete backup saved", false),
                Err(e) => self.tell(e.to_string(), true),
            }
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(result) => {
                    self.loading = None;
                    match result {
                        Ok(p) => {
                            self.import_start = p.start.clone();
                            self.import_end = p.end.clone();
                            self.import_opening = decimal(p.previous);
                            self.csv_confirmed = false;
                            self.preview = Some(p);
                        }
                        Err(e) => self.tell(e, true),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = None;
                    self.tell("The statement reader stopped unexpectedly", true);
                }
                _ => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_owned())
                .next()
        });
        if let Some(path) = dropped {
            self.start_import(path);
        }
        if !ctx.egui_wants_keyboard_input()
            && self.page == Page::Transactions
            && !self.selected.is_empty()
        {
            let split = ctx.input(|i| {
                if i.key_pressed(egui::Key::J) {
                    Some(Split::Joint)
                } else if i.key_pressed(egui::Key::Num1) {
                    Some(Split::PersonOne)
                } else if i.key_pressed(egui::Key::Num2) {
                    Some(Split::PersonTwo)
                } else {
                    None
                }
            });
            if let Some(split) = split {
                let ids = self.selected.iter().copied().collect::<Vec<_>>();
                self.set_split(&ids, split);
            }
        }
        self.sidebar(ui);
        self.status_footer(ui);
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(28, 26)),
            )
            .show(ui, |ui| {
                self.header(ui);
                match self.page {
                    Page::Overview => self.overview(ui),
                    Page::Transactions => self.transactions(ui),
                    Page::Payments => self.payments(ui),
                    Page::Import => self.import_page(ui),
                    Page::Statements => self.manage_statements(ui),
                }
            });
        self.details(ui);
        self.payment_dialog(&ctx);
        self.people_dialog(&ctx);
        if let Some(id) = self.delete_payment {
            egui::Window::new("Remove this recorded payment?")
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label(
                        "This removes the app record. No money is moved. You can undo this change.",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.delete_payment = None;
                        }
                        if ui.button("Remove record").clicked() {
                            self.change(|l| l.payments.retain(|p| p.id != id));
                            self.delete_payment = None;
                        }
                    });
                });
        }
    }
}

fn card() -> Frame {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(14)
        .inner_margin(20)
}

fn navigation_button(ui: &mut egui::Ui, page: Page, label: &str, active: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(170.0, 44.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let painter = ui.painter();
    if active || response.hovered() {
        painter.rect_filled(
            rect,
            9,
            if active {
                Color32::from_rgb(19, 46, 81)
            } else {
                SURFACE
            },
        );
    }
    if active {
        painter.rect_filled(
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 2.0, rect.center().y),
                Vec2::new(3.0, 18.0),
            ),
            2,
            BLUE,
        );
    }
    let color = if active { PALE_BLUE } else { MUTED };
    let origin = egui::pos2(rect.left() + 13.0, rect.center().y - 8.0);
    let point = |x, y| origin + Vec2::new(x, y);
    let stroke = Stroke::new(1.4, color);
    match page {
        Page::Overview => {
            for (x, y) in [(0.0, 0.0), (9.0, 0.0), (0.0, 9.0), (9.0, 9.0)] {
                painter.rect_stroke(
                    egui::Rect::from_min_size(point(x, y), Vec2::splat(6.0)),
                    1,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Page::Transactions => {
            for y in [2.0, 8.0, 14.0] {
                painter.circle_filled(point(1.0, y), 1.3, color);
                painter.line_segment([point(6.0, y), point(16.0, y)], stroke);
            }
        }
        Page::Payments => {
            painter.line_segment([point(0.0, 4.0), point(16.0, 4.0)], stroke);
            painter.line_segment([point(12.0, 0.0), point(16.0, 4.0)], stroke);
            painter.line_segment([point(12.0, 8.0), point(16.0, 4.0)], stroke);
            painter.line_segment([point(16.0, 12.0), point(0.0, 12.0)], stroke);
            painter.line_segment([point(4.0, 8.0), point(0.0, 12.0)], stroke);
            painter.line_segment([point(4.0, 16.0), point(0.0, 12.0)], stroke);
        }
        Page::Import => {
            painter.line_segment([point(8.0, 0.0), point(8.0, 11.0)], stroke);
            painter.line_segment([point(3.0, 6.0), point(8.0, 11.0)], stroke);
            painter.line_segment([point(13.0, 6.0), point(8.0, 11.0)], stroke);
            painter.line_segment([point(1.0, 11.0), point(1.0, 16.0)], stroke);
            painter.line_segment([point(1.0, 16.0), point(15.0, 16.0)], stroke);
            painter.line_segment([point(15.0, 16.0), point(15.0, 11.0)], stroke);
        }
        Page::Statements => {
            painter.rect_stroke(
                egui::Rect::from_min_size(point(1.0, 3.0), Vec2::new(14.0, 13.0)),
                2,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment([point(0.0, 2.0), point(7.0, 2.0)], stroke);
            painter.line_segment([point(4.0, 7.0), point(12.0, 7.0)], stroke);
            painter.line_segment([point(4.0, 11.0), point(12.0, 11.0)], stroke);
        }
    }
    painter.text(
        egui::pos2(rect.left() + 40.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        if active { INK } else { MUTED },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn balance_card_background(rect: egui::Rect, person: usize) -> egui::Shape {
    let (start, end) = if person == 0 {
        (
            Color32::from_rgb(21, 76, 133),
            Color32::from_rgb(14, 35, 65),
        )
    } else {
        (
            Color32::from_rgb(40, 58, 112),
            Color32::from_rgb(18, 32, 60),
        )
    };
    let color = |point: egui::Pos2| {
        let t = ((point.x - rect.left()) / rect.width() * 0.65
            + (point.y - rect.top()) / rect.height() * 0.35)
            .clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| (f32::from(a) * (1.0 - t) + f32::from(b) * t).round() as u8;
        Color32::from_rgb(
            blend(start.r(), end.r()),
            blend(start.g(), end.g()),
            blend(start.b(), end.b()),
        )
    };
    let mut mesh = egui::Mesh::default();
    let mut vertex = |point| {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: point,
            uv: egui::epaint::WHITE_UV,
            color: color(point),
        })
    };
    vertex(rect.center());
    let radius = 18.0;
    let corners = [
        (
            egui::pos2(rect.left() + radius, rect.top() + radius),
            std::f32::consts::PI,
        ),
        (
            egui::pos2(rect.right() - radius, rect.top() + radius),
            -std::f32::consts::FRAC_PI_2,
        ),
        (
            egui::pos2(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            egui::pos2(rect.left() + radius, rect.bottom() - radius),
            std::f32::consts::FRAC_PI_2,
        ),
    ];
    for (center, angle) in corners {
        for step in 0..=12 {
            let a = angle + step as f32 / 12.0 * std::f32::consts::FRAC_PI_2;
            vertex(center + Vec2::new(a.cos(), a.sin()) * radius);
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for i in 1..=count {
        mesh.indices
            .extend_from_slice(&[0, i, if i == count { 1 } else { i + 1 }]);
    }
    let mut shapes = vec![egui::Shape::mesh(mesh)];
    let center = egui::pos2(rect.right() - 106.0, rect.top() + 113.0);
    for radius in [40.0, 57.0, 74.0, 91.0] {
        shapes.push(egui::Shape::circle_stroke(
            center,
            radius,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(158, 208, 255, 14)),
        ));
    }
    egui::Shape::Vec(shapes)
}
fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(BLUE)
            .min_size(Vec2::new(0.0, 36.0)),
    )
}
fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).strong().color(MUTED));
}
fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    Frame::new()
        .fill(color.gamma_multiply(0.10))
        .corner_radius(5)
        .inner_margin(Margin::symmetric(7, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(9.0).strong().color(color));
        });
}
fn mini(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).size(12.0).color(MUTED));
    ui.label(RichText::new(value).size(21.0).strong());
}
fn kv(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).size(12.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(13.0).strong());
        });
    });
}
fn banner(ui: &mut egui::Ui, title: &str, body: &str, color: Color32) {
    Frame::new()
        .fill(Color32::from_rgb(29, 36, 47))
        .stroke(Stroke::new(1.0, Color32::from_rgb(76, 67, 50)))
        .corner_radius(9)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(color));
            ui.label(RichText::new(body).size(12.0).color(color));
        });
    ui.add_space(10.0);
}
fn split_combo(ui: &mut egui::Ui, id: &str, split: &mut Split, people: &[Person; 2]) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(split.label(people))
        .width(200.0)
        .show_ui(ui, |ui| {
            for s in Split::ALL {
                ui.selectable_value(split, s, s.label(people));
            }
        });
}
fn owner_label<'a>(owner: &'a str, people: &'a [Person; 2]) -> &'a str {
    let normalised = owner
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase();
    for person in people {
        if person.cardholder_names.iter().any(|alias| {
            alias
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_uppercase()
                == normalised
                && !normalised.is_empty()
        }) {
            return &person.name;
        }
    }
    if owner == "Opening balance" {
        "Opening"
    } else if owner.is_empty() {
        "—"
    } else {
        owner
    }
}
fn decimal(p: Pence) -> String {
    format!(
        "{}{}.{:02}",
        if p < 0 { "-" } else { "" },
        p.unsigned_abs() / 100,
        p.unsigned_abs() % 100
    )
}
fn compact_date(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| d.format("%d %b").to_string())
        .unwrap_or_else(|_| date.into())
}
fn short_date(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|d| d.format("%d %b %Y").to_string())
        .unwrap_or_else(|_| date.into())
}
fn open_path(path: &PathBuf) {
    let _ = std::process::Command::new(if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    })
    .arg(path)
    .spawn();
}
fn safe_csv(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.into()
    }
}
fn backup_file(root: &std::path::Path, path: &str) -> Result<String> {
    let source = PathBuf::from(path);
    let folder = root.join("attachments");
    std::fs::create_dir_all(&folder)?;
    let name = source.file_name().context("Invalid attachment path")?;
    let dest = folder.join(name);
    std::fs::copy(&source, &dest)?;
    Ok(format!("attachments/{}", name.to_string_lossy()))
}

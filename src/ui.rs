use eframe::egui::{self, Color32, Rounding, Margin, PopupCloseBehavior, RichText, Stroke, Vec2};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::history::{fetch_manifest_content, fetch_releases, ReleaseEntry};
use crate::i18n::{draw_flag, texts, Language, Texts};
use crate::slssteam::{check_slsteam_moon, find_best_compatible_release, SlsMoonStatus};
use crate::steam::{
    inspect_steam, lock_only, restart_steam, rollback_to_manifest, unlock_and_update_steam,
    unlock_updates, LockState, ManifestChannel, SteamStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Home,
    Advanced,
    Logs,
}

enum BgEvent {
    ReleasesLoaded {
        channel: ManifestChannel,
        page: u32,
        result: Result<Vec<ReleaseEntry>, String>,
    },
    SlsMoonChecked(Result<SlsMoonStatus, String>),
    OptimizeSelected {
        channel: ManifestChannel,
        releases: Vec<ReleaseEntry>,
        selected_sha: String,
    },
    Log(String),
    ActionFinished(Result<(), String>),
}

pub struct SteamRollbackrApp {
    lang: Language,
    active_tab: ActiveTab,
    status: SteamStatus,
    channel: ManifestChannel,
    releases: Vec<ReleaseEntry>,
    selected_idx: Option<usize>,
    details_entry: Option<ReleaseEntry>,
    current_page: u32,
    has_more: bool,
    loading_releases: bool,
    checking_moon: bool,
    moon_status: Option<Result<SlsMoonStatus, String>>,
    busy: bool,
    filter_text: String,
    logs: Vec<String>,
    initial_ready_only: bool,
    rx: Receiver<BgEvent>,
    tx: mpsc::Sender<BgEvent>,
}

fn apply_fluent_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(35, 35, 38);
    visuals.window_fill = Color32::from_rgb(43, 43, 46);
    visuals.extreme_bg_color = Color32::from_rgb(28, 28, 30);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(43, 43, 46);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 61));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(52, 52, 56);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(68, 68, 72));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(64, 64, 70);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(92, 92, 98));
    visuals.widgets.active.bg_fill = Color32::from_rgb(74, 74, 82);
    visuals.selection.bg_fill = Color32::from_rgb(48, 92, 148);
    let r = Rounding::same(6.0);
    visuals.widgets.inactive.rounding = r;
    visuals.widgets.hovered.rounding = r;
    visuals.widgets.active.rounding = r;
    visuals.widgets.noninteractive.rounding = Rounding::same(10.0);
    ctx.set_visuals(visuals);
}

fn card_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(Color32::from_rgb(43, 43, 46))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 61)))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(16.0))
}

fn pill_badge(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    egui::Frame::none()
        .fill(bg)
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(fg).strong());
        });
}

fn draw_sparkle(painter: &egui::Painter, cx: f32, cy: f32, s: f32, m: f32, color: Color32) {
    let pts_v = vec![
        egui::pos2(cx, cy - s),
        egui::pos2(cx + m, cy),
        egui::pos2(cx, cy + s),
        egui::pos2(cx - m, cy),
    ];
    let pts_h = vec![
        egui::pos2(cx - s, cy),
        egui::pos2(cx, cy - m),
        egui::pos2(cx + s, cy),
        egui::pos2(cx, cy + m),
    ];
    painter.add(egui::Shape::convex_polygon(pts_v, color, Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(pts_h, color, Stroke::NONE));
}

fn external_link_btn(ui: &mut egui::Ui, url: &str, hover: &str) {
    if ui
        .add(egui::Button::new(RichText::new("↗").size(12.0)).min_size(Vec2::new(24.0, 22.0)))
        .on_hover_text(hover)
        .clicked()
    {
        let _ = open_url(url);
    }
}

fn open_url(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open URL: {e}"))
}

impl SteamRollbackrApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_fluent_dark_theme(&cc.egui_ctx);

        let (tx, rx) = mpsc::channel();
        let status = inspect_steam();
        let default_channel = status
            .current_version
            .as_ref()
            .map(|v| v.channel)
            .unwrap_or(ManifestChannel::SteamRt64);

        let lang = Language::En;
        let t = texts(lang);

        let mut app = Self {
            lang,
            active_tab: ActiveTab::Home,
            status,
            channel: default_channel,
            releases: Vec::new(),
            selected_idx: None,
            details_entry: None,
            current_page: 1,
            has_more: true,
            loading_releases: false,
            checking_moon: false,
            moon_status: None,
            busy: false,
            filter_text: String::new(),
            logs: vec![t.ready_log.to_string()],
            initial_ready_only: true,
            rx,
            tx,
        };
        app.load_releases(1, true);
        app.check_moon();
        app
    }

    fn set_language(&mut self, new_lang: Language) {
        self.lang = new_lang;
        if self.initial_ready_only && self.logs.len() == 1 {
            self.logs[0] = texts(new_lang).ready_log.to_string();
        }
    }

    fn refresh_status(&mut self) {
        self.status = inspect_steam();
    }

    fn check_moon(&mut self) {
        if self.checking_moon {
            return;
        }
        self.checking_moon = true;
        let steam_dir = self.status.steam_dir.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = check_slsteam_moon(steam_dir.as_deref());
            let _ = tx.send(BgEvent::SlsMoonChecked(res));
        });
    }

    fn load_releases(&mut self, page: u32, reset: bool) {
        if self.loading_releases {
            return;
        }
        if reset {
            self.releases.clear();
            self.selected_idx = None;
            self.current_page = 1;
            self.has_more = true;
        }
        self.loading_releases = true;
        let channel = self.channel;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = fetch_releases(channel, page);
            let _ = tx.send(BgEvent::ReleasesLoaded {
                channel,
                page,
                result,
            });
        });
    }

    fn start_rollback(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            self.initial_ready_only = false;
            self.logs.push("Error: Steam directory not found.".to_string());
            return;
        };
        let Some(idx) = self.selected_idx else {
            self.initial_ready_only = false;
            self.logs.push("Error: Please select a release first.".to_string());
            return;
        };
        let entry = self.releases[idx].clone();
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs.push(format!(
            "Fetching manifest for {} ({})...",
            entry.date_display, entry.short_sha
        ));

        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = match fetch_manifest_content(&entry.sha, entry.channel) {
                Ok((manifest, ver)) => {
                    if let Some(v) = ver {
                        log_fn(&format!("Downloaded manifest version {v}."));
                    }
                    rollback_to_manifest(&steam_dir, entry.channel, &manifest, &mut log_fn)
                }
                Err(e) => Err(e),
            };
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_unlock(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = unlock_updates(&steam_dir, &mut log_fn);
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_unlock_and_update(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            self.initial_ready_only = false;
            self.logs.push("Error: Steam directory not found.".to_string());
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs
            .push("Unlocking Steam and updating to the latest official release...".to_string());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = unlock_and_update_steam(&steam_dir, &mut log_fn);
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_lock_only(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = lock_only(&steam_dir, &mut log_fn);
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_restart_steam(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = restart_steam(&steam_dir, &mut log_fn);
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn start_optimize(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            self.initial_ready_only = false;
            self.logs.push("Error: Steam directory not found.".to_string());
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs
            .push("Checking latest slsteam-moon compatible Steam release...".to_string());

        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut log_fn = |msg: &str| {
                let _ = tx.send(BgEvent::Log(msg.to_string()));
            };
            let res = match check_slsteam_moon(Some(&steam_dir)) {
                Ok(moon) => {
                    log_fn(&format!(
                        "slsteam-moon / steam-monitor pattern date: {}",
                        moon.monitor_display
                    ));
                    match find_best_compatible_release(moon.monitor_timestamp_utc) {
                        Ok((best, list)) => {
                            log_fn(&format!(
                                "Auto-selected: [{}] {} ({})",
                                best.channel.suffix(),
                                best.date_display,
                                best.short_sha
                            ));
                            let _ = tx.send(BgEvent::OptimizeSelected {
                                channel: best.channel,
                                releases: list,
                                selected_sha: best.sha.clone(),
                            });
                            match fetch_manifest_content(&best.sha, best.channel) {
                                Ok((manifest, ver)) => {
                                    if let Some(v) = ver {
                                        log_fn(&format!("Downloaded manifest version {v}."));
                                    }
                                    rollback_to_manifest(
                                        &steam_dir,
                                        best.channel,
                                        &manifest,
                                        &mut log_fn,
                                    )
                                }
                                Err(e) => Err(e),
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            };
            let _ = tx.send(BgEvent::ActionFinished(res));
        });
    }

    fn poll_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                BgEvent::ReleasesLoaded {
                    channel,
                    page,
                    result,
                } => {
                    if channel == self.channel {
                        self.loading_releases = false;
                        match result {
                            Ok(list) => {
                                if list.len() < 40 {
                                    self.has_more = false;
                                }
                                self.current_page = page;
                                self.releases.extend(list);
                            }
                            Err(e) => {
                                self.initial_ready_only = false;
                                self.logs.push(format!("Failed to load releases: {e}"));
                            }
                        }
                    }
                }
                BgEvent::SlsMoonChecked(res) => {
                    self.checking_moon = false;
                    if let Err(ref e) = res {
                        self.initial_ready_only = false;
                        self.logs.push(format!("slsteam-moon check failed: {e}"));
                    }
                    self.moon_status = Some(res);
                }
                BgEvent::OptimizeSelected {
                    channel,
                    releases,
                    selected_sha,
                } => {
                    self.channel = channel;
                    self.selected_idx = releases.iter().position(|r| r.sha == selected_sha);
                    self.releases = releases;
                    self.current_page = 1;
                    self.has_more = true;
                }
                BgEvent::Log(msg) => {
                    self.initial_ready_only = false;
                    self.logs.push(msg);
                }
                BgEvent::ActionFinished(res) => {
                    self.busy = false;
                    if let Err(e) = res {
                        self.initial_ready_only = false;
                        self.logs.push(format!("ERROR: {e}"));
                    }
                    self.refresh_status();
                    self.check_moon();
                }
            }
        }
    }

    fn render_sidebar(&mut self, ctx: &egui::Context, t: &Texts) {
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(190.0)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(28, 28, 30))
                    .inner_margin(Margin::symmetric(12.0, 14.0)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("SteamRollbackr")
                        .size(16.0)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(14.0);

                for (tab, label) in [
                    (ActiveTab::Home, t.nav_home),
                    (ActiveTab::Advanced, t.nav_advanced),
                    (ActiveTab::Logs, t.nav_logs),
                ] {
                    let active = self.active_tab == tab;
                    let fill = if active {
                        Color32::from_rgb(46, 46, 52)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let btn = egui::Button::new(
                        RichText::new(format!("   {label}"))
                            .size(14.0)
                            .color(if active { Color32::WHITE } else { Color32::LIGHT_GRAY }),
                    )
                    .fill(fill)
                    .stroke(Stroke::NONE)
                    .min_size(Vec2::new(ui.available_width(), 32.0));

                    let resp = ui.add(btn);
                    if active {
                        let r = resp.rect;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(r.left() + 2.0, r.top() + 6.0),
                                Vec2::new(3.0, r.height() - 12.0),
                            ),
                            Rounding::same(2.0),
                            Color32::from_rgb(86, 156, 244),
                        );
                    }
                    if resp.clicked() {
                        self.active_tab = tab;
                    }
                    ui.add_space(2.0);
                }

                // Bottom utilities
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(4.0);
                    // Language selector
                    ui.horizontal(|ui| {
                        let popup_id = ui.make_persistent_id("lang_dropdown_popup");
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(24.0, 16.0), egui::Sense::click());
                        draw_flag(ui.painter(), rect, self.lang);
                        let label_resp = ui.add(
                            egui::Button::new(RichText::new(self.lang.name()).small())
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        );
                        if resp.clicked() || label_resp.clicked() {
                            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                        }

                        egui::popup_below_widget(
                            ui,
                            popup_id,
                            &resp,
                            PopupCloseBehavior::CloseOnClick,
                            |ui| {
                                ui.set_min_width(120.0);
                                for lang_opt in Language::all() {
                                    if lang_opt == self.lang {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        let (r, flag_resp) = ui.allocate_exact_size(
                                            Vec2::new(22.0, 14.0),
                                            egui::Sense::click(),
                                        );
                                        draw_flag(ui.painter(), r, lang_opt);
                                        let btn_resp = ui.selectable_label(false, lang_opt.name());
                                        if flag_resp.clicked() || btn_resp.clicked() {
                                            self.set_language(lang_opt);
                                        }
                                    });
                                }
                            },
                        );
                    });

                    ui.add_space(6.0);
                    let has_steam = self.status.steam_dir.is_some();
                    if ui
                        .add_enabled(
                            has_steam && !self.busy,
                            egui::Button::new(t.restart_steam)
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::new(ui.available_width(), 28.0)),
                        )
                        .clicked()
                    {
                        self.start_restart_steam();
                    }

                    if ui
                        .add_enabled(
                            !self.busy,
                            egui::Button::new(t.refresh_status)
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::new(ui.available_width(), 28.0)),
                        )
                        .clicked()
                    {
                        self.refresh_status();
                        self.check_moon();
                    }
                    ui.separator();
                });
            });
    }

    fn render_home(&mut self, ui: &mut egui::Ui, t: &Texts) {
        let has_steam = self.status.steam_dir.is_some();

        // 1. Compatibility Banner at top
        let (banner_bg, banner_border, banner_fg, banner_text) = if self.checking_moon {
            (
                Color32::from_rgb(38, 44, 56),
                Color32::from_rgb(66, 80, 108),
                Color32::LIGHT_GRAY,
                t.banner_checking,
            )
        } else {
            match &self.moon_status {
                Some(Ok(st)) => match st.updated_for_latest_steam {
                    Some(true) => (
                        Color32::from_rgb(28, 56, 38),
                        Color32::from_rgb(56, 128, 78),
                        Color32::from_rgb(140, 235, 165),
                        t.banner_compatible,
                    ),
                    Some(false) => (
                        Color32::from_rgb(58, 46, 24),
                        Color32::from_rgb(138, 104, 42),
                        Color32::from_rgb(250, 212, 120),
                        t.banner_incompatible,
                    ),
                    None => (
                        Color32::from_rgb(43, 43, 46),
                        Color32::from_rgb(58, 58, 61),
                        Color32::LIGHT_GRAY,
                        t.banner_checking,
                    ),
                },
                Some(Err(_)) => (
                    Color32::from_rgb(58, 32, 32),
                    Color32::from_rgb(128, 58, 58),
                    Color32::from_rgb(245, 160, 160),
                    t.check_failed,
                ),
                None => (
                    Color32::from_rgb(43, 43, 46),
                    Color32::from_rgb(58, 58, 61),
                    Color32::LIGHT_GRAY,
                    t.banner_checking,
                ),
            }
        };

        egui::Frame::none()
            .fill(banner_bg)
            .stroke(Stroke::new(1.0_f32, banner_border))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if self.checking_moon {
                        ui.spinner();
                    }
                    ui.add(
                        egui::Label::new(RichText::new(banner_text).color(banner_fg).strong()).wrap(),
                    );
                });
            });

        ui.add_space(12.0);

        // 2. Dual Primary Action Card (Magic Rollback + Unlock & Update Steam)
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.columns(2, |cols| {
                // Left: Magic Rollback
                cols[0].vertical(|ui| {
                    let magic_btn = egui::Button::new(
                        RichText::new(format!("      {}", t.magic_rollback))
                            .size(15.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(42, 88, 142))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(76, 136, 206)))
                    .min_size(Vec2::new(ui.available_width(), 40.0));

                    let resp = ui.add_enabled(has_steam && !self.busy, magic_btn);
                    let r = resp.rect;
                    draw_sparkle(
                        ui.painter(),
                        r.center().x - 64.0,
                        r.center().y,
                        8.0,
                        2.5,
                        Color32::from_rgb(255, 215, 80),
                    );
                    if resp.clicked() {
                        self.start_optimize();
                    }
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(t.magic_desc)
                                .small()
                                .color(Color32::LIGHT_GRAY),
                        )
                        .wrap(),
                    );
                });

                // Right: Unlock & Update Steam
                cols[1].vertical(|ui| {
                    let restore_btn = egui::Button::new(
                        RichText::new(t.unlock_and_update)
                            .size(15.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(38, 92, 66))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(66, 146, 106)))
                    .min_size(Vec2::new(ui.available_width(), 40.0));

                    if ui
                        .add_enabled(has_steam && !self.busy, restore_btn)
                        .clicked()
                    {
                        self.start_unlock_and_update();
                    }
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(t.unlock_and_update_desc)
                                .small()
                                .color(Color32::LIGHT_GRAY),
                        )
                        .wrap(),
                    );
                });
            });

            if self.busy {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t.working).strong());
                    if let Some(last) = self.logs.last() {
                        ui.label(RichText::new(format!("— {last}")).color(Color32::LIGHT_GRAY));
                    }
                });
            }
        });

        ui.add_space(12.0);

        // 3. System Status Card
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t.system_status)
                        .size(16.0)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(6.0);
                match &self.status.lock_state {
                    LockState::Locked => pill_badge(
                        ui,
                        t.locked_pill,
                        Color32::from_rgb(35, 77, 46),
                        Color32::from_rgb(130, 230, 155),
                    ),
                    LockState::PartiallyLocked(_) => pill_badge(
                        ui,
                        t.partial_pill,
                        Color32::from_rgb(90, 66, 24),
                        Color32::from_rgb(250, 212, 120),
                    ),
                    LockState::Unlocked => pill_badge(
                        ui,
                        t.unlocked_pill,
                        Color32::from_rgb(82, 36, 36),
                        Color32::from_rgb(245, 150, 150),
                    ),
                }
            });

            ui.add_space(10.0);

            egui::Grid::new("system_status_grid")
                .num_columns(3)
                .spacing([18.0, 10.0])
                .show(ui, |ui| {
                    // Row 1: Steam Client
                    ui.label(RichText::new(t.steam_client).color(Color32::LIGHT_GRAY));
                    match &self.status.current_version {
                        Some(v) => {
                            ui.label(format!(
                                "{}  ({})  [{}]",
                                v.version_id,
                                v.formatted_date,
                                v.channel.suffix()
                            ));
                        }
                        None => {
                            ui.label(t.unknown_version);
                        }
                    }
                    if self.status.is_flatpak {
                        pill_badge(
                            ui,
                            "Flatpak",
                            Color32::from_rgb(44, 58, 86),
                            Color32::LIGHT_BLUE,
                        );
                    } else {
                        ui.label("");
                    }
                    ui.end_row();

                    // Row 2: Steam Path
                    ui.label(RichText::new(t.steam_path).color(Color32::LIGHT_GRAY));
                    match &self.status.steam_dir {
                        Some(dir) => {
                            ui.monospace(dir.display().to_string());
                        }
                        None => {
                            ui.label(RichText::new(t.steam_not_found).color(Color32::LIGHT_RED));
                        }
                    }
                    ui.label("");
                    ui.end_row();

                    // Row 3: slsteam-moon
                    ui.label(RichText::new("slsteam-moon").color(Color32::LIGHT_GRAY));
                    let moon_ver = self
                        .status
                        .lumen_versions
                        .slsteam_moon
                        .as_deref()
                        .unwrap_or_else(|| {
                            if self.status.lumen_versions.slssteam_installed {
                                t.installed_pill
                            } else {
                                t.not_found_pill
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.label(moon_ver);
                        if let Some(Ok(st)) = &self.moon_status {
                            if let Some(supported) = st.local_pattern_supported {
                                if supported {
                                    pill_badge(
                                        ui,
                                        t.local_pattern_ok,
                                        Color32::from_rgb(35, 77, 46),
                                        Color32::from_rgb(130, 230, 155),
                                    );
                                } else {
                                    pill_badge(
                                        ui,
                                        t.local_pattern_missing,
                                        Color32::from_rgb(82, 36, 36),
                                        Color32::from_rgb(245, 150, 150),
                                    );
                                }
                            }
                        }
                    });
                    external_link_btn(
                        ui,
                        "https://github.com/swwayps/slsteam-moon",
                        t.open_repo_hover,
                    );
                    ui.end_row();

                    // Row 4: LuaTools
                    ui.label(RichText::new("LuaTools").color(Color32::LIGHT_GRAY));
                    ui.horizontal(|ui| {
                        if let Some(tag) = &self.status.lumen_versions.luatools {
                            ui.label(tag);
                            pill_badge(
                                ui,
                                t.installed_pill,
                                Color32::from_rgb(59, 47, 92),
                                Color32::from_rgb(198, 178, 250),
                            );
                        } else if self.status.lumen_versions.luatools_installed {
                            pill_badge(
                                ui,
                                t.installed_pill,
                                Color32::from_rgb(59, 47, 92),
                                Color32::from_rgb(198, 178, 250),
                            );
                        } else {
                            pill_badge(
                                ui,
                                t.not_found_pill,
                                Color32::from_rgb(54, 54, 58),
                                Color32::GRAY,
                            );
                        }
                    });
                    external_link_btn(
                        ui,
                        "https://github.com/swwayps/luatools-moon",
                        t.open_repo_hover,
                    );
                    ui.end_row();

                    // Row 5: Lumen
                    ui.label(RichText::new("Lumen").color(Color32::LIGHT_GRAY));
                    ui.horizontal(|ui| {
                        if let Some(tag) = &self.status.lumen_versions.lumen {
                            ui.label(tag);
                            pill_badge(
                                ui,
                                t.installed_pill,
                                Color32::from_rgb(59, 47, 92),
                                Color32::from_rgb(198, 178, 250),
                            );
                        } else if self.status.lumen_versions.lumen_installed {
                            pill_badge(
                                ui,
                                t.installed_pill,
                                Color32::from_rgb(59, 47, 92),
                                Color32::from_rgb(198, 178, 250),
                            );
                        } else {
                            pill_badge(
                                ui,
                                t.not_found_pill,
                                Color32::from_rgb(54, 54, 58),
                                Color32::GRAY,
                            );
                        }
                    });
                    external_link_btn(
                        ui,
                        "https://github.com/swwayps/luatools-moon",
                        t.open_repo_hover,
                    );
                    ui.end_row();
                });
        });
    }

    fn render_advanced(&mut self, ui: &mut egui::Ui, t: &Texts) {
        ui.label(
            RichText::new(t.advanced_title)
                .size(20.0)
                .strong()
                .color(Color32::WHITE),
        );
        ui.label(RichText::new(t.advanced_subtitle).color(Color32::LIGHT_GRAY));
        ui.add_space(10.0);

        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.channel).strong());
                let mut changed = false;
                for (ch, label) in [
                    (ManifestChannel::SteamRt64, t.channel_steamrt64),
                    (ManifestChannel::Ubuntu12, t.channel_ubuntu12),
                ] {
                    if ui.selectable_value(&mut self.channel, ch, label).changed() {
                        changed = true;
                    }
                }
                if changed {
                    self.load_releases(1, true);
                }
            });

            let rec_msg = match self.status.current_version.as_ref().map(|v| v.channel) {
                Some(ManifestChannel::Ubuntu12) => t.tip_ubuntu12_active,
                _ => t.tip_steamrt64_rec,
            };
            ui.label(RichText::new(rec_msg).small().color(Color32::LIGHT_GRAY));
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(t.filter);
                ui.add(egui::TextEdit::singleline(&mut self.filter_text).hint_text(t.filter_hint));
                if ui.button(t.reload_list).clicked() && !self.loading_releases {
                    self.load_releases(1, true);
                }
            });
        });

        ui.add_space(10.0);
        ui.label(RichText::new(t.available_releases).strong());
        ui.add_space(4.0);

        let list_height = (ui.available_height() - 56.0).max(160.0);
        card_frame().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("release_list_scroll")
                .max_height(list_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let query = self.filter_text.to_lowercase();

                    for (i, entry) in self.releases.iter().enumerate() {
                        if !query.is_empty()
                            && !entry.date_display.to_lowercase().contains(&query)
                            && !entry.short_sha.to_lowercase().contains(&query)
                            && !entry.message.to_lowercase().contains(&query)
                        {
                            continue;
                        }
                        let selected = self.selected_idx == Some(i);
                        let short_title = if i == 0 {
                            format!("{}   {}", entry.date_display, t.latest_badge)
                        } else {
                            entry.date_display.clone()
                        };
                        let hover_details = format!(
                            "{}: {}\n{}: {}\n{}: {}\n{}: {}",
                            t.date_label,
                            entry.date_display,
                            t.channel,
                            entry.channel.suffix(),
                            t.commit_sha_label,
                            entry.sha,
                            t.commit_msg_label,
                            entry.message
                        );

                        ui.horizontal(|ui| {
                            let info_w = 56.0;
                            let btn_w = (ui.available_width() - info_w - 8.0).max(120.0);
                            let row_btn = egui::Button::new(short_title)
                                .selected(selected)
                                .min_size(Vec2::new(btn_w, 28.0));
                            if ui
                                .add(row_btn)
                                .on_hover_text(&hover_details)
                                .clicked()
                                && !self.busy
                            {
                                self.selected_idx = Some(i);
                            }

                            let info_btn =
                                egui::Button::new(t.info_btn).min_size(Vec2::new(info_w, 28.0));
                            if ui.add(info_btn).on_hover_text(t.info_hover).clicked() {
                                self.details_entry = Some(entry.clone());
                            }
                        });
                    }

                    ui.add_space(4.0);
                    if self.loading_releases {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t.loading_releases);
                        });
                    } else if self.has_more {
                        if ui.button(t.load_older).clicked() {
                            let next_page = self.current_page + 1;
                            self.load_releases(next_page, false);
                        }
                    }
                });
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let has_steam = self.status.steam_dir.is_some();
            let can_rollback = has_steam && self.selected_idx.is_some() && !self.busy;
            let can_unlock = has_steam && !self.busy && self.status.lock_state.is_locked();
            let can_lock = has_steam && !self.busy;

            if ui
                .add_enabled(can_rollback, egui::Button::new(t.rollback_and_lock))
                .clicked()
            {
                self.start_rollback();
            }
            if ui
                .add_enabled(can_unlock, egui::Button::new(t.unlock_updates))
                .clicked()
            {
                self.start_unlock();
            }
            if ui
                .add_enabled(can_lock, egui::Button::new(t.lock_current))
                .clicked()
            {
                self.start_lock_only();
            }
            if self.busy {
                ui.spinner();
                ui.label(t.working);
            }
        });
    }

    fn render_logs(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, t: &Texts) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t.logs_title)
                    .size(20.0)
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t.clear_logs).clicked() {
                    self.initial_ready_only = false;
                    self.logs.clear();
                }
                if ui.button(t.copy_logs).clicked() {
                    ctx.copy_text(self.logs.join("\n"));
                }
            });
        });
        ui.add_space(10.0);

        card_frame().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("activity_log_scroll")
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for line in &self.logs {
                        ui.monospace(line);
                    }
                });
        });
    }
}

impl eframe::App for SteamRollbackrApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        if self.busy || self.loading_releases || self.checking_moon {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let t = texts(self.lang);

        self.render_sidebar(ctx, &t);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(35, 35, 38))
                    .inner_margin(Margin::same(20.0)),
            )
            .show(ctx, |ui| match self.active_tab {
                ActiveTab::Home => self.render_home(ui, &t),
                ActiveTab::Advanced => self.render_advanced(ui, &t),
                ActiveTab::Logs => self.render_logs(ctx, ui, &t),
            });

        if let Some(entry) = self.details_entry.clone() {
            let mut open = true;
            egui::Window::new(t.release_details_title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.date_label).strong());
                        ui.label(&entry.date_display);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.channel).strong());
                        ui.label(entry.channel.suffix());
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t.commit_sha_label).strong());
                        ui.monospace(&entry.sha);
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new(t.commit_msg_label).strong());
                    ui.label(&entry.message);
                    ui.add_space(6.0);
                    let url = format!(
                        "https://github.com/SteamTracking/SteamTracking/commit/{}",
                        entry.sha
                    );
                    ui.hyperlink_to(t.view_on_github, url);
                });
            if !open {
                self.details_entry = None;
            }
        }
    }
}

use eframe::egui::{self, Color32, PopupCloseBehavior, RichText};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::history::{fetch_manifest_content, fetch_releases, ReleaseEntry};
use crate::i18n::{draw_flag, texts, Language};
use crate::slssteam::{check_slsteam_moon, find_best_compatible_release, SlsMoonStatus};
use crate::steam::{
    inspect_steam, lock_only, rollback_to_manifest, unlock_updates, LockState, ManifestChannel,
    SteamStatus,
};

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

impl SteamRollbackrApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
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

    fn start_optimize(&mut self) {
        let Some(steam_dir) = self.status.steam_dir.clone() else {
            self.initial_ready_only = false;
            self.logs.push("Error: Steam directory not found.".to_string());
            return;
        };
        self.busy = true;
        self.initial_ready_only = false;
        self.logs.clear();
        self.logs.push("Checking latest slsteam-moon compatible Steam release...".to_string());

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
}

impl eframe::App for SteamRollbackrApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        if self.busy || self.loading_releases || self.checking_moon {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let t = texts(self.lang);

        egui::TopBottomPanel::top("header_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("SteamRollbackr");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let popup_id = ui.make_persistent_id("lang_dropdown_popup");
                    let flag_size = egui::vec2(24.0, 16.0);
                    let (rect, resp) = ui.allocate_exact_size(flag_size, egui::Sense::click());
                    draw_flag(ui.painter(), rect, self.lang);
                    let resp = resp.on_hover_text(self.lang.name());
                    if resp.clicked() {
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
                                    let (r, flag_resp) =
                                        ui.allocate_exact_size(egui::vec2(22.0, 14.0), egui::Sense::click());
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
            });
            ui.add_space(6.0);

            // Status Card (Refresh Status moved inside top-right of this box)
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    match &self.status.steam_dir {
                        Some(dir) => {
                            ui.label(RichText::new(t.steam_path).strong());
                            ui.monospace(dir.display().to_string());
                            if self.status.is_flatpak {
                                ui.label(RichText::new("[Flatpak]").color(Color32::LIGHT_BLUE));
                            }
                        }
                        None => {
                            ui.label(
                                RichText::new(t.steam_not_found)
                                    .color(Color32::RED)
                                    .strong(),
                            );
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t.refresh_status).clicked() && !self.busy {
                            self.refresh_status();
                            self.check_moon();
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new(t.current_version).strong());
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
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new(t.update_lock_status).strong());
                    let (text, color) = match &self.status.lock_state {
                        LockState::Locked => (
                            t.locked.to_string(),
                            Color32::from_rgb(80, 200, 120),
                        ),
                        LockState::PartiallyLocked(reason) => (
                            format!("{} ({reason})", t.partially_locked),
                            Color32::YELLOW,
                        ),
                        LockState::Unlocked => (
                            t.unlocked.to_string(),
                            Color32::from_rgb(240, 100, 100),
                        ),
                    };
                    ui.label(RichText::new(text).color(color).strong());
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("slsteam-moon:").strong());
                    if self.checking_moon {
                        ui.spinner();
                        ui.label(t.checking_moon);
                    } else {
                        match &self.moon_status {
                            Some(Ok(st)) => {
                                match st.updated_for_latest_steam {
                                    Some(true) => {
                                        ui.label(
                                            RichText::new(t.moon_updated)
                                                .color(Color32::from_rgb(80, 200, 120))
                                                .strong(),
                                        );
                                    }
                                    Some(false) => {
                                        ui.label(
                                            RichText::new(t.moon_not_updated)
                                                .color(Color32::YELLOW)
                                                .strong(),
                                        );
                                    }
                                    None => {
                                        ui.label(t.moon_incomplete);
                                    }
                                }
                                if let Some(supported) = st.local_pattern_supported {
                                    let (p_txt, p_col) = if supported {
                                        (t.local_pattern_ok, Color32::from_rgb(80, 200, 120))
                                    } else {
                                        (t.local_pattern_missing, Color32::from_rgb(240, 100, 100))
                                    };
                                    ui.label(RichText::new(format!("|  {p_txt}")).color(p_col));
                                }
                                if ui.small_button(t.details_btn).clicked() {
                                    self.initial_ready_only = false;
                                    let mut lines = vec![
                                        format!("steam-monitor last update: {}", st.monitor_display),
                                        format!("slsteam-moon last commit: {}", st.moon_display),
                                    ];
                                    if let Some(sd) = &st.latest_steam_display {
                                        lines.push(format!("Latest Steam Linux release: {sd}"));
                                    }
                                    if let Some(sha) = &st.local_steamclient_sha {
                                        lines.push(format!("Local steamclient.so SHA256: {sha}"));
                                    }
                                    for l in lines {
                                        self.logs.push(l);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                ui.label(
                                    RichText::new(t.check_failed)
                                        .color(Color32::LIGHT_RED),
                                )
                                .on_hover_text(e);
                            }
                            None => {
                                ui.label(t.not_checked_yet);
                            }
                        }
                    }
                });
            });

            ui.add_space(6.0);

            // Big Magic Button above Channel line
            let has_steam = self.status.steam_dir.is_some();
            let magic_btn = egui::Button::new(
                RichText::new(format!("    {}", t.optimize_btn))
                    .size(15.0)
                    .strong(),
            )
            .fill(Color32::from_rgb(38, 79, 120))
            .min_size(egui::vec2(ui.available_width(), 34.0));

            let magic_resp = ui.add_enabled(has_steam && !self.busy, magic_btn);
            // Draw crisp gold 4-point sparkle icon inside the Magic Button
            let r = magic_resp.rect;
            let cx = r.center().x - 56.0;
            let cy = r.center().y;
            let s = 8.0_f32;
            let m = 2.6_f32;
            let gold = Color32::from_rgb(255, 215, 80);
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
            ui.painter()
                .add(egui::Shape::convex_polygon(pts_v, gold, egui::Stroke::NONE));
            ui.painter()
                .add(egui::Shape::convex_polygon(pts_h, gold, egui::Stroke::NONE));
            if magic_resp.clicked() {
                self.start_optimize();
            }
            ui.label(RichText::new(t.optimize_hover).small().color(Color32::LIGHT_GRAY));

            ui.add_space(6.0);

            // Channel & Recommendation Bar
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

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(t.filter);
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter_text).hint_text(t.filter_hint),
                );
                if ui.button(t.reload_list).clicked() && !self.loading_releases {
                    self.load_releases(1, true);
                }
            });
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("footer_panel").show(ctx, |ui| {
            ui.add_space(6.0);
            // Action Buttons
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

            ui.add_space(6.0);

            // Log Output
            ui.label(RichText::new(t.activity_log).strong());
            egui::Frame::group(ui.style()).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("activity_log_scroll")
                    .stick_to_bottom(true)
                    .max_height(95.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &self.logs {
                            ui.monospace(line);
                        }
                    });
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(RichText::new(t.available_releases).strong());
            ui.add_space(4.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("release_list_scroll")
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
                                    .min_size(egui::vec2(btn_w, 28.0));
                                if ui
                                    .add(row_btn)
                                    .on_hover_text(&hover_details)
                                    .clicked()
                                    && !self.busy
                                {
                                    self.selected_idx = Some(i);
                                }

                                let info_btn = egui::Button::new(t.info_btn)
                                    .min_size(egui::vec2(info_w, 28.0));
                                if ui
                                    .add(info_btn)
                                    .on_hover_text(t.info_hover)
                                    .clicked()
                                {
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

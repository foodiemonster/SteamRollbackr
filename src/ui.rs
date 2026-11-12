use eframe::egui::{self, Color32, RichText};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::history::{fetch_manifest_content, fetch_releases, ReleaseEntry};
use crate::slssteam::{check_slsteam_moon, SlsMoonStatus};
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
    Log(String),
    ActionFinished(Result<(), String>),
}

pub struct SteamRollbackrApp {
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

        let mut app = Self {
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
            logs: vec!["Ready. Select a release and click Rollback & Lock.".to_string()],
            rx,
            tx,
        };
        app.load_releases(1, true);
        app.check_moon();
        app
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
            self.logs.push("Error: Steam directory not found.".to_string());
            return;
        };
        let Some(idx) = self.selected_idx else {
            self.logs.push("Error: Please select a release first.".to_string());
            return;
        };
        let entry = self.releases[idx].clone();
        self.busy = true;
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
                                self.logs.push(format!("Failed to load releases: {e}"));
                            }
                        }
                    }
                }
                BgEvent::SlsMoonChecked(res) => {
                    self.checking_moon = false;
                    self.moon_status = Some(res);
                }
                BgEvent::Log(msg) => {
                    self.logs.push(msg);
                }
                BgEvent::ActionFinished(res) => {
                    self.busy = false;
                    if let Err(e) = res {
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

        egui::TopBottomPanel::top("header_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("SteamRollbackr");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Refresh Status").clicked() && !self.busy {
                        self.refresh_status();
                        self.check_moon();
                    }
                });
            });
            ui.add_space(6.0);

            // Status Card
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                match &self.status.steam_dir {
                    Some(dir) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Steam Path:").strong());
                            ui.monospace(dir.display().to_string());
                            if self.status.is_flatpak {
                                ui.label(RichText::new("[Flatpak]").color(Color32::LIGHT_BLUE));
                            }
                        });
                    }
                    None => {
                        ui.label(
                            RichText::new("Steam installation not found!")
                                .color(Color32::RED)
                                .strong(),
                        );
                    }
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Current Version:").strong());
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
                            ui.label("Unknown (no local client manifest found)");
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Update Lock Status:").strong());
                    let (text, color) = match &self.status.lock_state {
                        LockState::Locked => (
                            "LOCKED (Updates Blocked)".to_string(),
                            Color32::from_rgb(80, 200, 120),
                        ),
                        LockState::PartiallyLocked(reason) => (
                            format!("PARTIALLY LOCKED ({reason})"),
                            Color32::YELLOW,
                        ),
                        LockState::Unlocked => (
                            "UNLOCKED (Updates Allowed)".to_string(),
                            Color32::from_rgb(240, 100, 100),
                        ),
                    };
                    ui.label(RichText::new(text).color(color).strong());
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("slsteam-moon:").strong());
                    if self.checking_moon {
                        ui.spinner();
                        ui.label("Checking steam-monitor & slsteam-moon...");
                    } else {
                        match &self.moon_status {
                            Some(Ok(st)) => {
                                match st.updated_for_latest_steam {
                                    Some(true) => {
                                        ui.label(
                                            RichText::new("Updated for latest Steam")
                                                .color(Color32::from_rgb(80, 200, 120))
                                                .strong(),
                                        );
                                    }
                                    Some(false) => {
                                        ui.label(
                                            RichText::new("NOT yet updated for latest Steam")
                                                .color(Color32::YELLOW)
                                                .strong(),
                                        );
                                    }
                                    None => {
                                        ui.label("Pattern update check incomplete");
                                    }
                                }
                                if let Some(supported) = st.local_pattern_supported {
                                    let (p_txt, p_col) = if supported {
                                        ("Local client: Pattern OK", Color32::from_rgb(80, 200, 120))
                                    } else {
                                        ("Local client: No pattern", Color32::from_rgb(240, 100, 100))
                                    };
                                    ui.label(RichText::new(format!("|  {p_txt}")).color(p_col));
                                }
                                if ui.small_button("Details").clicked() {
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
                                    RichText::new(format!("Check failed ({e})"))
                                        .color(Color32::LIGHT_RED),
                                );
                            }
                            None => {
                                ui.label("Not checked yet");
                            }
                        }
                    }
                });
            });

            ui.add_space(6.0);

            // Channel & Recommendation Bar
            ui.horizontal(|ui| {
                ui.label(RichText::new("Channel:").strong());
                let mut changed = false;
                for ch in [ManifestChannel::SteamRt64, ManifestChannel::Ubuntu12] {
                    if ui
                        .selectable_value(&mut self.channel, ch, ch.label())
                        .changed()
                    {
                        changed = true;
                    }
                }
                if changed {
                    self.load_releases(1, true);
                }
            });

            let rec_msg = match self.status.current_version.as_ref().map(|v| v.channel) {
                Some(ManifestChannel::Ubuntu12) => {
                    "Tip: Your Steam currently uses ubuntu12 (Legacy). Use steamrt64 (64-bit / Recommended) for current builds, or ubuntu12 for pre-Nov 2026 builds."
                }
                _ => {
                    "Tip: steamrt64 (64-bit / Recommended) is the active branch for modern 64-bit Linux. Use ubuntu12 only for legacy pre-Nov 2026 builds."
                }
            };
            ui.label(RichText::new(rec_msg).small().color(Color32::LIGHT_GRAY));

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter_text)
                        .hint_text("Search by date, SHA, or message..."),
                );
                if ui.button("Reload List").clicked() && !self.loading_releases {
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
                    .add_enabled(can_rollback, egui::Button::new("Rollback & Lock Updates"))
                    .clicked()
                {
                    self.start_rollback();
                }

                if ui
                    .add_enabled(can_unlock, egui::Button::new("Unlock Updates"))
                    .clicked()
                {
                    self.start_unlock();
                }

                if ui
                    .add_enabled(can_lock, egui::Button::new("Lock Current Version"))
                    .clicked()
                {
                    self.start_lock_only();
                }

                if self.busy {
                    ui.spinner();
                    ui.label("Working...");
                }
            });

            ui.add_space(6.0);

            // Log Output
            ui.label(RichText::new("Activity Log:").strong());
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
            ui.label(RichText::new("Available Previous Releases (Newest First):").strong());
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
                                format!("{}   (Latest)", entry.date_display)
                            } else {
                                entry.date_display.clone()
                            };
                            let hover_details = format!(
                                "Date: {}\nChannel: {}\nCommit: {}\nMessage: {}",
                                entry.date_display,
                                entry.channel.suffix(),
                                entry.sha,
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

                                let info_btn = egui::Button::new("Info")
                                    .min_size(egui::vec2(info_w, 28.0));
                                if ui
                                    .add(info_btn)
                                    .on_hover_text("View full release details")
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
                                ui.label("Loading releases from SteamTracking...");
                            });
                        } else if self.has_more {
                            if ui.button("Load Older Releases...").clicked() {
                                let next_page = self.current_page + 1;
                                self.load_releases(next_page, false);
                            }
                        }
                    });
            });
        });

        if let Some(entry) = self.details_entry.clone() {
            let mut open = true;
            egui::Window::new("Release Details")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Date:").strong());
                        ui.label(&entry.date_display);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Channel:").strong());
                        ui.label(entry.channel.suffix());
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Commit SHA:").strong());
                        ui.monospace(&entry.sha);
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("Commit Message:").strong());
                    ui.label(&entry.message);
                    ui.add_space(6.0);
                    let url = format!(
                        "https://github.com/SteamTracking/SteamTracking/commit/{}",
                        entry.sha
                    );
                    ui.hyperlink_to("View commit on GitHub", url);
                });
            if !open {
                self.details_entry = None;
            }
        }
    }
}


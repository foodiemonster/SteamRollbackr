use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Pt,
    Es,
}

impl Language {
    pub fn name(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Pt => "Português",
            Language::Es => "Español",
        }
    }

    pub fn all() -> [Language; 3] {
        [Language::En, Language::Pt, Language::Es]
    }
}

/// Draws a crisp vector flag (UK, Brazil, or Spain) without relying on system emoji fonts.
pub fn draw_flag(painter: &egui::Painter, rect: Rect, lang: Language) {
    let w = rect.width();
    let h = rect.height();
    let x0 = rect.left();
    let y0 = rect.top();
    let c = rect.center();

    match lang {
        Language::En => {
            // UK Union Jack
            let blue = Color32::from_rgb(1, 33, 105);
            let white = Color32::WHITE;
            let red = Color32::from_rgb(200, 16, 46);
            painter.rect_filled(rect, 2.0, blue);

            let diag_w = (h * 0.22).max(2.0);
            let diag_r = (h * 0.11).max(1.2);
            painter.line_segment([rect.left_top(), rect.right_bottom()], Stroke::new(diag_w, white));
            painter.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(diag_w, white));
            painter.line_segment([rect.left_top(), rect.right_bottom()], Stroke::new(diag_r, red));
            painter.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(diag_r, red));

            // White cross
            painter.rect_filled(
                Rect::from_center_size(c, Vec2::new(w, h * 0.32)),
                0.0,
                white,
            );
            painter.rect_filled(
                Rect::from_center_size(c, Vec2::new(w * 0.22, h)),
                0.0,
                white,
            );
            // Red cross
            painter.rect_filled(
                Rect::from_center_size(c, Vec2::new(w, h * 0.18)),
                0.0,
                red,
            );
            painter.rect_filled(
                Rect::from_center_size(c, Vec2::new(w * 0.12, h)),
                0.0,
                red,
            );
        }
        Language::Pt => {
            // Brazil flag
            let green = Color32::from_rgb(0, 156, 59);
            let yellow = Color32::from_rgb(255, 223, 0);
            let blue = Color32::from_rgb(0, 39, 118);

            painter.rect_filled(rect, 2.0, green);
            let diamond = vec![
                Pos2::new(c.x, y0 + h * 0.14),
                Pos2::new(x0 + w * 0.86, c.y),
                Pos2::new(c.x, y0 + h * 0.86),
                Pos2::new(x0 + w * 0.14, c.y),
            ];
            painter.add(egui::Shape::convex_polygon(diamond, yellow, Stroke::NONE));
            painter.circle_filled(c, h * 0.23, blue);
        }
        Language::Es => {
            // Spain flag
            let red = Color32::from_rgb(170, 21, 27);
            let yellow = Color32::from_rgb(241, 191, 0);
            painter.rect_filled(rect, 2.0, red);
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x0, y0 + h * 0.25), Vec2::new(w, h * 0.5)),
                0.0,
                yellow,
            );
            // Small coat-of-arms accent
            painter.rect_filled(
                Rect::from_center_size(
                    Pos2::new(x0 + w * 0.30, c.y),
                    Vec2::new(w * 0.12, h * 0.28),
                ),
                1.0,
                red,
            );
        }
    }

    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(90)));
}

pub struct Texts {
    pub refresh_status: &'static str,
    pub steam_path: &'static str,
    pub steam_not_found: &'static str,
    pub current_version: &'static str,
    pub unknown_version: &'static str,
    pub update_lock_status: &'static str,
    pub locked: &'static str,
    pub partially_locked: &'static str,
    pub unlocked: &'static str,
    pub checking_moon: &'static str,
    pub moon_updated: &'static str,
    pub moon_not_updated: &'static str,
    pub moon_incomplete: &'static str,
    pub local_pattern_ok: &'static str,
    pub local_pattern_missing: &'static str,
    pub details_btn: &'static str,
    pub check_failed: &'static str,
    pub not_checked_yet: &'static str,
    pub channel: &'static str,
    pub channel_steamrt64: &'static str,
    pub channel_ubuntu12: &'static str,
    pub tip_ubuntu12_active: &'static str,
    pub tip_steamrt64_rec: &'static str,
    pub filter: &'static str,
    pub filter_hint: &'static str,
    pub reload_list: &'static str,
    pub available_releases: &'static str,
    pub latest_badge: &'static str,
    pub info_btn: &'static str,
    pub info_hover: &'static str,
    pub loading_releases: &'static str,
    pub load_older: &'static str,
    pub rollback_and_lock: &'static str,
    pub optimize_btn: &'static str,
    pub optimize_hover: &'static str,
    pub unlock_updates: &'static str,
    pub lock_current: &'static str,
    pub working: &'static str,
    pub activity_log: &'static str,
    pub ready_log: &'static str,
    pub release_details_title: &'static str,
    pub date_label: &'static str,
    pub commit_sha_label: &'static str,
    pub commit_msg_label: &'static str,
    pub view_on_github: &'static str,
}

pub fn texts(lang: Language) -> Texts {
    match lang {
        Language::En => Texts {
            refresh_status: "Refresh Status",
            steam_path: "Steam Path:",
            steam_not_found: "Steam installation not found!",
            current_version: "Current Version:",
            unknown_version: "Unknown (no local client manifest found)",
            update_lock_status: "Update Lock Status:",
            locked: "LOCKED (Updates Blocked)",
            partially_locked: "PARTIALLY LOCKED",
            unlocked: "UNLOCKED (Updates Allowed)",
            checking_moon: "Checking steam-monitor & slsteam-moon...",
            moon_updated: "Updated for latest Steam",
            moon_not_updated: "NOT yet updated for latest Steam",
            moon_incomplete: "Pattern update check incomplete",
            local_pattern_ok: "Local client: Pattern OK",
            local_pattern_missing: "Local client: No pattern",
            details_btn: "Details",
            check_failed: "Check failed",
            not_checked_yet: "Not checked yet",
            channel: "Channel:",
            channel_steamrt64: "steamrt64 (64-bit / Recommended)",
            channel_ubuntu12: "ubuntu12 (Legacy / Pre-Nov 2026)",
            tip_ubuntu12_active: "Tip: Your Steam currently uses ubuntu12 (Legacy). Use steamrt64 (64-bit / Recommended) for current builds, or ubuntu12 for pre-Nov 2026 builds.",
            tip_steamrt64_rec: "Tip: steamrt64 (64-bit / Recommended) is the active branch for modern 64-bit Linux. Use ubuntu12 only for legacy pre-Nov 2026 builds.",
            filter: "Filter:",
            filter_hint: "Search by date, SHA, or message...",
            reload_list: "Reload List",
            available_releases: "Available Previous Releases (Newest First):",
            latest_badge: "(Latest)",
            info_btn: "Info",
            info_hover: "View full release details",
            loading_releases: "Loading releases from SteamTracking...",
            load_older: "Load Older Releases...",
            rollback_and_lock: "Rollback & Lock Updates",
            optimize_btn: "Magic Button",
            optimize_hover: "Automatically selects the newest Steam release compatible with slsteam-moon, rolls back, and locks updates.",
            unlock_updates: "Unlock Updates",
            lock_current: "Lock Current Version",
            working: "Working...",
            activity_log: "Activity Log:",
            ready_log: "Ready. Select a release and click Rollback & Lock.",
            release_details_title: "Release Details",
            date_label: "Date:",
            commit_sha_label: "Commit SHA:",
            commit_msg_label: "Commit Message:",
            view_on_github: "View commit on GitHub",
        },
        Language::Pt => Texts {
            refresh_status: "Atualizar Status",
            steam_path: "Pasta do Steam:",
            steam_not_found: "Instalação do Steam não encontrada!",
            current_version: "Versão Atual:",
            unknown_version: "Desconhecida (manifesto local não encontrado)",
            update_lock_status: "Status de Bloqueio:",
            locked: "BLOQUEADO (Atualizações Bloqueadas)",
            partially_locked: "PARCIALMENTE BLOQUEADO",
            unlocked: "DESBLOQUEADO (Atualizações Permitidas)",
            checking_moon: "Verificando steam-monitor e slsteam-moon...",
            moon_updated: "Atualizado para o Steam mais recente",
            moon_not_updated: "AINDA NÃO atualizado para o Steam mais recente",
            moon_incomplete: "Verificação de padrões incompleta",
            local_pattern_ok: "Cliente local: Padrão OK",
            local_pattern_missing: "Cliente local: Sem padrão",
            details_btn: "Detalhes",
            check_failed: "Falha na verificação",
            not_checked_yet: "Ainda não verificado",
            channel: "Canal:",
            channel_steamrt64: "steamrt64 (64-bit / Recomendado)",
            channel_ubuntu12: "ubuntu12 (Legado / Pré-Nov 2026)",
            tip_ubuntu12_active: "Dica: Seu Steam usa ubuntu12 (Legado). Use steamrt64 (64-bit / Recomendado) para versões atuais, ou ubuntu12 para versões pré-Nov 2026.",
            tip_steamrt64_rec: "Dica: steamrt64 (64-bit / Recomendado) é o canal ativo para Linux 64-bit moderno. Use ubuntu12 apenas para versões antigas pré-Nov 2026.",
            filter: "Filtro:",
            filter_hint: "Buscar por data, SHA ou mensagem...",
            reload_list: "Recarregar Lista",
            available_releases: "Versões Anteriores Disponíveis (Mais Recentes Primeiro):",
            latest_badge: "(Mais recente)",
            info_btn: "Info",
            info_hover: "Ver detalhes completos da versão",
            loading_releases: "Carregando versões do SteamTracking...",
            load_older: "Carregar Versões Anteriores...",
            rollback_and_lock: "Reverter e Bloquear Atualizações",
            optimize_btn: "Botão Mágico",
            optimize_hover: "Seleciona automaticamente a versão mais recente do Steam compatível com slsteam-moon, reverte e bloqueia atualizações.",
            unlock_updates: "Desbloquear Atualizações",
            lock_current: "Bloquear Versão Atual",
            working: "Processando...",
            activity_log: "Registro de Atividades:",
            ready_log: "Pronto. Selecione uma versão e clique em Reverter e Bloquear.",
            release_details_title: "Detalhes da Versão",
            date_label: "Data:",
            commit_sha_label: "SHA do Commit:",
            commit_msg_label: "Mensagem do Commit:",
            view_on_github: "Ver commit no GitHub",
        },
        Language::Es => Texts {
            refresh_status: "Actualizar Estado",
            steam_path: "Ruta de Steam:",
            steam_not_found: "¡Instalación de Steam no encontrada!",
            current_version: "Versión Actual:",
            unknown_version: "Desconocida (no se encontró manifiesto local)",
            update_lock_status: "Estado de Bloqueo:",
            locked: "BLOQUEADO (Actualizaciones Bloqueadas)",
            partially_locked: "PARCIALMENTE BLOQUEADO",
            unlocked: "DESBLOQUEADO (Actualizaciones Permitidas)",
            checking_moon: "Comprobando steam-monitor y slsteam-moon...",
            moon_updated: "Actualizado para el último Steam",
            moon_not_updated: "AÚN NO actualizado para el último Steam",
            moon_incomplete: "Comprobación de patrones incompleta",
            local_pattern_ok: "Cliente local: Patrón OK",
            local_pattern_missing: "Cliente local: Sin patrón",
            details_btn: "Detalles",
            check_failed: "Error al comprobar",
            not_checked_yet: "Sin comprobar aún",
            channel: "Canal:",
            channel_steamrt64: "steamrt64 (64-bit / Recomendado)",
            channel_ubuntu12: "ubuntu12 (Heredado / Pre-Nov 2026)",
            tip_ubuntu12_active: "Consejo: Tu Steam usa ubuntu12 (Heredado). Usa steamrt64 (64-bit / Recomendado) para versiones actuales, o ubuntu12 para versiones pre-Nov 2026.",
            tip_steamrt64_rec: "Consejo: steamrt64 (64-bit / Recomendado) es la rama activa para Linux de 64 bits moderno. Usa ubuntu12 solo para versiones antiguas pre-Nov 2026.",
            filter: "Filtro:",
            filter_hint: "Buscar por fecha, SHA o mensaje...",
            reload_list: "Recargar Lista",
            available_releases: "Versiones Anteriores Disponibles (Más Recientes Primero):",
            latest_badge: "(Última)",
            info_btn: "Info",
            info_hover: "Ver detalles completos de la versión",
            loading_releases: "Cargando versiones desde SteamTracking...",
            load_older: "Cargar Versiones Anteriores...",
            rollback_and_lock: "Revertir y Bloquear Actualizaciones",
            optimize_btn: "Botón Mágico",
            optimize_hover: "Selecciona automáticamente la versión más reciente de Steam compatible con slsteam-moon, revierte y bloquea actualizaciones.",
            unlock_updates: "Desbloquear Actualizaciones",
            lock_current: "Bloquear Versión Actual",
            working: "Trabajando...",
            activity_log: "Registro de Actividad:",
            ready_log: "Listo. Selecciona una versión y pulsa Revertir y Bloquear.",
            release_details_title: "Detalles de la Versión",
            date_label: "Fecha:",
            commit_sha_label: "SHA del Commit:",
            commit_msg_label: "Mensaje del Commit:",
            view_on_github: "Ver commit en GitHub",
        },
    }
}


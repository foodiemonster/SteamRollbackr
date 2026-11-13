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
            let red = Color32::from_rgb(170, 21, 27);
            let yellow = Color32::from_rgb(241, 191, 0);
            painter.rect_filled(rect, 2.0, red);
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x0, y0 + h * 0.25), Vec2::new(w, h * 0.5)),
                0.0,
                yellow,
            );
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

    painter.rect_stroke(rect, 2.0, Stroke::new(1.0_f32, Color32::from_gray(90)));
}

pub struct Texts {
    pub nav_home: &'static str,
    pub nav_advanced: &'static str,
    pub nav_logs: &'static str,
    pub restart_steam: &'static str,
    pub refresh_status: &'static str,
    pub banner_compatible: &'static str,
    pub banner_incompatible: &'static str,
    pub banner_checking: &'static str,
    pub magic_rollback: &'static str,
    pub magic_desc: &'static str,
    pub unlock_and_update: &'static str,
    pub unlock_and_update_desc: &'static str,
    pub system_status: &'static str,
    pub steam_client: &'static str,
    pub steam_path: &'static str,
    pub steam_not_found: &'static str,
    pub unknown_version: &'static str,
    pub locked_pill: &'static str,
    pub partial_pill: &'static str,
    pub unlocked_pill: &'static str,
    pub installed_pill: &'static str,
    pub not_found_pill: &'static str,
    pub local_pattern_ok: &'static str,
    pub local_pattern_missing: &'static str,
    pub open_repo_hover: &'static str,
    pub check_failed: &'static str,
    pub advanced_title: &'static str,
    pub advanced_subtitle: &'static str,
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
    pub unlock_updates: &'static str,
    pub lock_current: &'static str,
    pub working: &'static str,
    pub logs_title: &'static str,
    pub copy_logs: &'static str,
    pub clear_logs: &'static str,
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
            nav_home: "Home",
            nav_advanced: "Advanced",
            nav_logs: "Logs",
            restart_steam: "Restart Steam",
            refresh_status: "Refresh Status",
            banner_compatible: "slsteam-moon is now compatible with the newest Steam client — feel free to unlock and update!",
            banner_incompatible: "slsteam-moon is not yet updated for the newest Steam client — stay locked or use Magic Rollback.",
            banner_checking: "Checking slsteam-moon & steam-monitor compatibility...",
            magic_rollback: "Magic Rollback",
            magic_desc: "Auto-picks the newest compatible Steam build, rolls back, and locks updates.",
            unlock_and_update: "Unlock & Update Steam",
            unlock_and_update_desc: "Removes the update lock and updates Steam to the latest official release.",
            system_status: "System Status",
            steam_client: "Steam Client",
            steam_path: "Steam Path",
            steam_not_found: "Steam installation not found",
            unknown_version: "Unknown",
            locked_pill: "Locked",
            partial_pill: "Partial Lock",
            unlocked_pill: "Unlocked",
            installed_pill: "Installed",
            not_found_pill: "Not found",
            local_pattern_ok: "Pattern OK",
            local_pattern_missing: "No Pattern",
            open_repo_hover: "Open project page on GitHub",
            check_failed: "Check failed",
            advanced_title: "Advanced Rollback",
            advanced_subtitle: "Manually pick a Steam release channel and build, or manage update locks.",
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
            unlock_updates: "Unlock Only",
            lock_current: "Lock Current Version",
            working: "Working...",
            logs_title: "Activity Logs",
            copy_logs: "Copy Logs",
            clear_logs: "Clear",
            ready_log: "Ready. Use Magic Rollback on Home or pick a version in Advanced.",
            release_details_title: "Release Details",
            date_label: "Date:",
            commit_sha_label: "Commit SHA:",
            commit_msg_label: "Commit Message:",
            view_on_github: "View commit on GitHub",
        },
        Language::Pt => Texts {
            nav_home: "Início",
            nav_advanced: "Avançado",
            nav_logs: "Registros",
            restart_steam: "Reiniciar Steam",
            refresh_status: "Atualizar Status",
            banner_compatible: "slsteam-moon já é compatível com o Steam mais recente — pode desbloquear e atualizar!",
            banner_incompatible: "slsteam-moon ainda não foi atualizado para o Steam mais recente — mantenha bloqueado ou use Magic Rollback.",
            banner_checking: "Verificando compatibilidade do slsteam-moon e steam-monitor...",
            magic_rollback: "Magic Rollback",
            magic_desc: "Escolhe automaticamente a versão mais recente compatível do Steam, reverte e bloqueia atualizações.",
            unlock_and_update: "Desbloquear e Atualizar Steam",
            unlock_and_update_desc: "Remove o bloqueio e atualiza o Steam para a versão oficial mais recente.",
            system_status: "Status do Sistema",
            steam_client: "Cliente Steam",
            steam_path: "Pasta do Steam",
            steam_not_found: "Instalação do Steam não encontrada",
            unknown_version: "Desconhecida",
            locked_pill: "Bloqueado",
            partial_pill: "Parcial",
            unlocked_pill: "Desbloqueado",
            installed_pill: "Instalado",
            not_found_pill: "Não encontrado",
            local_pattern_ok: "Padrão OK",
            local_pattern_missing: "Sem Padrão",
            open_repo_hover: "Abrir página do projeto no GitHub",
            check_failed: "Falha ao verificar",
            advanced_title: "Reversão Avançada",
            advanced_subtitle: "Escolha manualmente o canal e a versão do Steam, ou gerencie bloqueios.",
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
            rollback_and_lock: "Reverter e Bloquear",
            unlock_updates: "Apenas Desbloquear",
            lock_current: "Bloquear Versão Atual",
            working: "Processando...",
            logs_title: "Registro de Atividades",
            copy_logs: "Copiar",
            clear_logs: "Limpar",
            ready_log: "Pronto. Use Magic Rollback no Início ou escolha uma versão em Avançado.",
            release_details_title: "Detalhes da Versão",
            date_label: "Data:",
            commit_sha_label: "SHA do Commit:",
            commit_msg_label: "Mensagem do Commit:",
            view_on_github: "Ver commit no GitHub",
        },
        Language::Es => Texts {
            nav_home: "Inicio",
            nav_advanced: "Avanzado",
            nav_logs: "Registros",
            restart_steam: "Reiniciar Steam",
            refresh_status: "Actualizar Estado",
            banner_compatible: "slsteam-moon ya es compatible con el último Steam — ¡puedes desbloquear y actualizar!",
            banner_incompatible: "slsteam-moon aún no se ha actualizado para el último Steam — mantén el bloqueo o usa Magic Rollback.",
            banner_checking: "Comprobando compatibilidad de slsteam-moon y steam-monitor...",
            magic_rollback: "Magic Rollback",
            magic_desc: "Elige automáticamente la versión más reciente compatible de Steam, revierte y bloquea actualizaciones.",
            unlock_and_update: "Desbloquear y Actualizar Steam",
            unlock_and_update_desc: "Quita el bloqueo y actualiza Steam a la última versión oficial.",
            system_status: "Estado del Sistema",
            steam_client: "Cliente Steam",
            steam_path: "Ruta de Steam",
            steam_not_found: "Instalación de Steam no encontrada",
            unknown_version: "Desconocida",
            locked_pill: "Bloqueado",
            partial_pill: "Parcial",
            unlocked_pill: "Desbloqueado",
            installed_pill: "Instalado",
            not_found_pill: "No encontrado",
            local_pattern_ok: "Patrón OK",
            local_pattern_missing: "Sin Patrón",
            open_repo_hover: "Abrir página del proyecto en GitHub",
            check_failed: "Error al comprobar",
            advanced_title: "Reversión Avanzada",
            advanced_subtitle: "Elige manualmente el canal y la versión de Steam, o gestiona los bloqueos.",
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
            rollback_and_lock: "Revertir y Bloquear",
            unlock_updates: "Solo Desbloquear",
            lock_current: "Bloquear Versión Actual",
            working: "Trabajando...",
            logs_title: "Registro de Actividad",
            copy_logs: "Copiar",
            clear_logs: "Limpiar",
            ready_log: "Listo. Usa Magic Rollback en Inicio o elige una versión en Avanzado.",
            release_details_title: "Detalles de la Versión",
            date_label: "Fecha:",
            commit_sha_label: "SHA del Commit:",
            commit_msg_label: "Mensaje del Commit:",
            view_on_github: "Ver commit en GitHub",
        },
    }
}

use crate::config::{self, AppConfig};
use crate::i18n::{Language, Translations};
use crate::icons::Icons;
use crate::model::{format_size, DirNode, DriveInfo, ScanResult};
use crate::scanner::{self, ScanHandle};
use eframe::egui;
use egui_phosphor::regular;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use sysinfo::Disks;

/// Максимальное количество отображаемых детей узла дерева
const MAX_VISIBLE_CHILDREN: usize = 200;

pub struct CedarApp {
    root_node: Option<DirNode>,
    selected_path: Option<PathBuf>,
    scan_path: String,
    is_scanning: bool,
    scan: Option<ScanHandle>,
    available_drives: Vec<DriveInfo>,
    last_scan_duration: Option<Duration>,
    last_scan_size: u64,
    scan_speed_mbps: f64,
    config: AppConfig,
    translations: Translations,
    show_about_window: bool,
    show_delete_confirm: bool,
    path_to_delete: Option<PathBuf>,
    status_message: Option<String>,
    status_message_time: Option<Instant>,
    icons: Icons,
}

impl CedarApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Загружаем конфигурацию из хранилища eframe (старый способ,
        // для совместимости) или из файла конфигурации
        let config: AppConfig = if let Some(storage) = cc.storage {
            storage
                .get_string("config")
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(config::load_config)
        } else {
            config::load_config()
        };

        let translations = Translations::load(config.language);

        let mut drives = Vec::new();
        let disks = Disks::new_with_refreshed_list();

        for disk in disks.list() {
            if let Some(path) = disk.mount_point().to_str() {
                let name = disk.name().to_string_lossy().to_string();
                drives.push(DriveInfo {
                    path: path.to_string(),
                    name: if name.is_empty() {
                        path.to_string()
                    } else {
                        name
                    },
                    total_space: disk.total_space(),
                    available_space: disk.available_space(),
                    kind: format!("{:?}", disk.kind()),
                });
            }
        }

        let default_path = config
            .last_path
            .clone()
            .or_else(|| drives.first().map(|d| d.path.clone()))
            .unwrap_or_else(|| {
                #[cfg(windows)]
                {
                    String::from("C:\\")
                }
                #[cfg(not(windows))]
                {
                    String::from("/")
                }
            });

        let icons = Icons::new(&cc.egui_ctx, config.dark_mode);

        Self {
            root_node: None,
            selected_path: None,
            scan_path: default_path,
            is_scanning: false,
            scan: None,
            available_drives: drives,
            last_scan_duration: None,
            last_scan_size: 0,
            scan_speed_mbps: 0.0,
            config,
            translations,
            show_about_window: false,
            show_delete_confirm: false,
            path_to_delete: None,
            status_message: None,
            status_message_time: None,
            icons,
        }
    }

    fn set_language(&mut self, lang: Language) {
        self.config.language = lang;
        self.translations = Translations::load(lang);
    }

    /// Забирает результат завершившегося скана, если он готов
    fn poll_scan_result(&mut self, ctx: &egui::Context) {
        if !self.is_scanning {
            return;
        }

        let Some(scan) = &self.scan else {
            self.is_scanning = false;
            return;
        };

        let mut result = scanner::lock(&scan.result);
        let Some(scan_result) = result.take() else {
            return; // ещё сканируется
        };

        self.is_scanning = false;
        match scan_result {
            ScanResult::Complete(node, duration) => {
                self.last_scan_size = node.size;
                self.last_scan_duration = Some(duration);
                self.root_node = Some(node);

                // Рассчитываем скорость сканирования
                let secs = duration.as_secs_f64();
                if secs > 0.0 {
                    let size_mb = self.last_scan_size as f64 / (1024.0 * 1024.0);
                    self.scan_speed_mbps = size_mb / secs;
                }
            }
            ScanResult::Cancelled => {
                self.last_scan_duration = None;
                self.last_scan_size = 0;
                self.scan_speed_mbps = 0.0;
            }
            ScanResult::Error(err) => {
                self.last_scan_duration = None;
                self.last_scan_size = 0;
                self.scan_speed_mbps = 0.0;
                self.status_message = Some(format!(
                    "{}: {}",
                    self.translations.get("scan_error"),
                    err
                ));
                self.status_message_time = Some(Instant::now());
                eprintln!("Scan error: {}", err);
            }
        }

        ctx.request_repaint();
    }

    fn remove_from_tree(&mut self, path: &Path) {
        let Some(root) = &mut self.root_node else {
            return;
        };

        // Проверяем, не удаляем ли корневую папку
        if root.path == path {
            self.root_node = None;
            self.selected_path = None;
        } else {
            root.remove_child(path);
            // Если удалённый элемент был выбран, снимаем выделение
            if self.selected_path.as_deref() == Some(path) {
                self.selected_path = None;
            }
        }
    }

    fn start_scan(&mut self, path: String) {
        self.is_scanning = true;
        self.root_node = None;
        // Новый ScanHandle на каждый запуск: старый поток не может
        // повлиять на новый скан (см. комментарии в scanner.rs)
        self.scan = Some(scanner::start_scan(path, self.translations.clone()));
    }

    fn stop_scan(&mut self) {
        if let Some(scan) = &self.scan {
            scan.cancel.store(true, Ordering::Relaxed);
            crate::scanner::lock(&scan.progress).message =
                self.translations.get("scan_cancelled");
        }
        self.is_scanning = false;
    }
}

impl eframe::App for CedarApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Сохраняем последний путь
        self.config.last_path = Some(self.scan_path.clone());

        // Сохраняем конфигурацию в storage (для совместимости)
        if let Ok(json) = serde_json::to_string(&self.config) {
            storage.set_string("config", json);
        }

        // Также сохраняем в файл (основной способ)
        config::save_config(&self.config);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Применяем тему
        if self.config.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        self.show_menu_bar(ctx);
        self.show_top_panel(ctx);
        self.show_central_panel(ctx);
        self.show_bottom_panel(ctx);

        self.poll_scan_result(ctx);

        // Проверяем, нужно ли показать диалог удаления
        if self.path_to_delete.is_some() && !self.show_delete_confirm {
            self.show_delete_confirm = true;
        }

        self.show_delete_dialog(ctx);
        self.show_about_window(ctx);

        // Автоматически скрываем статусное сообщение через 5 секунд
        if let Some(time) = self.status_message_time {
            if time.elapsed().as_secs() > 5 {
                self.status_message = None;
                self.status_message_time = None;
            }
        }

        if self.is_scanning {
            ctx.request_repaint();
        }
    }
}

// Методы, связанные с отображением
impl CedarApp {
    fn show_menu_bar(&mut self, ctx: &egui::Context) {
        let menu_text = self.translations.get("menu");
        let app_title = self.translations.get("app_title");
        let home_page_text = self.translations.get("home_page");
        let light_theme_text = self.translations.get("light_theme");
        let dark_theme_text = self.translations.get("dark_theme");
        let language_text = self.translations.get("language");
        let about_text = self.translations.get("about");

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let current_lang = self.config.language;
                let is_dark = self.config.dark_mode;

                ui.menu_button(format!("{} {}", regular::LIST, menu_text), |ui| {
                    // Главная страница
                    if ui
                        .button(format!("{} {}", regular::HOUSE, home_page_text))
                        .clicked()
                    {
                        self.root_node = None;
                        self.selected_path = None;
                        ui.close_menu();
                    }

                    ui.separator();

                    // Выбор темы
                    if ui
                        .button(if is_dark {
                            format!("{} {}", regular::SUN, light_theme_text)
                        } else {
                            format!("{} {}", regular::MOON_STARS, dark_theme_text)
                        })
                        .clicked()
                    {
                        self.config.dark_mode = !self.config.dark_mode;
                        self.icons.reload(ctx, self.config.dark_mode);
                        // Принудительно сохраняем настройки в файл
                        config::save_config(&self.config);
                        ui.close_menu();
                    }

                    ui.separator();

                    // Выбор языка
                    ui.menu_button(format!("{} {}", regular::TRANSLATE, language_text), |ui| {
                        for lang in Language::all() {
                            if ui
                                .selectable_label(current_lang == lang, lang.name())
                                .clicked()
                            {
                                self.set_language(lang);
                                // Принудительно сохраняем настройки в файл
                                config::save_config(&self.config);
                                ui.close_menu();
                            }
                        }
                    });

                    ui.separator();

                    if ui
                        .button(format!("{} {}", regular::INFO, about_text))
                        .clicked()
                    {
                        self.show_about_window = true;
                        ui.close_menu();
                    }
                });

                ui.separator();
                ui.heading(app_title);
            });
        });
    }

    fn show_top_panel(&mut self, ctx: &egui::Context) {
        let path_label = self.translations.get("path");
        let browse_label = self.translations.get("browse");
        let scan_label = self.translations.get("scan");
        let stop_label = self.translations.get("stop");
        let files_label = self.translations.get("files");
        let dirs_label = self.translations.get("directories");
        let scanned_label = self.translations.get("scanned");
        let disk_label = self.translations.get("disk");
        let type_label = self.translations.get("type");
        let threads_label = self.translations.get("threads");
        let scanning_label = self.translations.get("scanning_label");
        let calculating_label = self.translations.get("calculating");

        let is_scanning = self.is_scanning;
        let scan = self.scan.clone();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label(path_label);

                // Находим текущий диск для отображения
                let drives = self.available_drives.clone();
                let current_display = drives
                    .iter()
                    .find(|d| d.path == self.scan_path)
                    .map(|d| format!("{} ({})", d.path, format_size(d.total_space)))
                    .unwrap_or_else(|| self.scan_path.clone());

                egui::ComboBox::from_label("")
                    .selected_text(current_display)
                    .show_ui(ui, |ui| {
                        for drive in &drives {
                            let label = format!(
                                "{} ({}) [{}]",
                                drive.path,
                                format_size(drive.total_space),
                                drive.kind
                            );
                            ui.selectable_value(&mut self.scan_path, drive.path.clone(), label);
                        }
                    });

                ui.text_edit_singleline(&mut self.scan_path);

                if ui
                    .button(format!("{} {}", regular::FOLDER_OPEN, browse_label))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.scan_path = path.display().to_string();
                    }
                }

                // Кнопка сканирования с SVG иконкой
                ui.add_enabled_ui(!is_scanning, |ui| {
                    let button = egui::Button::image_and_text(
                        egui::Image::new(&self.icons.search).max_size(egui::vec2(16.0, 16.0)),
                        &scan_label,
                    )
                    .min_size(egui::vec2(140.0, 0.0));
                    let response = ui.add(button);
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if response.clicked() {
                        let path = self.scan_path.clone();
                        self.start_scan(path);
                    }
                });

                // Кнопка остановки с SVG иконкой
                ui.add_enabled_ui(is_scanning, |ui| {
                    let button = egui::Button::image_and_text(
                        egui::Image::new(&self.icons.stop).max_size(egui::vec2(16.0, 16.0)),
                        &stop_label,
                    )
                    .min_size(egui::vec2(140.0, 0.0));
                    let response = ui.add(button);
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if response.clicked() {
                        self.stop_scan();
                    }
                });
            });

            if is_scanning {
                if let Some(scan) = &scan {
                    let progress = scanner::lock(&scan.progress);
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(&progress.message);
                    });

                    // Progress details
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} {}: {}",
                            regular::FILE, files_label, progress.files_scanned
                        ));
                        ui.separator();
                        ui.label(format!(
                            "{} {}: {}",
                            regular::FOLDER, dirs_label, progress.dirs_scanned
                        ));
                        ui.separator();
                        ui.label(format!(
                            "{} {}: {}",
                            regular::HARD_DRIVE,
                            scanned_label,
                            format_size(progress.total_size)
                        ));
                    });

                    ui.horizontal(|ui| {
                        if progress.disk_size > 0 {
                            ui.label(format!(
                                "{} {}: {}",
                                regular::DATABASE,
                                disk_label,
                                format_size(progress.disk_size)
                            ));
                            ui.separator();
                        }
                        if !progress.disk_type.is_empty() {
                            ui.label(format!(
                                "{} {}: {}",
                                regular::DISC, type_label, progress.disk_type
                            ));
                            ui.separator();
                        }
                        ui.label(format!(
                            "{} {}: {}",
                            regular::CPU, threads_label, progress.thread_count
                        ));
                    });

                    // Current path
                    if !progress.current_path.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} {}:", regular::FOLDER, scanning_label));
                            ui.label(&progress.current_path);
                        });
                    }

                    // Visual progress bar with real percentage
                    let available_width = ui.available_width();
                    let progress_value = if progress.disk_size > 0 {
                        (progress.total_size as f32 / progress.disk_size as f32).min(1.0)
                    } else {
                        0.0
                    };

                    let progress_text = if progress.disk_size > 0 {
                        format!("{:.1}%", progress_value * 100.0)
                    } else {
                        calculating_label
                    };

                    ui.add(
                        egui::ProgressBar::new(progress_value)
                            .text(progress_text)
                            .desired_width(available_width),
                    );
                }
            }

            ui.add_space(5.0);
        });
    }

    fn show_central_panel(&mut self, ctx: &egui::Context) {
        let available_drives_label = self.translations.get("available_drives");
        let scan_label = self.translations.get("scan");
        let available_label = self.translations.get("available");
        let dark_mode = self.config.dark_mode;
        let mut drive_scan_request: Option<String> = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.root_node.is_some() {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if let Some(root) = &mut self.root_node {
                            render_tree_node(
                                ui,
                                root,
                                0,
                                &mut self.selected_path,
                                &mut self.path_to_delete,
                                &self.icons,
                                &self.translations,
                            );
                        }
                    });
            } else if !self.is_scanning {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add_space(20.0);
                        ui.heading(format!(
                            "{} {}",
                            regular::HARD_DRIVE, available_drives_label
                        ));
                        ui.add_space(10.0);

                        // Отображаем диски в виде блоков (карточек) вертикально
                        let drives = self.available_drives.clone();

                        for drive in &drives {
                            if render_drive_card(
                                ui,
                                drive,
                                dark_mode,
                                &scan_label,
                                &available_label,
                                &mut self.scan_path,
                            ) {
                                drive_scan_request = Some(drive.path.clone());
                            }

                            ui.add_space(8.0);
                        }

                        ui.add_space(20.0);
                    });
            }
        });

        // Запуск скана после отрисовки (внутри замыкания панели
        // &mut self уже занят заимствованиями полей)
        if let Some(path) = drive_scan_request {
            self.start_scan(path);
        }
    }

    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
        let selected_label = self.translations.get("selected");
        let no_selection_label = self.translations.get("no_selection");
        let total_size_label = self.translations.get("total_size");
        let sata_efficiency = self.translations.get("sata_efficiency");
        let speed_tooltip = self.translations.get("scan_speed_tooltip");

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                // Показываем статусное сообщение или выбранный путь
                if let Some(status) = &self.status_message {
                    ui.label(status);
                } else if let Some(path) = &self.selected_path {
                    ui.label(format!("{}: {}", selected_label, path.display()));
                } else {
                    ui.label(no_selection_label);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Анализ производительности
                    if self.scan_speed_mbps > 0.0 {
                        ui.separator();

                        // Типичные скорости SSD для сравнения (MB/s);
                        // цифры для подсказки лежат в переводах
                        const TYPICAL_SSD_SPEED: f64 = 500.0; // SATA SSD

                        let efficiency_percent =
                            (self.scan_speed_mbps / TYPICAL_SSD_SPEED * 100.0).min(100.0);

                        let speed_color = if self.scan_speed_mbps > 200.0 {
                            egui::Color32::GREEN
                        } else if self.scan_speed_mbps > 100.0 {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::LIGHT_RED
                        };

                        ui.colored_label(
                            speed_color,
                            format!("⚡ {:.1} MB/s", self.scan_speed_mbps),
                        );

                        // Показываем эффективность
                        ui.label(
                            sata_efficiency.replace("%d", &format!("{:.0}", efficiency_percent)),
                        )
                        .on_hover_text(
                            speed_tooltip.replace("%d", &format!("{:.1}", self.scan_speed_mbps)),
                        );
                    }

                    if let Some(duration) = self.last_scan_duration {
                        ui.separator();
                        ui.label(format!("⏱ {:.2}s", duration.as_secs_f64()));
                    }

                    if let Some(root) = &self.root_node {
                        ui.separator();
                        ui.label(format!("{}: {}", total_size_label, format_size(root.size)));
                    }
                });
            });
        });
    }

    fn show_delete_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_delete_confirm {
            return;
        }

        let Some(path) = self.path_to_delete.clone() else {
            return;
        };

        let path_display = path.display().to_string();

        let title = self.translations.get("delete_confirm_title");
        let question = self.translations.get("delete_confirm_msg");
        let warning = self.translations.get("delete_warning");
        let restore = self.translations.get("delete_restore");
        let delete_label = self.translations.get("delete");
        let cancel_label = self.translations.get("cancel");

        let mut delete_confirmed = false;
        let mut cancelled = false;

        let red_color = egui::Color32::from_rgb(198, 61, 53); // #c63d35

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(10.0);

                    ui.label(question);
                    ui.add_space(5.0);
                    ui.label(egui::RichText::new(&path_display).strong());
                    ui.add_space(10.0);

                    ui.label(warning);
                    ui.label(restore);

                    ui.add_space(15.0);

                    ui.horizontal(|ui| {
                        // Красная кнопка удаления
                        let delete_button = egui::Button::new(
                            egui::RichText::new(format!("{} {}", regular::TRASH, delete_label))
                                .color(egui::Color32::WHITE),
                        )
                        .fill(red_color);

                        if ui.add(delete_button).clicked() {
                            delete_confirmed = true;
                        }

                        if ui
                            .button(format!("{} {}", regular::X, cancel_label))
                            .clicked()
                        {
                            cancelled = true;
                        }
                    });

                    ui.add_space(10.0);
                });
            });

        if delete_confirmed {
            // Выполняем удаление
            let deleted_label = self.translations.get("deleted_success");
            let error_label = self.translations.get("delete_error");

            match trash::delete(&path) {
                Ok(_) => {
                    // Удаляем из дерева
                    self.remove_from_tree(&path);
                    self.status_message =
                        Some(format!("{}: {}", deleted_label, path_display));
                }
                Err(e) => {
                    self.status_message = Some(format!("{}: {}", error_label, e));
                }
            }
            self.status_message_time = Some(Instant::now());
            self.show_delete_confirm = false;
            self.path_to_delete = None;
        }

        if cancelled {
            self.show_delete_confirm = false;
            self.path_to_delete = None;
        }
    }

    fn show_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about_window {
            return;
        }

        let title = self.translations.get("about_title");
        let description = if cfg!(windows) {
            self.translations.get("about_description")
        } else {
            self.translations.get("about_description_cross")
        };
        let version = self
            .translations
            .get("version")
            .replace("%d", env!("CARGO_PKG_VERSION"));
        let language_label = self.translations.get("language_label");
        let gui_label = self.translations.get("gui_label");
        let features = self.translations.get("features");
        let feature_scan = self.translations.get("feature_scan");
        let feature_tree = self.translations.get("feature_tree");
        let feature_detect = self.translations.get("feature_detect");
        let feature_multithread = self.translations.get("feature_multithread");
        let feature_speed = self.translations.get("feature_speed");
        let created_with = self.translations.get("created_with");
        let on_rust = self.translations.get("on_rust");
        let close = self.translations.get("close");

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);

                    // Иконка кедра
                    ui.add(
                        egui::Image::new(&self.icons.cedar).max_size(egui::vec2(80.0, 80.0)),
                    );
                    ui.add_space(5.0);

                    ui.heading("Cedar Folder Size Analyzer");
                    ui.add_space(10.0);

                    ui.label(description);
                    ui.add_space(10.0);

                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(version);
                    ui.label(format!("{} {}", regular::CODE, language_label));
                    ui.label(format!("{} {}", regular::PALETTE, gui_label));
                    ui.add_space(5.0);

                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(format!("{} {}", regular::SPARKLE, features));
                    ui.label(format!(
                        "  {} {}",
                        regular::LIGHTNING, feature_scan
                    ));
                    ui.label(format!("  {} {}", regular::TREE_STRUCTURE, feature_tree));
                    ui.label(format!("  {} {}", regular::HARD_DRIVES, feature_detect));
                    ui.label(format!("  {} {}", regular::CPU, feature_multithread));
                    ui.label(format!("  {} {}", regular::GAUGE, feature_speed));
                    ui.add_space(10.0);

                    ui.separator();
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.label(created_with);
                        ui.label(egui::RichText::new(regular::HEART).color(egui::Color32::RED));
                        ui.label(on_rust);
                    });

                    ui.add_space(10.0);

                    if ui.button(format!("{} {}", regular::X, close)).clicked() {
                        self.show_about_window = false;
                    }

                    ui.add_space(10.0);
                });
            });
    }
}

fn render_tree_node(
    ui: &mut egui::Ui,
    node: &mut DirNode,
    depth: usize,
    selected_path: &mut Option<PathBuf>,
    path_to_delete: &mut Option<PathBuf>,
    icons: &Icons,
    t: &Translations,
) {
    let indent = depth as f32 * 24.0; // Увеличили отступ для лучшей читаемости

    ui.horizontal(|ui| {
        ui.add_space(indent);

        let has_children = !node.children.is_empty();

        // Кнопка раскрытия только для папок с детьми
        if !node.is_file && has_children {
            // Используем иконки phosphor
            let expand_icon = if node.is_expanded {
                regular::CARET_DOWN
            } else {
                regular::CARET_RIGHT
            };

            if ui.button(expand_icon).clicked() {
                node.is_expanded = !node.is_expanded;
            }
        } else {
            ui.add_space(24.0);
        }

        // Иконка: всегда папка для папок, файл для файлов
        let icon_texture = if node.is_file { &icons.file } else { &icons.folder };

        let size_str = format_size(node.size);

        // Отображаем иконку как изображение с фиксированным размером
        ui.add(egui::Image::new(icon_texture).max_size(egui::vec2(16.0, 16.0)));

        let label = format!("{} - {}", node.name, size_str);

        let response = ui.selectable_label(selected_path.as_ref() == Some(&node.path), label);

        // Одиночный клик - выбор
        if response.clicked() {
            *selected_path = Some(node.path.clone());
        }

        // Двойной клик - раскрытие/свёртывание (только для папок с детьми)
        if !node.is_file && has_children && response.double_clicked() {
            node.is_expanded = !node.is_expanded;
        }

        // Контекстное меню (правый клик)
        response.context_menu(|ui| {
            if ui
                .button(format!("{} {}", regular::TRASH, t.get("delete_to_trash")))
                .clicked()
            {
                *path_to_delete = Some(node.path.clone());
                ui.close_menu();
            }

            if ui
                .button(format!("{} {}", regular::FOLDER_OPEN, t.get("open_in_explorer")))
                .clicked()
            {
                if let Err(e) = open::that(&node.path) {
                    eprintln!("Failed to open path: {}", e);
                }
                ui.close_menu();
            }

            if ui
                .button(format!("{} {}", regular::COPY, t.get("copy_path")))
                .clicked()
            {
                ui.output_mut(|o| o.copied_text = node.path.display().to_string());
                ui.close_menu();
            }
        });

        response.on_hover_text(node.path.display().to_string());
    });

    if node.is_expanded {
        let total_children = node.children.len();

        // Показываем только первые MAX_VISIBLE_CHILDREN элементов
        for child in node.children.iter_mut().take(MAX_VISIBLE_CHILDREN) {
            render_tree_node(
                ui,
                child,
                depth + 1,
                selected_path,
                path_to_delete,
                icons,
                t,
            );
        }

        // Если элементов больше, показываем индикатор
        if total_children > MAX_VISIBLE_CHILDREN {
            let hidden_count = total_children - MAX_VISIBLE_CHILDREN;
            let child_indent = (depth + 1) as f32 * 20.0;

            let hidden_label = t
                .get("more_items")
                .replace("%d", &hidden_count.to_string());

            ui.horizontal(|ui| {
                ui.add_space(child_indent);
                ui.add_space(20.0); // Вместо стрелки
                ui.label(
                    egui::RichText::new(hidden_label)
                        .italics()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_drive_card(
    ui: &mut egui::Ui,
    drive: &DriveInfo,
    dark_mode: bool,
    scan_label: &str,
    available_label: &str,
    scan_path: &mut String,
) -> bool {
    // Карточка для диска
    let used_space = drive.total_space.saturating_sub(drive.available_space);
    let usage_percent = if drive.total_space > 0 {
        (used_space as f64 / drive.total_space as f64) as f32
    } else {
        0.0
    };

    // Определяем иконку в зависимости от типа диска
    let disk_icon = if drive.kind.contains("SSD") {
        regular::HARD_DRIVES
    } else if drive.kind.contains("Removable") {
        regular::USB
    } else {
        regular::HARD_DRIVE
    };

    // Цвета для progressbar
    let progress_color = if usage_percent > 0.9 {
        egui::Color32::from_rgb(136, 0, 21) // Красный #880015
    } else if usage_percent > 0.75 {
        egui::Color32::from_rgb(255, 127, 39) // Оранжевый #FF7F27
    } else if dark_mode {
        egui::Color32::from_rgb(60, 130, 80) // Приглушенный зелёный
    } else {
        egui::Color32::from_rgb(80, 150, 100)
    };

    // Рамка для карточки - более компактная
    let scan_clicked = std::cell::Cell::new(false);
    egui::Frame::none()
        .fill(if dark_mode {
            egui::Color32::from_gray(35)
        } else {
            egui::Color32::from_rgb(230, 230, 230) // #E6E6E6
        })
        .stroke(egui::Stroke::new(
            1.0,
            if dark_mode {
                egui::Color32::from_gray(60)
            } else {
                egui::Color32::from_gray(220)
            },
        ))
        .rounding(6.0)
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            // Горизонтальное разделение на левую и правую часть
            ui.horizontal(|ui| {
                // ЛЕВАЯ ЧАСТЬ - информация о диске
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(disk_icon).size(20.0));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(egui::RichText::new(&drive.path).strong().size(16.0));
                            ui.label(
                                egui::RichText::new(&drive.name)
                                    .size(11.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    });

                    ui.add_space(4.0);

                    // Тип диска
                    ui.label(
                        egui::RichText::new(format!("{} {}", regular::DISC, drive.kind))
                            .size(11.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                });

                ui.add_space(20.0);

                // ПРАВАЯ ЧАСТЬ - статистика и кнопка
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Кнопка сканирования - компактная
                    let base_color = if dark_mode {
                        egui::Color32::from_rgb(50, 100, 180)
                    } else {
                        egui::Color32::from_rgb(70, 130, 200)
                    };

                    // Настраиваем стиль кнопки
                    let mut button_style = ui.style_mut().clone();
                    button_style.visuals.widgets.inactive.bg_fill = base_color;
                    button_style.visuals.widgets.inactive.weak_bg_fill = base_color;
                    button_style.visuals.widgets.inactive.bg_stroke =
                        egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);

                    // При наведении фон темнее на 20%
                    let hovered_color = egui::Color32::from_rgb(
                        (base_color.r() as f32 * 0.8) as u8,
                        (base_color.g() as f32 * 0.8) as u8,
                        (base_color.b() as f32 * 0.8) as u8,
                    );
                    button_style.visuals.widgets.hovered.bg_fill = hovered_color;
                    button_style.visuals.widgets.hovered.weak_bg_fill = hovered_color;
                    button_style.visuals.widgets.hovered.bg_stroke =
                        egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);

                    // При нажатии еще темнее
                    let active_color = egui::Color32::from_rgb(
                        (base_color.r() as f32 * 0.7) as u8,
                        (base_color.g() as f32 * 0.7) as u8,
                        (base_color.b() as f32 * 0.7) as u8,
                    );
                    button_style.visuals.widgets.active.bg_fill = active_color;
                    button_style.visuals.widgets.active.weak_bg_fill = active_color;
                    button_style.visuals.widgets.active.bg_stroke =
                        egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);

                    ui.set_style(button_style);

                    let button = egui::Button::new(
                        egui::RichText::new(format!(
                            "{} {}",
                            regular::MAGNIFYING_GLASS, scan_label
                        ))
                        .color(egui::Color32::WHITE),
                    )
                    .min_size(egui::vec2(120.0, 32.0));

                    let response = ui.add(button);
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if response.clicked() {
                        *scan_path = drive.path.clone();
                        scan_clicked.set(true);
                    }

                    ui.add_space(10.0);

                    // Статистика и progressbar
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;

                        // Progress bar с рамкой
                        // Цвет текста: белый на красном/оранжевом фоне, черный на зеленом
                        let text_color = if usage_percent > 0.75 {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::BLACK
                        };

                        let progress = egui::ProgressBar::new(usage_percent)
                            .text(
                                egui::RichText::new(format!("{:.1}%", usage_percent * 100.0))
                                    .color(text_color)
                                    .size(11.0),
                            )
                            .fill(progress_color)
                            .desired_width(200.0);

                        // Применяем стиль с рамкой
                        ui.style_mut().visuals.widgets.inactive.bg_stroke =
                            egui::Stroke::new(
                                1.0,
                                if dark_mode {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::BLACK
                                },
                            );
                        ui.style_mut().visuals.widgets.active.bg_stroke =
                            egui::Stroke::new(
                                1.0,
                                if dark_mode {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::BLACK
                                },
                            );

                        ui.add(progress);

                        // Информация о размерах - компактно
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(
                                egui::RichText::new(format_size(used_space))
                                    .size(11.0)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new("/")
                                    .size(11.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                            ui.label(
                                egui::RichText::new(format_size(drive.total_space))
                                    .size(11.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                            ui.label(
                                egui::RichText::new("•")
                                    .size(11.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} {}",
                                    format_size(drive.available_space),
                                    available_label
                                ))
                                .size(11.0)
                                .color(if dark_mode {
                                    egui::Color32::from_rgb(100, 180, 120)
                                } else {
                                    egui::Color32::from_rgb(60, 140, 80)
                                }),
                            );
                        });
                    });
                });
            });
        });

    scan_clicked.get()
}

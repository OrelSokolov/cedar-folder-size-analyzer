use eframe::egui;
use egui_phosphor::regular;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::Disks;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Baobab-RS - Disk Usage Analyzer"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Baobab-RS",
        options,
        Box::new(|cc| {
            // Загружаем шрифт Phosphor
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            
            // Настройка стиля для увеличения размеров элементов
            let mut style = (*cc.egui_ctx.style()).clone();
            
            // Увеличиваем размер текста
            style.text_styles = [
                (egui::TextStyle::Small, egui::FontId::new(12.0, egui::FontFamily::Proportional)),
                (egui::TextStyle::Body, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
                (egui::TextStyle::Button, egui::FontId::new(16.0, egui::FontFamily::Proportional)),
                (egui::TextStyle::Heading, egui::FontId::new(20.0, egui::FontFamily::Proportional)),
                (egui::TextStyle::Monospace, egui::FontId::new(14.0, egui::FontFamily::Monospace)),
            ].into();
            
            // Увеличиваем отступы и размеры элементов
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.spacing.button_padding = egui::vec2(8.0, 4.0);
            style.spacing.indent = 20.0;
            style.spacing.interact_size = egui::vec2(50.0, 24.0);
            
            cc.egui_ctx.set_style(style);
            
            Ok(Box::new(BaobabApp::default()))
        }),
    )
}

#[derive(Clone)]
struct DirNode {
    path: PathBuf,
    name: String,
    size: u64,
    children: Vec<DirNode>,
    is_expanded: bool,
    is_file: bool,  // true если это файл, false если папка
}

impl DirNode {
    fn new(path: PathBuf, name: String, size: u64, is_file: bool) -> Self {
        Self {
            path,
            name,
            size,
            children: Vec::new(),
            is_expanded: false,
            is_file,
        }
    }

    fn sort_by_size(&mut self) {
        self.children.sort_by(|a, b| b.size.cmp(&a.size));
        for child in &mut self.children {
            child.sort_by_size();
        }
    }
}

#[derive(Clone)]
struct ScanProgress {
    message: String,
    current_path: String,
    files_scanned: usize,
    dirs_scanned: usize,
    total_size: u64,
    disk_size: u64,
    disk_type: String,
    thread_count: usize,
}

impl Default for ScanProgress {
    fn default() -> Self {
        Self {
            message: String::new(),
            current_path: String::new(),
            files_scanned: 0,
            dirs_scanned: 0,
            total_size: 0,
            disk_size: 0,
            disk_type: String::new(),
            thread_count: 1,
        }
    }
}

enum ScanResult {
    InProgress,
    Complete(DirNode),
    Cancelled,
    Error(String),
}

struct DriveInfo {
    path: String,
    size: u64,
    kind: String,
}

struct BaobabApp {
    root_node: Option<DirNode>,
    selected_path: Option<PathBuf>,
    scan_path: String,
    is_scanning: bool,
    scan_progress: Arc<Mutex<ScanProgress>>,
    scan_result: Arc<Mutex<Option<ScanResult>>>,
    scan_cancel: Arc<AtomicBool>,
    available_drives: Vec<DriveInfo>,
    last_scan_duration: Option<Duration>,
    last_scan_size: u64,
    scan_speed_mbps: f64,
    dark_mode: bool,
    show_about_window: bool,
    show_delete_confirm: bool,
    path_to_delete: Option<PathBuf>,
    status_message: Option<String>,
    status_message_time: Option<Instant>,
}

impl Default for BaobabApp {
    fn default() -> Self {
        let mut drives = Vec::new();
        let disks = Disks::new_with_refreshed_list();
        
        for disk in disks.list() {
            if let Some(path) = disk.mount_point().to_str() {
                drives.push(DriveInfo {
                    path: path.to_string(),
                    size: disk.total_space(),
                    kind: format!("{:?}", disk.kind()),
                });
            }
        }
        
        let default_path = if !drives.is_empty() { 
            drives[0].path.clone()
        } else { 
            String::from("C:\\") 
        };
        
        Self {
            root_node: None,
            selected_path: None,
            scan_path: default_path,
            is_scanning: false,
            scan_progress: Arc::new(Mutex::new(ScanProgress::default())),
            scan_result: Arc::new(Mutex::new(None)),
            scan_cancel: Arc::new(AtomicBool::new(false)),
            available_drives: drives,
            last_scan_duration: None,
            last_scan_size: 0,
            scan_speed_mbps: 0.0,
            dark_mode: true,
            show_about_window: false,
            show_delete_confirm: false,
            path_to_delete: None,
            status_message: None,
            status_message_time: None,
        }
    }
}

impl BaobabApp {
    fn remove_from_tree(&mut self, path: &PathBuf) {
        fn remove_recursive(node: &mut DirNode, path: &PathBuf) -> bool {
            // Удаляем из детей
            node.children.retain(|child| &child.path != path);
            
            // Рекурсивно проверяем детей
            for child in &mut node.children {
                if remove_recursive(child, path) {
                    return true;
                }
            }
            
            false
        }
        
        if let Some(root) = &mut self.root_node {
            // Проверяем, не удаляем ли корневую папку
            if &root.path == path {
                self.root_node = None;
                self.selected_path = None;
            } else {
                remove_recursive(root, path);
                // Если удалённый элемент был выбран, снимаем выделение
                if self.selected_path.as_ref() == Some(path) {
                    self.selected_path = None;
                }
            }
        }
    }
    
    fn start_scan(&mut self, path: String) {
        self.is_scanning = true;
        self.root_node = None;
        self.scan_cancel.store(false, Ordering::Relaxed);
        
        let progress = self.scan_progress.clone();
        let result = self.scan_result.clone();
        let cancel = self.scan_cancel.clone();
        
        // Очищаем предыдущий результат
        *result.lock().unwrap() = None;
        
        // Получаем информацию о диске
        let (disk_size, disk_type, is_ssd) = get_disk_info(&path);
        
        {
            let mut prog = progress.lock().unwrap();
            prog.message = "Starting scan...".to_string();
            prog.current_path.clear();
            prog.files_scanned = 0;
            prog.dirs_scanned = 0;
            prog.total_size = 0;
            prog.disk_size = disk_size;
            prog.disk_type = disk_type.clone();
            prog.thread_count = if is_ssd {
                rayon::current_num_threads()
            } else {
                1
            };
        }
        
        thread::spawn(move || {
            scan_directory(&path, progress.clone(), result.clone(), cancel.clone(), is_ssd)
        });
    }
    
    fn stop_scan(&mut self) {
        self.scan_cancel.store(true, Ordering::Relaxed);
        self.is_scanning = false;
        
        let mut prog = self.scan_progress.lock().unwrap();
        prog.message = "Scan cancelled".to_string();
    }
}

fn get_disk_info(path: &str) -> (u64, String, bool) {
    let disks = Disks::new_with_refreshed_list();
    let path_buf = PathBuf::from(path);
    
    // Находим диск, на котором находится путь
    let mut best_match: Option<&sysinfo::Disk> = None;
    let mut best_match_len = 0;
    
    for disk in disks.list() {
        let mount_point = disk.mount_point();
        
        // Проверяем, начинается ли путь с точки монтирования
        if path_buf.starts_with(mount_point) {
            let mount_len = mount_point.as_os_str().len();
            if mount_len > best_match_len {
                best_match = Some(disk);
                best_match_len = mount_len;
            }
        }
    }
    
    if let Some(disk) = best_match {
        let size = disk.total_space();
        let disk_type = format!("{:?}", disk.kind());
        
        // Определяем, является ли диск SSD
        let is_ssd = matches!(disk.kind(), sysinfo::DiskKind::SSD);
        
        (size, disk_type, is_ssd)
    } else {
        (0, "Unknown".to_string(), false)
    }
}

fn get_disk_size(path: &str) -> u64 {
    get_disk_info(path).0
}

const MAX_VISIBLE_CHILDREN: usize = 200;

fn render_tree_node_static(
    ui: &mut egui::Ui,
    node: &mut DirNode,
    depth: usize,
    selected_path: &mut Option<PathBuf>,
    path_to_delete: &mut Option<PathBuf>,
) {
    let indent = depth as f32 * 24.0; // Увеличили отступ для лучшей читаемости
    
    ui.horizontal(|ui| {
        ui.add_space(indent);
        
        let has_children = !node.children.is_empty();
        
        // Кнопка раскрытия только для папок с детьми
        if !node.is_file && has_children {
            // Используем иконки phosphor
            let expand_icon = if node.is_expanded { regular::CARET_DOWN } else { regular::CARET_RIGHT };
            
            // Обычная кнопка вместо small_button для большего размера
            if ui.button(expand_icon).clicked() {
                node.is_expanded = !node.is_expanded;
            }
        } else {
            ui.add_space(24.0);
        }
        
        // Иконка: всегда папка для папок, файл для файлов
        let icon = if node.is_file { 
            regular::FILE_TEXT 
        } else { 
            regular::FOLDER 
        };
        
        let size_str = format_size(node.size);
        let label = format!("{} {} - {}", icon, node.name, size_str);
        
        let response = ui.selectable_label(
            selected_path.as_ref() == Some(&node.path),
            label,
        );
        
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
            if ui.button(format!("{} Удалить в корзину", regular::TRASH)).clicked() {
                *path_to_delete = Some(node.path.clone());
                ui.close_menu();
            }
            
            if ui.button(format!("{} Открыть в проводнике", regular::FOLDER_OPEN)).clicked() {
                if let Err(e) = open::that(&node.path) {
                    eprintln!("Failed to open path: {}", e);
                }
                ui.close_menu();
            }
            
            if ui.button(format!("{} Копировать путь", regular::COPY)).clicked() {
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
            render_tree_node_static(ui, child, depth + 1, selected_path, path_to_delete);
        }
        
        // Если элементов больше, показываем индикатор
        if total_children > MAX_VISIBLE_CHILDREN {
            let hidden_count = total_children - MAX_VISIBLE_CHILDREN;
            let child_indent = (depth + 1) as f32 * 20.0;
            
            ui.horizontal(|ui| {
                ui.add_space(child_indent);
                ui.add_space(20.0); // Вместо стрелки
                ui.label(
                    egui::RichText::new(format!("... еще {} элементов", hidden_count))
                        .italics()
                        .color(ui.visuals().weak_text_color())
                );
            });
        }
    }
}

impl eframe::App for BaobabApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Применяем тему
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
        
        // Меню-бар
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(format!("{} Меню", regular::LIST), |ui| {
                    if ui.button(if self.dark_mode { 
                        format!("{} Светлая тема", regular::SUN) 
                    } else { 
                        format!("{} Тёмная тема", regular::MOON_STARS) 
                    }).clicked() {
                        self.dark_mode = !self.dark_mode;
                        ui.close_menu();
                    }
                    
                    ui.separator();
                    
                    if ui.button(format!("{} О программе", regular::INFO)).clicked() {
                        self.show_about_window = true;
                        ui.close_menu();
                    }
                });
                
                ui.separator();
                ui.heading(format!("{} Baobab-RS - Disk Usage Analyzer", regular::TREE_STRUCTURE));
            });
        });
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            
            ui.horizontal(|ui| {
                ui.label("Path:");
                
                // Находим текущий диск для отображения
                let current_display = self.available_drives
                    .iter()
                    .find(|d| d.path == self.scan_path)
                    .map(|d| format!("{} ({})", d.path, format_size(d.size)))
                    .unwrap_or_else(|| self.scan_path.clone());
                
                egui::ComboBox::from_label("")
                    .selected_text(&current_display)
                    .show_ui(ui, |ui| {
                        for drive in &self.available_drives {
                            let label = format!("{} ({}) [{}]", 
                                drive.path, 
                                format_size(drive.size),
                                drive.kind
                            );
                            ui.selectable_value(&mut self.scan_path, drive.path.clone(), label);
                        }
                    });
                
                ui.text_edit_singleline(&mut self.scan_path);
                
                if ui.button(format!("{} Browse", regular::FOLDER_OPEN)).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.scan_path = path.display().to_string();
                    }
                }
                
                let scan_button = ui.add_enabled(
                    !self.is_scanning,
                    egui::Button::new(format!("{} Scan", regular::MAGNIFYING_GLASS)),
                );
                
                if scan_button.clicked() {
                    self.start_scan(self.scan_path.clone());
                }
                
                let stop_button = ui.add_enabled(
                    self.is_scanning,
                    egui::Button::new(format!("{} Stop", regular::STOP)),
                );
                
                if stop_button.clicked() {
                    self.stop_scan();
                }
            });
            
            if self.is_scanning {
                if let Ok(progress) = self.scan_progress.lock() {
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(&progress.message);
                    });
                    
                    // Progress details
                    ui.horizontal(|ui| {
                        ui.label(format!("{} Files: {}", regular::FILE, progress.files_scanned));
                        ui.separator();
                        ui.label(format!("{} Directories: {}", regular::FOLDER, progress.dirs_scanned));
                        ui.separator();
                        ui.label(format!("{} Scanned: {}", regular::HARD_DRIVE, format_size(progress.total_size)));
                    });
                    
                    ui.horizontal(|ui| {
                        if progress.disk_size > 0 {
                            ui.label(format!("{} Disk: {}", regular::DATABASE, format_size(progress.disk_size)));
                            ui.separator();
                        }
                        if !progress.disk_type.is_empty() {
                            ui.label(format!("{} Type: {}", regular::DISC, progress.disk_type));
                            ui.separator();
                        }
                        ui.label(format!("{} Threads: {}", regular::CPU, progress.thread_count));
                    });
                    
                    // Current path
                    if !progress.current_path.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("📂 Scanning:");
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
                        "Calculating...".to_string()
                    };
                    
                    ui.add(
                        egui::ProgressBar::new(progress_value)
                            .text(progress_text)
                            .desired_width(available_width)
                    );
                }
            }
            
            ui.add_space(5.0);
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.root_node.is_some() {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if let Some(root) = &mut self.root_node {
                            render_tree_node_static(ui, root, 0, &mut self.selected_path, &mut self.path_to_delete);
                        }
                    });
            } else if !self.is_scanning {
                ui.vertical_centered(|ui| {
                    ui.add_space(200.0);
                    ui.heading(format!("{} Select a path and click 'Scan' to begin", regular::HAND_POINTING));
                    ui.add_space(20.0);
                    ui.label("Available drives:");
                    for drive in &self.available_drives {
                        ui.label(format!("  {} {} - {} [{}]", 
                            regular::HARD_DRIVE,
                            drive.path, 
                            format_size(drive.size),
                            drive.kind
                        ));
                    }
                });
            }
        });
        
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                // Показываем статусное сообщение или выбранный путь
                if let Some(status) = &self.status_message {
                    ui.label(status);
                } else if let Some(path) = &self.selected_path {
                    ui.label(format!("Selected: {}", path.display()));
                } else {
                    ui.label("No selection");
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Анализ производительности
                    if self.scan_speed_mbps > 0.0 {
                        ui.separator();
                        
                        // Типичные скорости SSD для сравнения
                        let typical_ssd_speed = 500.0; // MB/s типичный SATA SSD
                        let nvme_speed = 3500.0; // MB/s NVMe SSD
                        
                        let efficiency_percent = (self.scan_speed_mbps / typical_ssd_speed * 100.0).min(100.0);
                        
                        let speed_color = if self.scan_speed_mbps > 200.0 {
                            egui::Color32::GREEN
                        } else if self.scan_speed_mbps > 100.0 {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::LIGHT_RED
                        };
                        
                        ui.colored_label(
                            speed_color,
                            format!("⚡ {:.1} MB/s", self.scan_speed_mbps)
                        );
                        
                        // Показываем эффективность
                        ui.label(format!("(~{:.0}% of SATA SSD)", efficiency_percent))
                            .on_hover_text(format!(
                                "Scan speed: {:.1} MB/s\n\
                                Typical SATA SSD: ~{} MB/s\n\
                                Typical NVMe SSD: ~{} MB/s\n\
                                \n\
                                Note: Scan speed limited by:\n\
                                - Metadata reading (not sequential)\n\
                                - File system overhead\n\
                                - Small file processing\n\
                                - CPU processing time",
                                self.scan_speed_mbps, typical_ssd_speed, nvme_speed
                            ));
                    }
                    
                    if let Some(duration) = self.last_scan_duration {
                        ui.separator();
                        ui.label(format!("⏱ {:.2}s", duration.as_secs_f64()));
                    }
                    
                    if let Some(root) = &self.root_node {
                        ui.separator();
                        ui.label(format!("💾 {}", format_size(root.size)));
                    }
                });
            });
        });
        
        // Check if scan is complete
        if self.is_scanning {
            if let Ok(mut result) = self.scan_result.try_lock() {
                if let Some(scan_result) = result.take() {
                    match scan_result {
                        ScanResult::Complete(node) => {
                            self.is_scanning = false;
                            self.last_scan_size = node.size;
                            self.root_node = Some(node);
                            
                            // Получаем время сканирования из прогресса
                            if let Ok(prog) = self.scan_progress.lock() {
                                if let Some(duration_str) = prog.message.strip_prefix("Complete in ") {
                                    // Парсим длительность из сообщения
                                    if let Some(secs_str) = duration_str.strip_suffix("s") {
                                        if let Ok(secs) = secs_str.parse::<f64>() {
                                            self.last_scan_duration = Some(Duration::from_secs_f64(secs));
                                            
                                            // Рассчитываем скорость сканирования
                                            if secs > 0.0 {
                                                let size_mb = self.last_scan_size as f64 / (1024.0 * 1024.0);
                                                self.scan_speed_mbps = size_mb / secs;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ScanResult::Cancelled => {
                            self.is_scanning = false;
                            self.last_scan_duration = None;
                            self.last_scan_size = 0;
                            self.scan_speed_mbps = 0.0;
                        }
                        ScanResult::Error(err) => {
                            self.is_scanning = false;
                            self.last_scan_duration = None;
                            self.last_scan_size = 0;
                            self.scan_speed_mbps = 0.0;
                            eprintln!("Scan error: {}", err);
                        }
                        ScanResult::InProgress => {
                            // Возвращаем обратно
                            *result = Some(ScanResult::InProgress);
                        }
                    }
                }
            }
            ctx.request_repaint();
        }
        
        // Проверяем, нужно ли показать диалог удаления
        if self.path_to_delete.is_some() && !self.show_delete_confirm {
            self.show_delete_confirm = true;
        }
        
        // Диалог подтверждения удаления
        if self.show_delete_confirm {
            if let Some(path) = self.path_to_delete.clone() {
                let path_display = path.display().to_string();
                
                let mut delete_confirmed = false;
                let mut cancelled = false;
                
                egui::Window::new("⚠ Подтверждение удаления")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.vertical(|ui| {
                            ui.add_space(10.0);
                            
                            ui.label("Вы действительно хотите удалить в корзину:");
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new(&path_display).strong());
                            ui.add_space(10.0);
                            
                            ui.label("⚠ Элемент будет перемещён в корзину Windows.");
                            ui.label("Вы сможете восстановить его из корзины.");
                            
                            ui.add_space(15.0);
                            
                            ui.horizontal(|ui| {
                                if ui.button(format!("{} Удалить в корзину", regular::TRASH)).clicked() {
                                    delete_confirmed = true;
                                }
                                
                                if ui.button(format!("{} Отмена", regular::X)).clicked() {
                                    cancelled = true;
                                }
                            });
                            
                            ui.add_space(10.0);
                        });
                    });
                
                if delete_confirmed {
                    // Выполняем удаление
                    match trash::delete(&path) {
                        Ok(_) => {
                            // Удаляем из дерева
                            self.remove_from_tree(&path);
                            self.status_message = Some(format!("✓ Удалено в корзину: {}", path_display));
                            self.status_message_time = Some(Instant::now());
                        }
                        Err(e) => {
                            self.status_message = Some(format!("✗ Ошибка удаления: {}", e));
                            self.status_message_time = Some(Instant::now());
                        }
                    }
                    self.show_delete_confirm = false;
                    self.path_to_delete = None;
                }
                
                if cancelled {
                    self.show_delete_confirm = false;
                    self.path_to_delete = None;
                }
            }
        }
        
        // Автоматически скрываем статусное сообщение через 5 секунд
        if let Some(time) = self.status_message_time {
            if time.elapsed().as_secs() > 5 {
                self.status_message = None;
                self.status_message_time = None;
            }
        }
        
        // Окно "О программе"
        if self.show_about_window {
            egui::Window::new("О программе")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.heading(format!("{} Baobab-RS", regular::TREE_STRUCTURE));
                        ui.add_space(10.0);
                        
                        ui.label("Анализатор использования дискового пространства");
                        ui.label("для Windows");
                        ui.add_space(10.0);
                        
                        ui.separator();
                        ui.add_space(10.0);
                        
                        ui.label("Версия: 0.1.0");
                        ui.label(format!("{} Язык: Rust", regular::CODE));
                        ui.label(format!("{} GUI: egui + phosphor", regular::PALETTE));
                        ui.add_space(5.0);
                        
                        ui.separator();
                        ui.add_space(10.0);
                        
                        ui.label(format!("{} Возможности:", regular::SPARKLE));
                        ui.label(format!("  {} Быстрое сканирование дисков и папок", regular::LIGHTNING));
                        ui.label(format!("  {} Древовидное отображение структуры", regular::TREE_STRUCTURE));
                        ui.label(format!("  {} Автоопределение типа диска (SSD/HDD)", regular::HARD_DRIVES));
                        ui.label(format!("  {} Адаптивная многопоточность", regular::CPU));
                        ui.label(format!("  {} Анализ скорости сканирования", regular::GAUGE));
                        ui.add_space(10.0);
                        
                        ui.separator();
                        ui.add_space(10.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("Создано с");
                            ui.label(egui::RichText::new(regular::HEART).color(egui::Color32::RED));
                            ui.label("на Rust");
                        });
                        
                        ui.add_space(10.0);
                        
                        if ui.button(format!("{} Закрыть", regular::X)).clicked() {
                            self.show_about_window = false;
                        }
                        
                        ui.add_space(10.0);
                    });
                });
        }
    }
}

fn scan_directory(
    path: &str,
    progress: Arc<Mutex<ScanProgress>>,
    result: Arc<Mutex<Option<ScanResult>>>,
    cancel: Arc<AtomicBool>,
    use_parallel: bool,
) {
    let start_time = Instant::now();
    let path_buf = PathBuf::from(path);
    
    if !path_buf.exists() {
        let mut prog = progress.lock().unwrap();
        prog.message = "Error: Path does not exist".to_string();
        *result.lock().unwrap() = Some(ScanResult::Error("Path does not exist".to_string()));
        return;
    }
    
    {
        let mut prog = progress.lock().unwrap();
        prog.message = if use_parallel {
            "Scanning (parallel mode)...".to_string()
        } else {
            "Scanning (single-threaded mode)...".to_string()
        };
    }
    
    // Счётчики для прогресса (атомарные для многопоточности)
    let file_count = Arc::new(AtomicUsize::new(0));
    let dir_count = Arc::new(AtomicUsize::new(0));
    let total_size = Arc::new(AtomicUsize::new(0));
    
    // Однопоточная рекурсивная функция для глубоких уровней
    fn scan_recursive_single(
        path: &Path,
        cancel: &Arc<AtomicBool>,
        file_count: &Arc<AtomicUsize>,
        dir_count: &Arc<AtomicUsize>,
        total_size: &Arc<AtomicUsize>,
    ) -> Option<DirNode> {
        // Проверка отмены
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| path.to_str().unwrap_or("Unknown"))
            .to_string();
        
        let mut node = DirNode::new(path.to_path_buf(), name, 0, false);
        let mut dir_size = 0u64;
        
        // Читаем содержимое директории
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => return Some(node),
        };
        
        let mut children = Vec::new();
        
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            
            // Используем file_type() - не следует символическим ссылкам
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            
            if file_type.is_dir() {
                // Рекурсивно сканируем подпапку
                if let Some(child_node) = scan_recursive_single(
                    &entry.path(),
                    cancel,
                    file_count,
                    dir_count,
                    total_size,
                ) {
                    dir_size += child_node.size;
                    children.push(child_node);
                    dir_count.fetch_add(1, Ordering::Relaxed);
                }
            } else if file_type.is_file() {
                // Добавляем файл как узел дерева
                if let Ok(metadata) = entry.metadata() {
                    let file_size = metadata.len();
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let file_node = DirNode::new(entry.path(), file_name, file_size, true);
                    
                    dir_size += file_size;
                    children.push(file_node);
                    file_count.fetch_add(1, Ordering::Relaxed);
                    total_size.fetch_add(file_size as usize, Ordering::Relaxed);
                }
            }
        }
        
        node.size = dir_size;
        node.children = children;
        
        Some(node)
    }
    
    // Параллельная функция для первого уровня (использует rayon)
    fn scan_recursive_parallel(
        path: &Path,
        cancel: &Arc<AtomicBool>,
        file_count: &Arc<AtomicUsize>,
        dir_count: &Arc<AtomicUsize>,
        total_size: &Arc<AtomicUsize>,
        depth: usize,
    ) -> Option<DirNode> {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| path.to_str().unwrap_or("Unknown"))
            .to_string();
        
        let mut node = DirNode::new(path.to_path_buf(), name, 0, false);
        
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => return Some(node),
        };
        
        // Собираем все записи
        let entries_vec: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        
        let mut dir_size = 0u64;
        let mut children = Vec::new();
        
        // На первых 2 уровнях используем параллелизм
        if depth < 2 {
            let results: Vec<_> = entries_vec
                .par_iter()
                .filter_map(|entry| {
                    if cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    
                    let file_type = entry.file_type().ok()?;
                    
                    if file_type.is_dir() {
                        let child = scan_recursive_parallel(
                            &entry.path(),
                            cancel,
                            file_count,
                            dir_count,
                            total_size,
                            depth + 1,
                        )?;
                        dir_count.fetch_add(1, Ordering::Relaxed);
                        Some((child.size, Some(child)))
                    } else if file_type.is_file() {
                        let metadata = entry.metadata().ok()?;
                        let file_size = metadata.len();
                        file_count.fetch_add(1, Ordering::Relaxed);
                        total_size.fetch_add(file_size as usize, Ordering::Relaxed);
                        Some((file_size, None))
                    } else {
                        None
                    }
                })
                .collect();
            
            for (size, child_opt) in results {
                dir_size += size;
                if let Some(child) = child_opt {
                    children.push(child);
                }
            }
        } else {
            // Глубже 2 уровней - однопоточно
            for entry in entries_vec {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                
                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                
                if file_type.is_dir() {
                    if let Some(child_node) = scan_recursive_single(
                        &entry.path(),
                        cancel,
                        file_count,
                        dir_count,
                        total_size,
                    ) {
                        dir_size += child_node.size;
                        children.push(child_node);
                        dir_count.fetch_add(1, Ordering::Relaxed);
                    }
                } else if file_type.is_file() {
                    if let Ok(metadata) = entry.metadata() {
                        let file_size = metadata.len();
                        dir_size += file_size;
                        file_count.fetch_add(1, Ordering::Relaxed);
                        total_size.fetch_add(file_size as usize, Ordering::Relaxed);
                    }
                }
            }
        }
        
        node.size = dir_size;
        node.children = children;
        
        Some(node)
    }
    
    // Сортировка после сканирования
    fn sort_tree(node: &mut DirNode) {
        node.children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
        for child in &mut node.children {
            sort_tree(child);
        }
    }
    
    // Поток для обновления прогресса
    let progress_clone = progress.clone();
    let file_count_clone = file_count.clone();
    let dir_count_clone = dir_count.clone();
    let total_size_clone = total_size.clone();
    let cancel_clone = cancel.clone();
    
    let progress_thread = thread::spawn(move || {
        while !cancel_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(200));
            
            let mut prog = progress_clone.lock().unwrap();
            prog.files_scanned = file_count_clone.load(Ordering::Relaxed);
            prog.dirs_scanned = dir_count_clone.load(Ordering::Relaxed);
            prog.total_size = total_size_clone.load(Ordering::Relaxed) as u64;
        }
    });
    
    // Выбираем режим сканирования в зависимости от типа диска
    let root_result = if use_parallel {
        scan_recursive_parallel(
            &path_buf,
            &cancel,
            &file_count,
            &dir_count,
            &total_size,
            0,
        )
    } else {
        scan_recursive_single(
            &path_buf,
            &cancel,
            &file_count,
            &dir_count,
            &total_size,
        )
    };
    
    // Останавливаем поток прогресса
    cancel.store(true, Ordering::Relaxed);
    let _ = progress_thread.join();
    cancel.store(false, Ordering::Relaxed);
    
    // Отправляем результат
    let elapsed = start_time.elapsed();
    
    match root_result {
        Some(mut root) => {
            // Обновляем финальную статистику
            {
                let mut prog = progress.lock().unwrap();
                prog.files_scanned = file_count.load(Ordering::Relaxed);
                prog.dirs_scanned = dir_count.load(Ordering::Relaxed);
                prog.total_size = total_size.load(Ordering::Relaxed) as u64;
                prog.message = "Sorting...".to_string();
            }
            
            // Сортируем дерево после сканирования
            sort_tree(&mut root);
            
            root.is_expanded = true;
            
            let mut prog = progress.lock().unwrap();
            prog.message = format!("Complete in {:.2}s", elapsed.as_secs_f64());
            
            *result.lock().unwrap() = Some(ScanResult::Complete(root));
        }
        None => {
            *result.lock().unwrap() = Some(ScanResult::Cancelled);
        }
    }
}

fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;
    
    if size >= TB {
        format!("{:.2} TB", size as f64 / TB as f64)
    } else if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}


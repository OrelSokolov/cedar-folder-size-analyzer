use eframe::egui;
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
        Box::new(|_cc| Ok(Box::new(BaobabApp::default()))),
    )
}

#[derive(Clone)]
struct DirNode {
    path: PathBuf,
    name: String,
    size: u64,
    children: Vec<DirNode>,
    is_expanded: bool,
}

impl DirNode {
    fn new(path: PathBuf, name: String, size: u64) -> Self {
        Self {
            path,
            name,
            size,
            children: Vec::new(),
            is_expanded: false,
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

struct BaobabApp {
    root_node: Option<DirNode>,
    selected_path: Option<PathBuf>,
    scan_path: String,
    is_scanning: bool,
    scan_progress: Arc<Mutex<ScanProgress>>,
    scan_result: Arc<Mutex<Option<ScanResult>>>,
    scan_cancel: Arc<AtomicBool>,
    available_drives: Vec<String>,
    last_scan_duration: Option<Duration>,
}

impl Default for BaobabApp {
    fn default() -> Self {
        let mut drives = Vec::new();
        let disks = Disks::new_with_refreshed_list();
        
        for disk in disks.list() {
            if let Some(name) = disk.mount_point().to_str() {
                drives.push(name.to_string());
            }
        }
        
        Self {
            root_node: None,
            selected_path: None,
            scan_path: if !drives.is_empty() { 
                drives[0].clone() 
            } else { 
                String::from("C:\\") 
            },
            is_scanning: false,
            scan_progress: Arc::new(Mutex::new(ScanProgress::default())),
            scan_result: Arc::new(Mutex::new(None)),
            scan_cancel: Arc::new(AtomicBool::new(false)),
            available_drives: drives,
            last_scan_duration: None,
        }
    }
}

impl BaobabApp {
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

fn render_tree_node_static(
    ui: &mut egui::Ui,
    node: &mut DirNode,
    depth: usize,
    selected_path: &mut Option<PathBuf>,
) {
    let indent = depth as f32 * 20.0;
    
    ui.horizontal(|ui| {
        ui.add_space(indent);
        
        let has_children = !node.children.is_empty();
        
        if has_children {
            let arrow = if node.is_expanded { "▼" } else { "▶" };
            if ui.button(arrow).clicked() {
                node.is_expanded = !node.is_expanded;
            }
        } else {
            ui.add_space(20.0);
        }
        
        let icon = if has_children { "📁" } else { "📄" };
        
        let size_str = format_size(node.size);
        let label = format!("{} {} - {}", icon, node.name, size_str);
        
        let response = ui.selectable_label(
            selected_path.as_ref() == Some(&node.path),
            label,
        );
        
        if response.clicked() {
            *selected_path = Some(node.path.clone());
        }
        
        response.on_hover_text(node.path.display().to_string());
    });
    
    if node.is_expanded {
        for child in &mut node.children {
            render_tree_node_static(ui, child, depth + 1, selected_path);
        }
    }
}

impl eframe::App for BaobabApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.heading("🌳 Baobab-RS - Disk Usage Analyzer");
            });
            ui.add_space(5.0);
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Path:");
                
                egui::ComboBox::from_label("")
                    .selected_text(&self.scan_path)
                    .show_ui(ui, |ui| {
                        for drive in &self.available_drives {
                            ui.selectable_value(&mut self.scan_path, drive.clone(), drive);
                        }
                    });
                
                ui.text_edit_singleline(&mut self.scan_path);
                
                if ui.button("📂 Browse").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.scan_path = path.display().to_string();
                    }
                }
                
                let scan_button = ui.add_enabled(
                    !self.is_scanning,
                    egui::Button::new("🔍 Scan"),
                );
                
                if scan_button.clicked() {
                    self.start_scan(self.scan_path.clone());
                }
                
                let stop_button = ui.add_enabled(
                    self.is_scanning,
                    egui::Button::new("⏹ Stop"),
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
                        ui.label(format!("📄 Files: {}", progress.files_scanned));
                        ui.separator();
                        ui.label(format!("📁 Directories: {}", progress.dirs_scanned));
                        ui.separator();
                        ui.label(format!("💾 Scanned: {}", format_size(progress.total_size)));
                    });
                    
                    ui.horizontal(|ui| {
                        if progress.disk_size > 0 {
                            ui.label(format!("📦 Disk: {}", format_size(progress.disk_size)));
                            ui.separator();
                        }
                        if !progress.disk_type.is_empty() {
                            ui.label(format!("💿 Type: {}", progress.disk_type));
                            ui.separator();
                        }
                        ui.label(format!("🧵 Threads: {}", progress.thread_count));
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
                let selected_path = self.selected_path.clone();
                
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if let Some(root) = &mut self.root_node {
                            render_tree_node_static(ui, root, 0, &mut self.selected_path);
                        }
                    });
            } else if !self.is_scanning {
                ui.vertical_centered(|ui| {
                    ui.add_space(200.0);
                    ui.heading("👈 Select a path and click 'Scan' to begin");
                    ui.add_space(20.0);
                    ui.label("Available drives:");
                    for drive in &self.available_drives {
                        ui.label(format!("  • {}", drive));
                    }
                });
            }
        });
        
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                if let Some(path) = &self.selected_path {
                    ui.label(format!("Selected: {}", path.display()));
                } else {
                    ui.label("No selection");
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(duration) = self.last_scan_duration {
                        ui.separator();
                        ui.label(format!("⏱ Scan time: {:.2}s", duration.as_secs_f64()));
                    }
                    
                    if let Some(root) = &self.root_node {
                        ui.separator();
                        ui.label(format!("💾 Total size: {}", format_size(root.size)));
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
                            self.root_node = Some(node);
                            
                            // Получаем время сканирования из прогресса
                            if let Ok(prog) = self.scan_progress.lock() {
                                if let Some(duration_str) = prog.message.strip_prefix("Complete in ") {
                                    // Парсим длительность из сообщения
                                    if let Some(secs_str) = duration_str.strip_suffix("s") {
                                        if let Ok(secs) = secs_str.parse::<f64>() {
                                            self.last_scan_duration = Some(Duration::from_secs_f64(secs));
                                        }
                                    }
                                }
                            }
                        }
                        ScanResult::Cancelled => {
                            self.is_scanning = false;
                            self.last_scan_duration = None;
                        }
                        ScanResult::Error(err) => {
                            self.is_scanning = false;
                            self.last_scan_duration = None;
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
        
        let mut node = DirNode::new(path.to_path_buf(), name, 0);
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
            
            // Используем metadata из entry (быстрее!)
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            
            if metadata.is_dir() {
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
            } else if metadata.is_file() {
                let file_size = metadata.len();
                dir_size += file_size;
                file_count.fetch_add(1, Ordering::Relaxed);
                total_size.fetch_add(file_size as usize, Ordering::Relaxed);
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
        
        let mut node = DirNode::new(path.to_path_buf(), name, 0);
        
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
                    
                    let metadata = entry.metadata().ok()?;
                    
                    if metadata.is_dir() {
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
                    } else if metadata.is_file() {
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
                
                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                
                if metadata.is_dir() {
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
                } else if metadata.is_file() {
                    let file_size = metadata.len();
                    dir_size += file_size;
                    file_count.fetch_add(1, Ordering::Relaxed);
                    total_size.fetch_add(file_size as usize, Ordering::Relaxed);
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


use eframe::egui;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use sysinfo::Disks;
use walkdir::WalkDir;

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

struct BaobabApp {
    root_node: Option<DirNode>,
    selected_path: Option<PathBuf>,
    scan_path: String,
    is_scanning: bool,
    scan_progress: Arc<Mutex<Option<String>>>,
    available_drives: Vec<String>,
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
            scan_progress: Arc::new(Mutex::new(None)),
            available_drives: drives,
        }
    }
}

impl BaobabApp {
    fn start_scan(&mut self, path: String) {
        self.is_scanning = true;
        self.root_node = None;
        
        let progress = self.scan_progress.clone();
        *progress.lock().unwrap() = Some("Starting scan...".to_string());
        
        thread::spawn(move || {
            scan_directory(&path, progress.clone())
        });
    }

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
            });
            
            if self.is_scanning {
                if let Ok(progress) = self.scan_progress.lock() {
                    if let Some(msg) = progress.as_ref() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(msg);
                        });
                    }
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
                    if let Some(root) = &self.root_node {
                        ui.label(format!("Total size: {}", format_size(root.size)));
                    }
                });
            });
        });
        
        // Check if scan is complete
        if self.is_scanning {
            if let Ok(progress) = self.scan_progress.try_lock() {
                if let Some(msg) = progress.as_ref() {
                    if msg.starts_with("COMPLETE:") {
                        self.is_scanning = false;
                        // Parse the result
                        if let Some(data) = msg.strip_prefix("COMPLETE:") {
                            if let Ok(node) = serde_json::from_str::<SerializedNode>(data) {
                                self.root_node = Some(deserialize_node(node));
                            }
                        }
                    }
                }
            }
            ctx.request_repaint();
        }
    }
}

fn scan_directory(path: &str, progress: Arc<Mutex<Option<String>>>) -> Option<DirNode> {
    let path_buf = PathBuf::from(path);
    
    if !path_buf.exists() {
        *progress.lock().unwrap() = Some(format!("Error: Path does not exist"));
        return None;
    }
    
    let mut size_map: HashMap<PathBuf, u64> = HashMap::new();
    let mut dir_children: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    
    *progress.lock().unwrap() = Some(format!("Scanning files..."));
    
    // First pass: calculate sizes
    for entry in WalkDir::new(&path_buf)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_path = entry.path().to_path_buf();
        
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                let size = metadata.len();
                size_map.insert(entry_path.clone(), size);
                
                // Add to parent's children
                if let Some(parent) = entry_path.parent() {
                    dir_children
                        .entry(parent.to_path_buf())
                        .or_insert_with(Vec::new)
                        .push(entry_path);
                }
            }
        } else if entry.file_type().is_dir() {
            size_map.entry(entry_path.clone()).or_insert(0);
            
            if let Some(parent) = entry_path.parent() {
                if parent != entry_path {
                    dir_children
                        .entry(parent.to_path_buf())
                        .or_insert_with(Vec::new)
                        .push(entry_path);
                }
            }
        }
    }
    
    *progress.lock().unwrap() = Some(format!("Calculating directory sizes..."));
    
    // Second pass: calculate directory sizes bottom-up
    let mut all_dirs: Vec<PathBuf> = size_map.keys()
        .filter(|p| p.is_dir())
        .cloned()
        .collect();
    
    all_dirs.sort_by(|a, b| b.as_os_str().len().cmp(&a.as_os_str().len()));
    
    for dir in all_dirs {
        if let Some(children) = dir_children.get(&dir) {
            let total_size: u64 = children
                .iter()
                .filter_map(|child| size_map.get(child))
                .sum();
            
            *size_map.get_mut(&dir).unwrap() += total_size;
        }
    }
    
    *progress.lock().unwrap() = Some(format!("Building tree structure..."));
    
    // Build tree structure
    fn build_tree(
        path: &Path,
        size_map: &HashMap<PathBuf, u64>,
        dir_children: &HashMap<PathBuf, Vec<PathBuf>>,
    ) -> DirNode {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| path.to_str().unwrap_or("Unknown"))
            .to_string();
        
        let size = *size_map.get(path).unwrap_or(&0);
        let mut node = DirNode::new(path.to_path_buf(), name, size);
        
        if let Some(children) = dir_children.get(path) {
            let mut child_nodes: Vec<DirNode> = children
                .iter()
                .filter(|child| child.is_dir())
                .map(|child| build_tree(child, size_map, dir_children))
                .collect();
            
            child_nodes.sort_by(|a, b| b.size.cmp(&a.size));
            node.children = child_nodes;
        }
        
        node
    }
    
    let mut root = build_tree(&path_buf, &size_map, &dir_children);
    root.is_expanded = true;
    
    // Serialize and send completion
    let serialized = serialize_node(&root);
    if let Ok(json) = serde_json::to_string(&serialized) {
        *progress.lock().unwrap() = Some(format!("COMPLETE:{}", json));
    }
    
    Some(root)
}

// Serialization helpers (since we can't send DirNode across threads directly)
#[derive(serde::Serialize, serde::Deserialize)]
struct SerializedNode {
    path: String,
    name: String,
    size: u64,
    children: Vec<SerializedNode>,
    is_expanded: bool,
}

fn serialize_node(node: &DirNode) -> SerializedNode {
    SerializedNode {
        path: node.path.display().to_string(),
        name: node.name.clone(),
        size: node.size,
        children: node.children.iter().map(serialize_node).collect(),
        is_expanded: node.is_expanded,
    }
}

fn deserialize_node(snode: SerializedNode) -> DirNode {
    DirNode {
        path: PathBuf::from(snode.path),
        name: snode.name,
        size: snode.size,
        children: snode.children.into_iter().map(deserialize_node).collect(),
        is_expanded: snode.is_expanded,
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


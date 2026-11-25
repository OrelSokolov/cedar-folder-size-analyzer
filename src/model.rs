use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone)]
pub struct DirNode {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub children: Vec<DirNode>,
    pub is_expanded: bool,
    pub is_file: bool, // true если это файл, false если папка
}

impl DirNode {
    pub fn new(path: PathBuf, name: String, size: u64, is_file: bool) -> Self {
        Self {
            path,
            name,
            size,
            children: Vec::new(),
            is_expanded: false,
            is_file,
        }
    }

    /// Сортирует дерево по убыванию размера
    pub fn sort_by_size(&mut self) {
        self.children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
        for child in &mut self.children {
            child.sort_by_size();
        }
    }

    /// Удаляет узел с путём `path` из поддерева и пересчитывает размеры
    /// всех родителей. Возвращает суммарный размер удалённого узла.
    pub(crate) fn remove_child(&mut self, path: &Path) -> u64 {
        let mut removed = 0u64;

        self.children.retain(|child| {
            if child.path == path {
                removed = child.size;
                false
            } else {
                true
            }
        });

        for child in &mut self.children {
            removed += child.remove_child(path);
        }

        self.size = self.size.saturating_sub(removed);
        removed
    }
}

#[derive(Clone, Default)]
pub struct ScanProgress {
    pub message: String,
    pub current_path: String,
    pub files_scanned: usize,
    pub dirs_scanned: usize,
    pub total_size: u64,
    pub disk_size: u64,
    pub disk_type: String,
    pub thread_count: usize,
}

pub enum ScanResult {
    Complete(DirNode, Duration),
    Cancelled,
    Error(String),
}

#[derive(Clone)]
pub struct DriveInfo {
    pub path: String,
    pub name: String,
    pub total_space: u64,
    pub available_space: u64,
    pub kind: String,
}

pub fn format_size(size: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    fn node(path: &str, size: u64, is_file: bool) -> DirNode {
        DirNode::new(PathBuf::from(path), path.to_string(), size, is_file)
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(MB), "1.00 MB");
        assert_eq!(format_size(3 * GB), "3.00 GB");
        assert_eq!(format_size(2 * TB), "2.00 TB");
    }

    #[test]
    fn test_remove_child_recomputes_parent_sizes() {
        // root/
        //   a/ (300) -> a1 (100), a2 (200)
        //   b/ (400) -> b1 (400)
        let mut root = node("/root", 700, false);
        let mut a = node("/root/a", 300, false);
        a.children.push(node("/root/a/a1", 100, true));
        a.children.push(node("/root/a/a2", 200, true));
        let mut b = node("/root/b", 400, false);
        b.children.push(node("/root/b/b1", 400, true));
        root.children.push(a);
        root.children.push(b);

        // Удаляем файл глубоко в дереве
        let removed = root.remove_child(Path::new("/root/a/a2"));
        assert_eq!(removed, 200);
        assert_eq!(root.size, 500, "размер корня должен уменьшиться");
        assert_eq!(
            root.children[0].size, 100,
            "размер /root/a должен уменьшиться"
        );
        assert_eq!(root.children[0].children.len(), 1);

        // Удаляем целую ветку
        let removed = root.remove_child(Path::new("/root/b"));
        assert_eq!(removed, 400);
        assert_eq!(root.size, 100);
        assert_eq!(root.children.len(), 1);
    }

    #[test]
    fn test_remove_missing_child_keeps_sizes() {
        let mut root = node("/root", 100, false);
        root.children.push(node("/root/x", 100, true));

        let removed = root.remove_child(Path::new("/root/nope"));
        assert_eq!(removed, 0);
        assert_eq!(root.size, 100);
    }
}

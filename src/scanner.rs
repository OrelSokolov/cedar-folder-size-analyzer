use crate::i18n::Translations;
use crate::model::{DirNode, ScanProgress, ScanResult};
use rayon::prelude::*;
#[cfg(target_os = "linux")]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::Disks;

/// Состояние одного запуска сканирования.
/// Каждому скану выдаётся свой `ScanHandle`, поэтому повторный запуск
/// не может столкнуться с завершающимся старым потоком: у них разные
/// флаги отмены и разные каналы результата.
#[derive(Clone)]
pub struct ScanHandle {
    pub progress: Arc<Mutex<ScanProgress>>,
    pub result: Arc<Mutex<Option<ScanResult>>>,
    pub cancel: Arc<AtomicBool>,
}

/// Запускает сканирование в отдельном потоке и возвращает дескриптор
/// для отслеживания прогресса, получения результата и отмены.
pub fn start_scan(path: String, translations: Translations) -> ScanHandle {
    let handle = ScanHandle {
        progress: Arc::new(Mutex::new(ScanProgress::default())),
        result: Arc::new(Mutex::new(None)),
        cancel: Arc::new(AtomicBool::new(false)),
    };

    let (disk_size, disk_type, is_ssd) = get_disk_info(&path);

    {
        let mut prog = lock(&handle.progress);
        prog.message = translations.get("starting_scan");
        prog.disk_size = disk_size;
        prog.disk_type = disk_type;
        prog.thread_count = if is_ssd {
            rayon::current_num_threads()
        } else {
            1
        };
    }

    let thread_handle = handle.clone();
    thread::spawn(move || {
        scan_directory(&path, thread_handle, translations, is_ssd);
    });

    handle
}

pub fn get_disk_info(path: &str) -> (u64, String, bool) {
    let disks = Disks::new_with_refreshed_list();
    let path_buf = PathBuf::from(path);

    // Находим диск с самой длинной точкой монтирования,
    // которая является префиксом пути
    let mut best_match: Option<&sysinfo::Disk> = None;
    let mut best_match_len = 0;

    for disk in disks.list() {
        let mount_point = disk.mount_point();

        if path_buf.starts_with(mount_point) {
            let mount_len = mount_point.as_os_str().len();
            if mount_len > best_match_len {
                best_match = Some(disk);
                best_match_len = mount_len;
            }
        }
    }

    match best_match {
        Some(disk) => {
            let size = disk.total_space();
            let disk_type = format!("{:?}", disk.kind());
            let is_ssd = matches!(disk.kind(), sysinfo::DiskKind::SSD);
            (size, disk_type, is_ssd)
        }
        None => (0, "Unknown".to_string(), false),
    }
}

/// Снимок информации о точках монтирования, собранный один раз перед
/// сканированием. Раньше /proc/mounts читался и разбирался для каждого
 /// проверяемого файла, что было основным узким местом на Linux.
struct MountSnapshot {
    /// Точки монтирования виртуальных ФС (proc, sysfs, tmpfs, ...)
    virtual_mounts: Vec<PathBuf>,
    /// Точки монтирования snap-пакетов (внутри /snap)
    snap_mounts: Vec<PathBuf>,
    /// Точки монтирования других дисков (не корень текущего сканирования)
    other_disks: Vec<PathBuf>,
}

impl MountSnapshot {
    fn new(scan_root: &Path) -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::linux(scan_root)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = scan_root;
            Self {
                virtual_mounts: Vec::new(),
                snap_mounts: Vec::new(),
                other_disks: Vec::new(),
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn linux(scan_root: &Path) -> Self {
        let mut virtual_mounts = vec![PathBuf::from("/proc"), PathBuf::from("/sys"), PathBuf::from("/dev")];
        let mut snap_mounts = Vec::new();

        if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 3 {
                    continue;
                }
                let mount_point = PathBuf::from(parts[1]);
                let fs_type = parts[2];

                const VIRTUAL_FS_TYPES: [&str; 15] = [
                    "proc", "sysfs", "devtmpfs", "tmpfs", "devpts", "cgroup", "cgroup2",
                    "pstore", "bpf", "tracefs", "debugfs", "securityfs", "hugetlbfs",
                    "mqueue", "overlay",
                ];

                if VIRTUAL_FS_TYPES.contains(&fs_type) && !virtual_mounts.contains(&mount_point) {
                    virtual_mounts.push(mount_point.clone());
                }

                // Snap-пакеты монтируются как точки монтирования внутри /snap,
                // но реальный контент лежит в .snap файлах
                if mount_point.starts_with("/snap/") && !snap_mounts.contains(&mount_point) {
                    snap_mounts.push(mount_point);
                }
            }
        }

        // Точки монтирования других дисков — всё, что не корень сканирования
        let other_disks = Disks::new_with_refreshed_list()
            .list()
            .iter()
            .map(|disk| disk.mount_point().to_path_buf())
            .filter(|mount| mount != scan_root)
            .collect();

        Self {
            virtual_mounts,
            snap_mounts,
            other_disks,
        }
    }

    fn is_virtual(&self, path: &Path) -> bool {
        self.virtual_mounts.iter().any(|mount| path.starts_with(mount))
            || self.snap_mounts.iter().any(|mount| path == mount)
    }

    /// Является ли путь точкой монтирования другого диска
    fn is_other_disk_mount(&self, path: &Path) -> bool {
        self.other_disks.iter().any(|mount| path == mount)
    }
}

struct Counters {
    files: AtomicUsize,
    dirs: AtomicUsize,
    size: AtomicU64,
    /// Шардированный набор inode для дедупликации hardlinks (Linux).
    /// Шарды уменьшают конкуренцию потоков rayon за блокировку.
    #[cfg(target_os = "linux")]
    inodes: Vec<Mutex<HashSet<u64>>>,
}

impl Counters {
    fn new() -> Self {
        Self {
            files: AtomicUsize::new(0),
            dirs: AtomicUsize::new(0),
            size: AtomicU64::new(0),
            #[cfg(target_os = "linux")]
            inodes: (0..16).map(|_| Mutex::new(HashSet::new())).collect(),
        }
    }
}

struct ScanContext {
    cancel: Arc<AtomicBool>,
    counters: Arc<Counters>,
    mounts: MountSnapshot,
    /// Каталог, обрабатываемый прямо сейчас (для отображения в UI)
    current: Arc<Mutex<String>>,
}

impl ScanContext {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Возвращает размер файла, если его нужно учитывать, или None — если
/// файл пропускается (виртуальная ФС, повторный hardlink).
///
/// Символические ссылки обрабатываются самим `file_type()`: DirEntry
/// не следует по ссылкам, поэтому `is_dir()`/`is_file()` для них ложны
/// и они отсеиваются веткой else. Отдельный symlink_metadata не нужен.
fn process_file_entry(entry: &std::fs::DirEntry, sc: &ScanContext) -> Option<u64> {
    #[cfg(target_os = "linux")]
    let entry_path = entry.path();

    let metadata = entry.metadata().ok()?;
    let file_size = metadata.len();

    #[cfg(target_os = "linux")]
    use std::os::unix::fs::MetadataExt;

    // Подстраховка: файлы >1TB в виртуальных ФС (некоторые узлы /proc и /sys
    // сообщают нереальные размеры)
    #[cfg(target_os = "linux")]
    if file_size > 1_000_000_000_000 && sc.mounts.is_virtual(&entry_path) {
        return None;
    }

    // Дедупликация hardlinks на Linux
    #[cfg(target_os = "linux")]
    if metadata.nlink() > 1 {
        let shard_index = (metadata.ino() as usize) % sc.counters.inodes.len();
        let mut seen = lock(&sc.counters.inodes[shard_index]);
        if !seen.insert(metadata.ino()) {
            // Этот файл уже посчитан через другую hardlink
            return None;
        }
    }

    // Примечание: на Windows hardlinks не дедуплицируются. Стабильного
    // способа получить file index нет (MetadataExt::file_index unstable),
    // а открытие хендла ради проверки каждого файла слишком дорого для
    // сканера. NTFS-hardlinks редки, двойной учёт незначителен.

    sc.counters.files.fetch_add(1, Ordering::Relaxed);
    sc.counters.size.fetch_add(file_size, Ordering::Relaxed);
    Some(file_size)
}

/// Обрабатывает одну запись каталога. Возвращает (размер, узел дерева):
/// для файлов узел None, для каталогов — дочернее поддерево.
fn process_entry(entry: &std::fs::DirEntry, sc: &ScanContext, depth: usize, parallel: bool) -> Option<(u64, Option<DirNode>)> {
    if sc.cancelled() {
        return None;
    }

    let entry_path = entry.path();

    if sc.mounts.is_virtual(&entry_path) {
        return None;
    }

    let file_type = entry.file_type().ok()?;

    if file_type.is_dir() {
        // Точки монтирования пропускаем на любом уровне, а не только в
        // корне: их содержимое будет посчитано при отдельном сканировании.
        // Это исключает двойной учёт (bind-маунты, subvolumes), из-за
        // которого сумма могла превысить объём диска.
        if sc.mounts.is_other_disk_mount(&entry_path) {
            return None;
        }

        let child = scan_dir(&entry_path, sc, depth + 1, parallel)?;
        sc.counters.dirs.fetch_add(1, Ordering::Relaxed);
        Some((child.size, Some(child)))
    } else if file_type.is_file() {
        let size = process_file_entry(entry, sc)?;
        let node = DirNode::new(
            entry_path,
            entry.file_name().to_string_lossy().to_string(),
            size,
            true,
        );
        Some((size, Some(node)))
    } else {
        None // симлинки и прочее
    }
}

/// Рекурсивный обход каталога. На первых двух уровнях (при `parallel`)
/// используется rayon, глубже — однопоточный обход.
fn scan_dir(path: &Path, sc: &ScanContext, depth: usize, parallel: bool) -> Option<DirNode> {
    if sc.cancelled() {
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

    // Обновляем текущий путь для отображения в UI
    *lock(&sc.current) = path.to_string_lossy().to_string();

    let entries: Vec<std::fs::DirEntry> = entries.filter_map(|e| e.ok()).collect();

    let mut children = Vec::new();
    let mut dir_size = 0u64;

    if parallel && depth < 2 {
        let results: Vec<(u64, Option<DirNode>)> = entries
            .par_iter()
            .filter_map(|entry| process_entry(entry, sc, depth, true))
            .collect();

        for (size, child) in results {
            dir_size += size;
            if let Some(child) = child {
                children.push(child);
            }
        }
    } else {
        for entry in entries {
            if sc.cancelled() {
                break;
            }

            match process_entry(&entry, sc, depth, false) {
                Some((size, child)) => {
                    dir_size += size;
                    if let Some(child) = child {
                        children.push(child);
                    }
                }
                None => continue,
            }
        }
    }

    node.size = dir_size;
    node.children = children;

    Some(node)
}

fn scan_directory(path: &str, handle: ScanHandle, translations: Translations, use_parallel: bool) {
    let start_time = Instant::now();
    let path_buf = PathBuf::from(path);

    if !path_buf.exists() {
        lock(&handle.progress).message = translations.get("error_path_not_exist");
        *lock(&handle.result) = Some(ScanResult::Error("Path does not exist".to_string()));
        return;
    }

    lock(&handle.progress).message = if use_parallel {
        translations.get("scanning_parallel")
    } else {
        translations.get("scanning_single")
    };

    let sc = ScanContext {
        cancel: handle.cancel.clone(),
        counters: Arc::new(Counters::new()),
        mounts: MountSnapshot::new(&path_buf),
        current: Arc::new(Mutex::new(String::new())),
    };

    // Поток, перекачивающий счётчики в прогресс для UI
    let done = Arc::new(AtomicBool::new(false));
    {
        let progress = handle.progress.clone();
        let counters = sc.counters.clone();
        let current = sc.current.clone();
        let cancel = handle.cancel.clone();
        let done_flag = done.clone();
        thread::spawn(move || {
            while !done_flag.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(200));
                let mut prog = lock(&progress);
                prog.files_scanned = counters.files.load(Ordering::Relaxed);
                prog.dirs_scanned = counters.dirs.load(Ordering::Relaxed);
                prog.total_size = counters.size.load(Ordering::Relaxed);
                prog.current_path = lock(&current).clone();
            }
        });
    }
    // Поток прогресса завершится по `done`; detach не мешает:
    // он живёт максимум 200мс после установки флага.

    let root_result = scan_dir(&path_buf, &sc, 0, use_parallel);

    done.store(true, Ordering::Relaxed);

    // Отменённый скан не публикуем как завершённый
    if handle.cancel.load(Ordering::Relaxed) {
        *lock(&handle.result) = Some(ScanResult::Cancelled);
        return;
    }

    match root_result {
        Some(mut root) => {
            {
                let mut prog = lock(&handle.progress);
                prog.files_scanned = sc.counters.files.load(Ordering::Relaxed);
                prog.dirs_scanned = sc.counters.dirs.load(Ordering::Relaxed);
                prog.total_size = sc.counters.size.load(Ordering::Relaxed);
                prog.message = translations.get("sorting");
            }

            root.sort_by_size();
            root.is_expanded = true;

            *lock(&handle.result) = Some(ScanResult::Complete(root, start_time.elapsed()));
        }
        None => {
            *lock(&handle.result) = Some(ScanResult::Cancelled);
        }
    }
}

/// Блокировка мьютекса, устойчивая к панике другого потока:
/// отравленный лок всё равно отдаёт данные.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_test_context(root: &Path, cancelled: bool) -> ScanContext {
        ScanContext {
            cancel: Arc::new(AtomicBool::new(cancelled)),
            counters: Arc::new(Counters::new()),
            mounts: MountSnapshot::new(root),
            current: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Тестовые каталоги нельзя создавать в /tmp: он часто смонтирован как
    /// tmpfs и корректно отбрасывается сканером как виртуальная ФС.
    /// Используем target/ внутри проекта (в .gitignore, не tmpfs).
    fn unique_tmp_dir(tag: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("scan-test")
            .join(format!(
                "{}-{}",
                std::process::id(),
                tag
            ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create tmp dir");
        dir
    }

    #[test]
    fn test_scan_dir_files_on_all_levels() {
        let root = unique_tmp_dir("tree");
        // root/f0 (10), root/a/fa (20), root/a/b/fb (40)
        fs::write(root.join("f0.txt"), vec![0u8; 10]).unwrap();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/fa.txt"), vec![0u8; 20]).unwrap();
        fs::write(root.join("a/b/fb.txt"), vec![0u8; 40]).unwrap();

        let sc = make_test_context(&root, false);
        let node = scan_dir(&root, &sc, 0, true).expect("scan must return root node");

        assert_eq!(node.size, 70, "суммарный размер");
        assert_eq!(node.children.len(), 2, "f0.txt и a/ в корне");

        let a = node.children.iter().find(|c| c.name == "a").unwrap();
        assert_eq!(a.size, 60);
        assert_eq!(
            a.children.len(),
            2,
            "файл fa.txt и папка b должны попасть в дерево"
        );

        let b = a.children.iter().find(|c| c.name == "b").unwrap();
        assert_eq!(b.children.len(), 1, "глубокий файл fb.txt в дереве");
        assert_eq!(b.children[0].name, "fb.txt");

        assert_eq!(sc.counters.files.load(Ordering::Relaxed), 3);
        assert_eq!(sc.counters.dirs.load(Ordering::Relaxed), 2);
        assert_eq!(sc.counters.size.load(Ordering::Relaxed), 70);
        assert!(
            !lock(&sc.current).is_empty(),
            "current path должен обновляться"
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_cancelled_scan_returns_none() {
        let root = unique_tmp_dir("cancel");
        fs::write(root.join("x.txt"), vec![0u8; 5]).unwrap();

        let sc = make_test_context(&root, true);
        assert!(scan_dir(&root, &sc, 0, false).is_none());
        assert_eq!(sc.counters.size.load(Ordering::Relaxed), 0);

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_symlinks_are_not_followed() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let root = unique_tmp_dir("symlink");
            fs::write(root.join("real.txt"), vec![0u8; 30]).unwrap();
            symlink(root.join("real.txt"), root.join("link.txt")).unwrap();

            let sc = make_test_context(&root, false);
            let node = scan_dir(&root, &sc, 0, true).unwrap();

            assert_eq!(node.size, 30, "симлинк не должен добавлять размер");
            assert_eq!(node.children.len(), 1, "симлинк не должен попасть в дерево");
            assert_eq!(sc.counters.files.load(Ordering::Relaxed), 1);

            fs::remove_dir_all(&root).unwrap();
        }
    }
}

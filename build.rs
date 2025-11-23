fn main() {
    // Встраиваем иконку только для Windows
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("wix/Product.ico");
        res.set("ProductName", "Cedar Folder Size Analyzer");
        res.set("FileDescription", "Cedar Folder Size Analyzer - Disk space analyzer for Windows");
        res.set("CompanyName", "Oleg Orlov");
        res.set("LegalCopyright", "Copyright (C) 2025 Oleg Orlov. Freeware.");
        res.set("OriginalFilename", "cedar-folder-size-analyzer.exe");
        
        // Устанавливаем версию (ВАЖНО для корректного обновления через MSI)
        res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, 0x0001000000000000);
        res.set_version_info(winresource::VersionInfo::FILEVERSION, 0x0001000000000000);
        res.set("ProductVersion", "0.1.0.0");
        res.set("FileVersion", "0.1.0.0");
        
        res.compile().unwrap();
    }
}


fn main() {
    // Встраиваем иконку только для Windows
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("wix/Product.ico");
        res.set("ProductName", "Cedar Folder Size Analyzer");
        res.set("FileDescription", "Cedar Folder Size Analyzer - Disk space analyzer for Windows");
        res.set("CompanyName", "Oleg Orlov");
        res.set("LegalCopyright", "Copyright (C) 2025 Oleg Orlov");
        res.compile().unwrap();
    }
}


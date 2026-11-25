use eframe::egui;

// Иконки пререндерены в PNG на этапе сборки проекта (из src/icons/*.svg),
// чтобы не тянуть в бинарник весь SVG-стек (resvg/usvg/tiny-skia).
const ICON_CEDAR: &[u8] = include_bytes!("icons/png/cedar.png");

const ICON_FOLDER_DARK: &[u8] = include_bytes!("icons/png/dark/folder.png");
const ICON_FILE_DARK: &[u8] = include_bytes!("icons/png/dark/file.png");
const ICON_SEARCH_DARK: &[u8] = include_bytes!("icons/png/dark/search.png");
const ICON_STOP_DARK: &[u8] = include_bytes!("icons/png/dark/stop.png");

const ICON_FOLDER_LIGHT: &[u8] = include_bytes!("icons/png/light/folder.png");
const ICON_FILE_LIGHT: &[u8] = include_bytes!("icons/png/light/file.png");
const ICON_SEARCH_LIGHT: &[u8] = include_bytes!("icons/png/light/search.png");
const ICON_STOP_LIGHT: &[u8] = include_bytes!("icons/png/light/stop.png");

fn decode_image(data: &[u8]) -> egui::ColorImage {
    let img = image::load_from_memory(data)
        .expect("Failed to decode icon PNG")
        .to_rgba8();
    let (width, height) = img.dimensions();
    egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], img.as_raw())
}

/// Иконка приложения для окна (нужны сырые пиксели, а не текстура egui)
pub fn load_icon() -> egui::IconData {
    let image = decode_image(ICON_CEDAR);
    egui::IconData {
        width: image.width() as u32,
        height: image.height() as u32,
        rgba: image.pixels.iter().flat_map(|p| p.to_array()).collect(),
    }
}

/// Набор текстур-иконок, зависящий от темы
pub struct Icons {
    pub folder: egui::TextureHandle,
    pub file: egui::TextureHandle,
    pub search: egui::TextureHandle,
    pub stop: egui::TextureHandle,
    pub cedar: egui::TextureHandle,
}

impl Icons {
    pub fn new(ctx: &egui::Context, dark_mode: bool) -> Self {
        let (folder, file, search, stop) = theme_icons(dark_mode);
        Self {
            folder: load_texture(ctx, "icon_folder", folder),
            file: load_texture(ctx, "icon_file", file),
            search: load_texture(ctx, "icon_search", search),
            stop: load_texture(ctx, "icon_stop", stop),
            cedar: load_texture(ctx, "icon_cedar", ICON_CEDAR),
        }
    }

    /// Перезагружает иконки при смене темы.
    /// `load_texture` с тем же именем заменяет текстуру, поэтому все
    /// существующие ссылки на неё остаются валидными.
    pub fn reload(&mut self, ctx: &egui::Context, dark_mode: bool) {
        let (folder, file, search, stop) = theme_icons(dark_mode);
        self.folder = load_texture(ctx, "icon_folder", folder);
        self.file = load_texture(ctx, "icon_file", file);
        self.search = load_texture(ctx, "icon_search", search);
        self.stop = load_texture(ctx, "icon_stop", stop);
    }
}

fn theme_icons(dark_mode: bool) -> (&'static [u8], &'static [u8], &'static [u8], &'static [u8]) {
    if dark_mode {
        (
            ICON_FOLDER_DARK,
            ICON_FILE_DARK,
            ICON_SEARCH_DARK,
            ICON_STOP_DARK,
        )
    } else {
        (
            ICON_FOLDER_LIGHT,
            ICON_FILE_LIGHT,
            ICON_SEARCH_LIGHT,
            ICON_STOP_LIGHT,
        )
    }
}

fn load_texture(ctx: &egui::Context, name: &str, data: &[u8]) -> egui::TextureHandle {
    let image = decode_image(data);
    ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
}

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod i18n;
mod icons;
mod model;
mod scanner;

use app::CedarApp;
use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    // Загружаем иконку приложения
    let icon_data = icons::load_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([1100.0, 600.0])
            .with_title("Cedar Folder Size")
            .with_icon(icon_data),
        ..Default::default()
    };

    eframe::run_native(
        "cedar_folder_size_analyzer",
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
                (
                    egui::TextStyle::Small,
                    egui::FontId::new(12.0, egui::FontFamily::Proportional),
                ),
                (
                    egui::TextStyle::Body,
                    egui::FontId::new(16.0, egui::FontFamily::Proportional),
                ),
                (
                    egui::TextStyle::Button,
                    egui::FontId::new(16.0, egui::FontFamily::Proportional),
                ),
                (
                    egui::TextStyle::Heading,
                    egui::FontId::new(20.0, egui::FontFamily::Proportional),
                ),
                (
                    egui::TextStyle::Monospace,
                    egui::FontId::new(14.0, egui::FontFamily::Monospace),
                ),
            ]
            .into();

            // Увеличиваем отступы и размеры элементов
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 8.0);
            style.spacing.indent = 20.0;
            style.spacing.interact_size = egui::vec2(50.0, 24.0);

            cc.egui_ctx.set_style(style);

            Ok(Box::new(CedarApp::new(cc)))
        }),
    )
}

use std::fs::File;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Читаем SVG файл
    let svg_data = std::fs::read("../src/icons/cedar.svg")?;
    
    // Парсим SVG
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg_data, &opt)?;
    
    // Размеры для ICO файла
    let sizes = vec![16, 32, 48, 64, 128, 256];
    let mut images = Vec::new();
    
    for &size in &sizes {
        // Создаем pixmap для рендеринга
        let mut pixmap = tiny_skia::Pixmap::new(size, size)
            .ok_or("Failed to create pixmap")?;
        
        // Заполняем прозрачным фоном
        pixmap.fill(tiny_skia::Color::TRANSPARENT);
        
        // Вычисляем масштаб
        let svg_size = tree.size();
        let scale_x = size as f32 / svg_size.width();
        let scale_y = size as f32 / svg_size.height();
        let scale = scale_x.min(scale_y);
        
        // Центрируем изображение
        let offset_x = (size as f32 - svg_size.width() * scale) / 2.0;
        let offset_y = (size as f32 - svg_size.height() * scale) / 2.0;
        
        // Создаем трансформацию
        let transform = tiny_skia::Transform::from_translate(offset_x, offset_y)
            .post_scale(scale, scale);
        
        // Рендерим SVG
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        
        // Конвертируем в PNG и сохраняем как image
        let png_data = pixmap.encode_png()?;
        let img = image::load_from_memory(&png_data)?;
        images.push(img.to_rgba8());
        
        println!("Создано изображение {}x{}", size, size);
    }
    
    // Создаем ICO файл вручную
    let ico_path = "../wix/Product.ico";
    let mut ico_file = File::create(ico_path)?;
    
    // Заголовок ICO
    ico_file.write_all(&[0, 0])?; // Reserved
    ico_file.write_all(&[1, 0])?; // Type (1 = ICO)
    ico_file.write_all(&(images.len() as u16).to_le_bytes())?; // Count
    
    let mut image_data = Vec::new();
    let mut offset = 6 + images.len() * 16; // Header + directory entries
    
    // Directory entries
    for (i, img) in images.iter().enumerate() {
        let size = sizes[i];
        let png_data = {
            use image::codecs::png::PngEncoder;
            use image::ImageEncoder;
            let mut buf = Vec::new();
            let encoder = PngEncoder::new(&mut buf);
            encoder.write_image(
                img.as_raw(),
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )?;
            buf
        };
        
        // ICONDIRENTRY
        ico_file.write_all(&[if size < 256 { size as u8 } else { 0 }])?; // Width
        ico_file.write_all(&[if size < 256 { size as u8 } else { 0 }])?; // Height
        ico_file.write_all(&[0])?; // Color count
        ico_file.write_all(&[0])?; // Reserved
        ico_file.write_all(&[1, 0])?; // Planes
        ico_file.write_all(&[32, 0])?; // Bit count
        ico_file.write_all(&(png_data.len() as u32).to_le_bytes())?; // Size
        ico_file.write_all(&(offset as u32).to_le_bytes())?; // Offset
        
        image_data.push(png_data.clone());
        offset += png_data.len();
    }
    
    // Записываем данные изображений
    for data in image_data {
        ico_file.write_all(&data)?;
    }
    
    println!("\nICO файл успешно создан: {}", ico_path);
    println!("Размеры: {:?}", sizes);
    
    Ok(())
}


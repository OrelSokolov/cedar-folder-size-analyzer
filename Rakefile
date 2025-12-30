# frozen_string_literal: true

require 'fileutils'

# Константы проекта
PROJECT_NAME = 'cedar-folder-size-analyzer'
VERSION = File.read('Cargo.toml')
              .match(/version\s*=\s*"([^"]+)"/)[1]

# Пути
TARGET_DIR = 'target'
RELEASE_DIR = File.join(TARGET_DIR, 'release')
WIX_DIR = 'wix'
WIX_TOOLS_DIR = 'wix-tools'
WIX_TARGET_DIR = File.join(TARGET_DIR, 'wix')
DEBIAN_DIR = 'debian'
DEBIAN_ICONS_DIR = File.join(DEBIAN_DIR, 'icons')
ICON_SVG = File.join('src', 'icons', 'cedar.svg')
DEB_TARGET_DIR = File.join(TARGET_DIR, 'debian')

# WiX инструменты
CANDLE = File.join(WIX_TOOLS_DIR, 'candle.exe')
LIGHT = File.join(WIX_TOOLS_DIR, 'light.exe')

namespace :build do
  desc 'Сборка standalone версии (release build)'
  task :standalone do
    puts "\n#{colorize('▶', :cyan)} Сборка standalone версии #{VERSION}...\n"
    
    # Сборка через cargo
    sh 'cargo build --release' do |ok, _res|
      if ok
        puts colorize("\n✓ Сборка завершена успешно!", :green)
        
        # Проверка исполняемого файла (разные имена для разных платформ)
        exe_paths = [
          File.join(RELEASE_DIR, "#{PROJECT_NAME}.exe"),  # Windows
          File.join(RELEASE_DIR, PROJECT_NAME)             # Linux/macOS
        ]
        
        exe_path = exe_paths.find { |path| File.exist?(path) }
        if exe_path
          file_size = File.size(exe_path) / (1024.0 * 1024.0)
          puts colorize("✓ Исполняемый файл: #{exe_path} (#{file_size.round(2)} MB)", :green)
        end
      else
        puts colorize('✗ Ошибка сборки!', :red)
        exit 1
      end
    end
  end

  desc 'Сборка MSI пакета'
  task :msi => :standalone do
    puts "\n#{colorize('▶', :cyan)} Сборка MSI пакета для версии #{VERSION}...\n"
    
    # Проверка наличия WiX tools
    unless File.exist?(CANDLE) && File.exist?(LIGHT)
      puts colorize("✗ WiX Toolset не найден в директории #{WIX_TOOLS_DIR}!", :red)
      exit 1
    end
    
    # Создание целевой директории для WiX
    FileUtils.mkdir_p(WIX_TARGET_DIR)
    
    # Путь к исходному .wxs файлу
    wxs_file = File.join(WIX_DIR, 'main.wxs')
    unless File.exist?(wxs_file)
      puts colorize("✗ Файл #{wxs_file} не найден!", :red)
      exit 1
    end
    
    # Путь к выходным файлам
    wixobj_file = File.join(WIX_TARGET_DIR, 'main.wixobj')
    msi_file = File.join(WIX_TARGET_DIR, "#{PROJECT_NAME}-#{VERSION}-x86_64.msi")
    wixpdb_file = File.join(WIX_TARGET_DIR, "#{PROJECT_NAME}-#{VERSION}-x86_64.wixpdb")
    
    # Шаг 1: Компиляция .wxs в .wixobj через candle.exe
    puts colorize('▶ Шаг 1: Компиляция WiX источника...', :cyan)
    
    candle_cmd = [
      %("#{CANDLE}"),
      '-nologo',
      '-ext', 'WixUIExtension',
      %("-dCargoTargetBinDir=#{File.absolute_path(RELEASE_DIR)}"),
      %("-dVersion=#{VERSION}"),
      '-arch', 'x64',
      '-out', %("#{wixobj_file}"),
      %("#{wxs_file}")
    ].join(' ')
    
    sh candle_cmd do |ok, _res|
      unless ok
        puts colorize('✗ Ошибка компиляции WiX!', :red)
        exit 1
      end
    end
    puts colorize('✓ WiX источник скомпилирован', :green)
    
    # Шаг 2: Линковка .wixobj в .msi через light.exe
    puts colorize('▶ Шаг 2: Создание MSI пакета...', :cyan)
    
    light_cmd = [
      %("#{LIGHT}"),
      '-nologo',
      '-ext', 'WixUIExtension',
      '-out', %("#{msi_file}"),
      '-pdbout', %("#{wixpdb_file}"),
      '-sval',  # Пропускаем валидацию ICE (Internal Consistency Evaluators)
      %("#{wixobj_file}")
    ].join(' ')
    
    sh light_cmd do |ok, _res|
      unless ok
        puts colorize('✗ Ошибка создания MSI!', :red)
        exit 1
      end
    end
    
    puts colorize("\n✓ MSI пакет успешно создан!", :green)
    
    # Вывод информации о созданных файлах
    if File.exist?(msi_file)
      file_size = File.size(msi_file) / (1024.0 * 1024.0)
      puts colorize("✓ MSI файл: #{msi_file} (#{file_size.round(2)} MB)", :green)
    end
    
    puts colorize("\n✨ Готово!\n", :green)
  end

end

namespace :deb do
  desc 'Подготовка иконок для deb пакета (конвертация SVG в PNG)'
  task :icons do
    puts "\n#{colorize('▶', :cyan)} Подготовка иконок для deb пакета...\n"
    
    # Проверка наличия исходной SVG иконки
    unless File.exist?(ICON_SVG)
      puts colorize("✗ Файл #{ICON_SVG} не найден!", :red)
      exit 1
    end
    
    # Создание директории для иконок
    FileUtils.mkdir_p(DEBIAN_ICONS_DIR)
    
    # Размеры иконок для разных разрешений
    icon_sizes = [16, 32, 48, 64, 128, 256]
    
    # Проверка наличия инструментов для конвертации
    converter = nil
    if system('which inkscape > /dev/null 2>&1')
      converter = 'inkscape'
      puts colorize('✓ Найден inkscape', :green)
    elsif system('which rsvg-convert > /dev/null 2>&1')
      converter = 'rsvg-convert'
      puts colorize('✓ Найден rsvg-convert', :green)
    else
      puts colorize('✗ Не найден ни inkscape, ни rsvg-convert!', :red)
      puts colorize('  Установите один из них:', :yellow)
      puts colorize('    sudo apt install inkscape', :yellow)
      puts colorize('    или', :yellow)
      puts colorize('    sudo apt install librsvg2-bin', :yellow)
      exit 1
    end
    
    # Конвертация SVG в PNG разных размеров
    icon_sizes.each do |size|
      png_file = File.join(DEBIAN_ICONS_DIR, "cedar-#{size}.png")
      
      if converter == 'inkscape'
        cmd = [
          'inkscape',
          '--export-type=png',
          "--export-filename=#{png_file}",
          "--export-width=#{size}",
          "--export-height=#{size}",
          ICON_SVG
        ].join(' ')
      else
        cmd = [
          'rsvg-convert',
          '-w', size.to_s,
          '-h', size.to_s,
          '-o', png_file,
          ICON_SVG
        ].join(' ')
      end
      
      sh cmd do |ok, _res|
        unless ok
          puts colorize("✗ Ошибка конвертации иконки #{size}x#{size}!", :red)
          exit 1
        end
      end
      
      puts colorize("✓ Создана иконка: cedar-#{size}.png", :green)
    end
    
    # Копирование SVG для scalable иконки
    scalable_svg = File.join(DEBIAN_ICONS_DIR, 'cedar-scalable.svg')
    FileUtils.cp(ICON_SVG, scalable_svg)
    puts colorize("✓ Скопирована scalable иконка: cedar-scalable.svg", :green)
    
    puts colorize("\n✓ Все иконки подготовлены!\n", :green)
  end

  desc 'Упаковка бинарника в deb пакет (без сборки)'
  task :build do
    puts "\n#{colorize('▶', :cyan)} Упаковка deb пакета для версии #{VERSION}...\n"
    
    # Проверка наличия cargo-deb
    unless system('which cargo-deb > /dev/null 2>&1')
      puts colorize('✗ cargo-deb не установлен!', :red)
      puts colorize('  Установите его командой:', :yellow)
      puts colorize('    cargo install cargo-deb', :yellow)
      exit 1
    end
    
    puts colorize('✓ cargo-deb найден', :green)
    
    # Проверка наличия бинарника
    binary_path = File.join(RELEASE_DIR, PROJECT_NAME)
    unless File.exist?(binary_path)
      puts colorize("✗ Бинарник не найден: #{binary_path}", :red)
      puts colorize('  Запустите сборку: rake build:standalone', :yellow)
      puts colorize('  или используйте: rake deb:rebuild', :yellow)
      exit 1
    end
    puts colorize("✓ Бинарник найден", :green)
    
    # Проверка наличия desktop файла
    desktop_file = File.join(DEBIAN_DIR, "#{PROJECT_NAME}.desktop")
    unless File.exist?(desktop_file)
      puts colorize("✗ Файл #{desktop_file} не найден!", :red)
      exit 1
    end
    puts colorize("✓ Desktop файл найден", :green)
    
    # Проверка наличия иконок
    icon_sizes = [16, 32, 48, 64, 128, 256]
    icon_sizes.each do |size|
      icon_file = File.join(DEBIAN_ICONS_DIR, "cedar-#{size}.png")
      unless File.exist?(icon_file)
        puts colorize("✗ Иконка #{icon_file} не найдена!", :red)
        puts colorize('  Запустите подготовку иконок: rake deb:icons', :yellow)
        puts colorize('  или используйте: rake deb:rebuild', :yellow)
        exit 1
      end
    end
    puts colorize("✓ Все PNG иконки найдены", :green)
    
    scalable_icon = File.join(DEBIAN_ICONS_DIR, 'cedar-scalable.svg')
    unless File.exist?(scalable_icon)
      puts colorize("✗ Scalable иконка #{scalable_icon} не найдена!", :red)
      puts colorize('  Запустите подготовку иконок: rake deb:icons', :yellow)
      puts colorize('  или используйте: rake deb:rebuild', :yellow)
      exit 1
    end
    puts colorize("✓ Scalable иконка найдена", :green)
    
    # Создание deb пакета
    puts colorize('▶ Создание deb пакета через cargo-deb...', :cyan)
    
    sh 'cargo deb' do |ok, _res|
      unless ok
        puts colorize('✗ Ошибка создания deb пакета!', :red)
        exit 1
      end
    end
    
    # Поиск созданного deb файла
    deb_files = Dir.glob(File.join(DEB_TARGET_DIR, "*.deb"))
    if deb_files.empty?
      # Также проверяем в корне target/debian
      deb_files = Dir.glob(File.join(TARGET_DIR, "*.deb"))
    end
    
    if deb_files.any?
      deb_file = deb_files.first
      file_size = File.size(deb_file) / (1024.0 * 1024.0)
      puts colorize("\n✓ Deb пакет успешно создан!", :green)
      puts colorize("✓ Deb файл: #{deb_file} (#{file_size.round(2)} MB)", :green)
    else
      puts colorize("\n⚠ Deb файл не найден в ожидаемых местах", :yellow)
      puts colorize("  Проверьте вывод cargo-deb выше", :yellow)
    end
    
    puts colorize("\n✨ Готово!\n", :green)
  end

  desc 'Полная пересборка: сборка + подготовка иконок + упаковка в deb'
  task :rebuild => ['build:standalone', :icons, :build] do
    puts colorize("\n✨ Полная пересборка завершена!\n", :green)
  end
end

# Вспомогательная функция для цветного вывода
def colorize(text, color)
  colors = {
    red: 31,
    green: 32,
    yellow: 33,
    cyan: 36,
    magenta: 35,
    white: 37
  }
  
  code = colors[color] || 37
  "\e[#{code}m#{text}\e[0m"
end

# Задача по умолчанию - показать доступные команды
task :default do
  puts colorize("\n╔═══════════════════════════════════════════════════════════════╗", :magenta)
  puts colorize("║   Cedar Folder Size Analyzer - Build System                  ║", :magenta)
  puts colorize("╚═══════════════════════════════════════════════════════════════╝\n", :magenta)
  puts "Версия: #{colorize(VERSION, :cyan)}\n\n"
  puts "Доступные команды:\n\n"
  puts "  #{colorize('rake build:standalone', :green)}  - Сборка standalone версии (release build)"
  puts "  #{colorize('rake build:msi', :green)}         - Сборка MSI пакета (включает standalone)"
  puts "  #{colorize('rake deb:icons', :green)}         - Подготовка иконок для deb пакета"
  puts "  #{colorize('rake deb:build', :green)}         - Упаковка бинарника в deb (без сборки)"
  puts "  #{colorize('rake deb:rebuild', :green)}       - Полная пересборка: сборка + иконки + deb\n\n"
  puts "Примеры использования:\n\n"
  puts "  rake build:standalone    # Собрать только бинарник"
  puts "  rake build:msi           # Собрать MSI установщик"
  puts "  rake deb:build           # Упаковать готовый бинарник в deb"
  puts "  rake deb:rebuild         # Полная пересборка deb пакета\n\n"
end


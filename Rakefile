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
        
        exe_path = File.join(RELEASE_DIR, "#{PROJECT_NAME}.exe")
        if File.exist?(exe_path)
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
  puts "  #{colorize('rake build:msi', :green)}         - Сборка MSI пакета (включает standalone)\n\n"
  puts "Примеры использования:\n\n"
  puts "  rake build:standalone    # Собрать только .exe файл"
  puts "  rake build:msi           # Собрать MSI установщик\n\n"
end


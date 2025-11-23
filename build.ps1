#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Скрипт для сборки Cedar Folder Size Analyzer

.DESCRIPTION
    Этот скрипт помогает собрать проект в различных форматах:
    - Standalone: только .exe файл
    - MSI: установочный пакет Windows Installer

.PARAMETER Target
    Цель сборки: standalone или msi

.PARAMETER Clean
    Очистить целевую директорию перед сборкой

.EXAMPLE
    .\build.ps1 -Target standalone
    
.EXAMPLE
    .\build.ps1 -Target msi
    
.EXAMPLE
    .\build.ps1 -Target msi -Clean
#>

param(
    [Parameter(Mandatory=$false, HelpMessage="Цель сборки: standalone или msi")]
    [ValidateSet('standalone', 'msi', 'help')]
    [string]$Target = 'help',
    
    [Parameter(Mandatory=$false)]
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

# Константы проекта
$ProjectName = "cedar-folder-size-analyzer"
$CargoToml = Get-Content "Cargo.toml" -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Write-Host "Ошибка: не удалось найти версию в Cargo.toml" -ForegroundColor Red
    exit 1
}

# Пути
$TargetDir = "target"
$ReleaseDir = Join-Path $TargetDir "release"
$WixDir = "wix"
$WixToolsDir = "wix-tools"
$WixTargetDir = Join-Path $TargetDir "wix"

# WiX инструменты
$Candle = Join-Path $WixToolsDir "candle.exe"
$Light = Join-Path $WixToolsDir "light.exe"

# Функции для цветного вывода
function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    Write-Host $Message -ForegroundColor $Color
}

function Write-Step {
    param([string]$Message)
    Write-ColorOutput "`n▶ $Message" "Cyan"
}

function Write-Success {
    param([string]$Message)
    Write-ColorOutput "✓ $Message" "Green"
}

function Write-Error {
    param([string]$Message)
    Write-ColorOutput "✗ $Message" "Red"
}

function Write-Header {
    Write-ColorOutput @"

╔═══════════════════════════════════════════════════════════════╗
║   Cedar Folder Size Analyzer - Build System                  ║
╚═══════════════════════════════════════════════════════════════╝
"@ "Magenta"
    Write-ColorOutput "Версия: $Version`n" "Cyan"
}

# Функция для сборки standalone версии
function Build-Standalone {
    Write-Step "Сборка standalone версии $Version..."
    
    if ($Clean) {
        Write-Step "Очистка целевой директории..."
        if (Test-Path $ReleaseDir) {
            Remove-Item -Path $ReleaseDir -Recurse -Force
            Write-Success "Целевая директория очищена"
        }
    }
    
    # Сборка через cargo
    Write-Step "Компиляция проекта..."
    cargo build --release
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Сборка завершена успешно!"
        
        $ExePath = Join-Path $ReleaseDir "$ProjectName.exe"
        if (Test-Path $ExePath) {
            $FileSize = (Get-Item $ExePath).Length / 1MB
            Write-Success "Исполняемый файл: $ExePath ($([math]::Round($FileSize, 2)) MB)"
        }
    } else {
        Write-Error "Ошибка сборки!"
        exit 1
    }
}

# Функция для сборки MSI пакета
function Build-MSI {
    # Сначала собираем standalone
    Build-Standalone
    
    Write-Step "Сборка MSI пакета для версии $Version..."
    
    # Проверка наличия WiX tools
    if (-not (Test-Path $Candle) -or -not (Test-Path $Light)) {
        Write-Error "WiX Toolset не найден в директории $WixToolsDir!"
        Write-Host "Скачайте WiX Toolset 3.11 с https://wixtoolset.org/" -ForegroundColor Yellow
        exit 1
    }
    
    # Создание целевой директории для WiX
    if (-not (Test-Path $WixTargetDir)) {
        New-Item -ItemType Directory -Path $WixTargetDir -Force | Out-Null
    }
    
    # Путь к исходному .wxs файлу
    $WxsFile = Join-Path $WixDir "main.wxs"
    if (-not (Test-Path $WxsFile)) {
        Write-Error "Файл $WxsFile не найден!"
        exit 1
    }
    
    # Путь к выходным файлам
    $WixObjFile = Join-Path $WixTargetDir "main.wixobj"
    $MsiFile = Join-Path $WixTargetDir "$ProjectName-$Version-x86_64.msi"
    $WixPdbFile = Join-Path $WixTargetDir "$ProjectName-$Version-x86_64.wixpdb"
    
    # Шаг 1: Компиляция .wxs в .wixobj через candle.exe
    Write-Step "Шаг 1: Компиляция WiX источника..."
    
    $CandleArgs = @(
        '-nologo',
        '-ext', 'WixUIExtension',
        "-dCargoTargetBinDir=$(Resolve-Path $ReleaseDir)",
        "-dVersion=$Version",
        '-arch', 'x64',
        '-out', $WixObjFile,
        $WxsFile
    )
    
    & $Candle $CandleArgs
    
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Ошибка компиляции WiX!"
        exit 1
    }
    Write-Success "WiX источник скомпилирован"
    
    # Шаг 2: Линковка .wixobj в .msi через light.exe
    Write-Step "Шаг 2: Создание MSI пакета..."
    
    $LightArgs = @(
        '-nologo',
        '-ext', 'WixUIExtension',
        '-out', $MsiFile,
        '-pdbout', $WixPdbFile,
        '-sval',  # Пропускаем валидацию ICE
        $WixObjFile
    )
    
    & $Light $LightArgs
    
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Ошибка создания MSI!"
        exit 1
    }
    
    Write-Success "`nMSI пакет успешно создан!"
    
    # Вывод информации о созданных файлах
    if (Test-Path $MsiFile) {
        $FileSize = (Get-Item $MsiFile).Length / 1MB
        Write-Success "MSI файл: $MsiFile ($([math]::Round($FileSize, 2)) MB)"
    }
    
    Write-ColorOutput "`n✨ Готово!`n" "Green"
}

# Функция для отображения справки
function Show-Help {
    Write-Header
    
    Write-ColorOutput "Доступные команды:`n" "Yellow"
    Write-Host "  .\build.ps1 -Target standalone  " -NoNewline
    Write-Host "- Сборка standalone версии (release build)" -ForegroundColor Gray
    Write-Host "  .\build.ps1 -Target msi         " -NoNewline
    Write-Host "- Сборка MSI пакета (включает standalone)" -ForegroundColor Gray
    Write-Host ""
    
    Write-ColorOutput "Опции:`n" "Yellow"
    Write-Host "  -Clean                          " -NoNewline
    Write-Host "- Очистить целевую директорию перед сборкой" -ForegroundColor Gray
    Write-Host ""
    
    Write-ColorOutput "Примеры использования:`n" "Yellow"
    Write-Host "  .\build.ps1 -Target standalone" -ForegroundColor Green
    Write-Host "    # Собрать только .exe файл`n"
    
    Write-Host "  .\build.ps1 -Target msi" -ForegroundColor Green
    Write-Host "    # Собрать MSI установщик`n"
    
    Write-Host "  .\build.ps1 -Target msi -Clean" -ForegroundColor Green
    Write-Host "    # Очистить и собрать MSI установщик`n"
    
    Write-ColorOutput "Эквивалентные rake команды:`n" "Yellow"
    Write-Host "  rake build:standalone           " -NoNewline
    Write-Host "=> .\build.ps1 -Target standalone" -ForegroundColor Cyan
    Write-Host "  rake build:msi                  " -NoNewline
    Write-Host "=> .\build.ps1 -Target msi" -ForegroundColor Cyan
    Write-Host ""
}

# Проверка, что мы в корне проекта
if (-not (Test-Path "Cargo.toml")) {
    Write-Error "Cargo.toml не найден! Запустите скрипт из корня проекта."
    exit 1
}

# Обработка команд
switch ($Target) {
    'standalone' {
        Write-Header
        Build-Standalone
    }
    'msi' {
        Write-Header
        Build-MSI
    }
    'help' {
        Show-Help
    }
    default {
        Show-Help
    }
}


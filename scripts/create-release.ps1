#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Скрипт для создания нового релиза Cedar Folder Size Analyzer

.DESCRIPTION
    Этот скрипт помогает создать новый релиз:
    1. Проверяет статус git
    2. Обновляет версию в Cargo.toml
    3. Создает коммит с изменениями
    4. Создает git тег
    5. Отправляет изменения и тег в GitLab
    
    GitLab CI автоматически соберёт релиз и создаст артефакты.

.PARAMETER Version
    Версия релиза в формате semver (например: 0.1.0, 1.0.0)

.PARAMETER Message
    Сообщение для тега (опционально)

.PARAMETER DryRun
    Режим предпросмотра без реального выполнения команд

.EXAMPLE
    .\scripts\create-release.ps1 -Version 0.2.0
    
.EXAMPLE
    .\scripts\create-release.ps1 -Version 0.2.0 -Message "Added new features"
    
.EXAMPLE
    .\scripts\create-release.ps1 -Version 0.2.0 -DryRun
#>

param(
    [Parameter(Mandatory=$true, HelpMessage="Версия релиза (например: 0.1.0)")]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,
    
    [Parameter(Mandatory=$false)]
    [string]$Message = "",
    
    [Parameter(Mandatory=$false)]
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

# Цвета для вывода
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

function Write-Warning {
    param([string]$Message)
    Write-ColorOutput "⚠ $Message" "Yellow"
}

# Проверка, что мы в корне проекта
if (-not (Test-Path "Cargo.toml")) {
    Write-Error "Cargo.toml не найден! Запустите скрипт из корня проекта."
    exit 1
}

Write-ColorOutput @"
╔═══════════════════════════════════════════════════════════════╗
║   Cedar Folder Size Analyzer - Release Creator               ║
╚═══════════════════════════════════════════════════════════════╝
"@ "Magenta"

if ($DryRun) {
    Write-Warning "РЕЖИМ ПРЕДПРОСМОТРА (DRY RUN) - команды не будут выполнены"
}

$tagName = "v$Version"

# Шаг 1: Проверка git статуса
Write-Step "Шаг 1: Проверка git статуса"
$gitStatus = git status --porcelain
if ($gitStatus -and -not $DryRun) {
    Write-Error "Рабочая директория не чистая! Закоммитьте или отмените изменения."
    Write-Host "`nИзменённые файлы:"
    git status --short
    exit 1
}
Write-Success "Рабочая директория чистая"

# Шаг 2: Проверка, что тег не существует
Write-Step "Шаг 2: Проверка существования тега"
$existingTag = git tag -l $tagName
if ($existingTag) {
    Write-Error "Тег $tagName уже существует!"
    exit 1
}
Write-Success "Тег $tagName доступен"

# Шаг 3: Проверка удалённого репозитория
Write-Step "Шаг 3: Проверка удалённого репозитория"
$remoteUrl = git config --get remote.origin.url
if (-not $remoteUrl) {
    Write-Error "Удалённый репозиторий 'origin' не настроен!"
    exit 1
}
Write-Success "Удалённый репозиторий: $remoteUrl"

# Шаг 4: Обновление версии в Cargo.toml
Write-Step "Шаг 4: Обновление версии в Cargo.toml"
$cargoContent = Get-Content "Cargo.toml" -Raw
$newCargoContent = $cargoContent -replace 'version = "\d+\.\d+\.\d+"', "version = `"$Version`""

if ($DryRun) {
    Write-Host "Будет обновлена версия в Cargo.toml на: $Version"
} else {
    $newCargoContent | Set-Content "Cargo.toml" -NoNewline
    Write-Success "Версия обновлена на $Version"
}

# Шаг 5: Обновление CHANGELOG.md
Write-Step "Шаг 5: Обновление CHANGELOG.md"
if (Test-Path "CHANGELOG.md") {
    $changelogContent = Get-Content "CHANGELOG.md" -Raw
    $date = Get-Date -Format "yyyy-MM-dd"
    
    if ($changelogContent -match '\[Unreleased\]') {
        $newChangelogContent = $changelogContent -replace '\[Unreleased\]', "[Unreleased]`n`n## [$Version] - $date"
        
        if ($DryRun) {
            Write-Host "Будет добавлена запись в CHANGELOG.md для версии $Version"
        } else {
            $newChangelogContent | Set-Content "CHANGELOG.md" -NoNewline
            Write-Success "CHANGELOG.md обновлён"
        }
    } else {
        Write-Warning "Секция [Unreleased] не найдена в CHANGELOG.md, пропускаем обновление"
    }
} else {
    Write-Warning "CHANGELOG.md не найден, пропускаем обновление"
}

# Шаг 6: Создание коммита
Write-Step "Шаг 6: Создание коммита с изменениями"
$commitMessage = "Release version $Version"

if ($DryRun) {
    Write-Host "git add Cargo.toml Cargo.lock CHANGELOG.md"
    Write-Host "git commit -m `"$commitMessage`""
} else {
    git add Cargo.toml Cargo.lock
    if (Test-Path "CHANGELOG.md") {
        git add CHANGELOG.md
    }
    git commit -m $commitMessage
    Write-Success "Изменения закоммичены"
}

# Шаг 7: Создание тега
Write-Step "Шаг 7: Создание git тега"
if (-not $Message) {
    $Message = "Release Cedar Folder Size Analyzer $Version"
}

if ($DryRun) {
    Write-Host "git tag -a $tagName -m `"$Message`""
} else {
    git tag -a $tagName -m $Message
    Write-Success "Тег $tagName создан"
}

# Шаг 8: Отправка в удалённый репозиторий
Write-Step "Шаг 8: Отправка в GitLab"

if ($DryRun) {
    Write-Host "git push origin master"
    Write-Host "git push origin $tagName"
    Write-ColorOutput "`n═══════════════════════════════════════════" "Magenta"
    Write-ColorOutput "DRY RUN завершён успешно!" "Yellow"
    Write-ColorOutput "Для реальной отправки запустите без флага -DryRun" "Yellow"
} else {
    # Запрос подтверждения
    Write-Warning "`nВнимание! Следующие действия будут выполнены:"
    Write-Host "  1. Отправка коммита в удалённый репозиторий"
    Write-Host "  2. Отправка тега $tagName"
    Write-Host "  3. Запуск GitLab CI/CD для сборки релиза"
    Write-Host ""
    
    $confirmation = Read-Host "Продолжить? (y/N)"
    
    if ($confirmation -eq 'y' -or $confirmation -eq 'Y') {
        git push origin master
        git push origin $tagName
        
        Write-ColorOutput "`n═══════════════════════════════════════════" "Magenta"
        Write-Success "Релиз $tagName успешно отправлен!"
        Write-ColorOutput "`nGitLab CI начнёт сборку автоматически." "Cyan"
        Write-ColorOutput "Проверить статус можно здесь:" "Cyan"
        Write-ColorOutput "  $remoteUrl/-/pipelines" "White"
        Write-ColorOutput "`nПосле завершения сборки релиз будет доступен здесь:" "Cyan"
        Write-ColorOutput "  $remoteUrl/-/releases/$tagName" "White"
        Write-ColorOutput "`nАртефакты релиза:" "Cyan"
        Write-Host "  • cedar-folder-size-analyzer-$Version-x86_64.msi"
        Write-Host "  • cedar-folder-size-analyzer-$Version-x86_64.zip"
        Write-Host "  • cedar-folder-size-analyzer.exe"
    } else {
        Write-Warning "`nОтменено пользователем."
        Write-Host "Для отмены изменений выполните:"
        Write-Host "  git reset --hard HEAD~1"
        Write-Host "  git tag -d $tagName"
        exit 0
    }
}

Write-ColorOutput "`n✨ Готово!`n" "Green"


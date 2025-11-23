# GitLab API - Примеры использования

Полезные примеры использования GitLab API для автоматизации работы с релизами и CI/CD.

## 🔑 Аутентификация

Создайте Personal Access Token в GitLab:
1. Settings → Access Tokens
2. Создайте токен с правами: `api`, `read_repository`, `write_repository`
3. Сохраните токен

```powershell
# Сохраните токен в переменную окружения
$env:GITLAB_TOKEN = "your-token-here"
$env:GITLAB_PROJECT_ID = "your-project-id"  # Например: 12345678
```

## 📦 Работа с релизами

### Получить список релизов

```powershell
$headers = @{
    "PRIVATE-TOKEN" = $env:GITLAB_TOKEN
}

$releases = Invoke-RestMethod -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/releases" -Headers $headers

# Вывести информацию о релизах
$releases | ForEach-Object {
    Write-Host "$($_.name) - $($_.tag_name) - Released: $($_.released_at)"
}
```

### Получить информацию о конкретном релизе

```powershell
$tagName = "v0.1.0"
$release = Invoke-RestMethod -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/releases/$tagName" -Headers $headers

# Вывести информацию
Write-Host "Name: $($release.name)"
Write-Host "Tag: $($release.tag_name)"
Write-Host "Description: $($release.description)"

# Список артефактов
Write-Host "`nAssets:"
$release.assets.links | ForEach-Object {
    Write-Host "  - $($_.name): $($_.url)"
}
```

### Создать релиз вручную (через API)

```powershell
$body = @{
    name = "Cedar Folder Size Analyzer v0.2.0"
    tag_name = "v0.2.0"
    description = "New release with awesome features"
} | ConvertTo-Json

$release = Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/releases" `
    -Method Post `
    -Headers $headers `
    -ContentType "application/json" `
    -Body $body

Write-Host "Release created: $($release.tag_name)"
```

### Обновить описание релиза

```powershell
$tagName = "v0.2.0"
$body = @{
    description = @"
## 🌲 Cedar Folder Size Analyzer v0.2.0

### ✨ New Features
- Feature 1
- Feature 2

### 🐛 Bug Fixes
- Fixed bug 1
- Fixed bug 2

### 📦 Downloads
See assets below.
"@
} | ConvertTo-Json

Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/releases/$tagName" `
    -Method Put `
    -Headers $headers `
    -ContentType "application/json" `
    -Body $body

Write-Host "Release description updated"
```

### Удалить релиз

```powershell
$tagName = "v0.2.0-beta"

Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/releases/$tagName" `
    -Method Delete `
    -Headers $headers

Write-Host "Release $tagName deleted"
```

## 🔄 Работа с Pipelines

### Получить список pipelines

```powershell
$pipelines = Invoke-RestMethod -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines" -Headers $headers

# Показать последние 5 pipelines
$pipelines | Select-Object -First 5 | ForEach-Object {
    Write-Host "ID: $($_.id) | Status: $($_.status) | Ref: $($_.ref) | Created: $($_.created_at)"
}
```

### Получить детали pipeline

```powershell
$pipelineId = 12345
$pipeline = Invoke-RestMethod -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines/$pipelineId" -Headers $headers

Write-Host "Pipeline #$($pipeline.id)"
Write-Host "Status: $($pipeline.status)"
Write-Host "Ref: $($pipeline.ref)"
Write-Host "Duration: $($pipeline.duration) seconds"
```

### Получить jobs из pipeline

```powershell
$pipelineId = 12345
$jobs = Invoke-RestMethod -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines/$pipelineId/jobs" -Headers $headers

$jobs | ForEach-Object {
    Write-Host "Job: $($_.name) | Stage: $($_.stage) | Status: $($_.status)"
}
```

### Скачать артефакты из job

```powershell
$jobId = 67890
$outputFile = "artifacts.zip"

Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/jobs/$jobId/artifacts" `
    -Headers $headers `
    -OutFile $outputFile

Write-Host "Artifacts downloaded to $outputFile"
```

### Запустить новый pipeline

```powershell
$body = @{
    ref = "master"
    variables = @(
        @{
            key = "CUSTOM_VAR"
            value = "custom_value"
        }
    )
} | ConvertTo-Json -Depth 3

$pipeline = Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipeline" `
    -Method Post `
    -Headers $headers `
    -ContentType "application/json" `
    -Body $body

Write-Host "Pipeline started: $($pipeline.id)"
```

### Отменить pipeline

```powershell
$pipelineId = 12345

Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines/$pipelineId/cancel" `
    -Method Post `
    -Headers $headers

Write-Host "Pipeline $pipelineId cancelled"
```

### Повторить pipeline

```powershell
$pipelineId = 12345

$pipeline = Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines/$pipelineId/retry" `
    -Method Post `
    -Headers $headers

Write-Host "Pipeline retried: $($pipeline.id)"
```

## 🏷️ Работа с тегами

### Получить список тегов

```powershell
$tags = Invoke-RestMethod -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/repository/tags" -Headers $headers

$tags | Select-Object -First 10 | ForEach-Object {
    Write-Host "$($_.name) - $($_.commit.created_at)"
}
```

### Создать тег

```powershell
$body = @{
    tag_name = "v0.3.0"
    ref = "master"
    message = "Release version 0.3.0"
} | ConvertTo-Json

$tag = Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/repository/tags" `
    -Method Post `
    -Headers $headers `
    -ContentType "application/json" `
    -Body $body

Write-Host "Tag created: $($tag.name)"
```

### Удалить тег

```powershell
$tagName = "v0.2.0-beta"

Invoke-RestMethod `
    -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/repository/tags/$tagName" `
    -Method Delete `
    -Headers $headers

Write-Host "Tag $tagName deleted"
```

## 📊 Полезные скрипты

### Скрипт: Скачать все артефакты последнего релиза

```powershell
# Получить последний релиз
$releases = Invoke-RestMethod -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/releases" -Headers $headers
$latestRelease = $releases[0]

Write-Host "Downloading artifacts for $($latestRelease.tag_name)..."

# Создать директорию для артефактов
$outputDir = "releases\$($latestRelease.tag_name)"
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

# Скачать все asset links
$latestRelease.assets.links | ForEach-Object {
    $fileName = $_.name
    $url = $_.url
    
    Write-Host "Downloading $fileName..."
    
    Invoke-WebRequest -Uri $url -OutFile "$outputDir\$fileName" -Headers $headers
}

Write-Host "`nAll artifacts downloaded to $outputDir"
```

### Скрипт: Проверить статус последнего pipeline для тега

```powershell
function Get-TagPipelineStatus {
    param([string]$TagName)
    
    # Получить pipelines для тега
    $pipelines = Invoke-RestMethod `
        -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines?ref=$TagName" `
        -Headers $headers
    
    if ($pipelines.Count -eq 0) {
        Write-Host "No pipelines found for tag $TagName" -ForegroundColor Yellow
        return
    }
    
    $latestPipeline = $pipelines[0]
    
    # Получить детали
    $pipeline = Invoke-RestMethod `
        -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines/$($latestPipeline.id)" `
        -Headers $headers
    
    # Вывести статус
    $statusColor = switch ($pipeline.status) {
        "success" { "Green" }
        "failed" { "Red" }
        "running" { "Yellow" }
        default { "White" }
    }
    
    Write-Host "`nPipeline for $TagName" -ForegroundColor Cyan
    Write-Host "Status: " -NoNewline
    Write-Host $pipeline.status -ForegroundColor $statusColor
    Write-Host "Duration: $($pipeline.duration) seconds"
    Write-Host "URL: $($pipeline.web_url)"
    
    # Получить jobs
    $jobs = Invoke-RestMethod `
        -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/pipelines/$($pipeline.id)/jobs" `
        -Headers $headers
    
    Write-Host "`nJobs:"
    $jobs | ForEach-Object {
        $jobStatusColor = switch ($_.status) {
            "success" { "Green" }
            "failed" { "Red" }
            "running" { "Yellow" }
            default { "White" }
        }
        Write-Host "  $($_.name) [$($_.stage)]: " -NoNewline
        Write-Host $_.status -ForegroundColor $jobStatusColor
    }
}

# Использование
Get-TagPipelineStatus -TagName "v0.1.0"
```

### Скрипт: Автоматическое обновление описаний релизов из CHANGELOG

```powershell
function Update-ReleaseFromChangelog {
    param([string]$TagName)
    
    # Читаем CHANGELOG.md
    $changelog = Get-Content "CHANGELOG.md" -Raw
    
    # Извлекаем секцию для версии (без 'v' prefix)
    $version = $TagName -replace '^v', ''
    $pattern = "## \[$version\].*?(?=## \[|$)"
    
    if ($changelog -match $pattern) {
        $releaseNotes = $Matches[0]
        
        # Обновляем релиз
        $body = @{
            description = $releaseNotes
        } | ConvertTo-Json
        
        Invoke-RestMethod `
            -Uri "https://gitlab.com/api/v4/projects/$env:GITLAB_PROJECT_ID/releases/$TagName" `
            -Method Put `
            -Headers $headers `
            -ContentType "application/json" `
            -Body $body
        
        Write-Host "Release $TagName updated from CHANGELOG.md" -ForegroundColor Green
    } else {
        Write-Host "Version $version not found in CHANGELOG.md" -ForegroundColor Yellow
    }
}

# Использование
Update-ReleaseFromChangelog -TagName "v0.1.0"
```

## 🔗 Дополнительная информация

- [GitLab API Documentation](https://docs.gitlab.com/ee/api/)
- [Releases API](https://docs.gitlab.com/ee/api/releases/)
- [Pipelines API](https://docs.gitlab.com/ee/api/pipelines.html)
- [Jobs API](https://docs.gitlab.com/ee/api/jobs.html)
- [Tags API](https://docs.gitlab.com/ee/api/tags.html)

## 💡 Совет

Для более сложной автоматизации рассмотрите использование:
- [GitLab CLI (`glab`)](https://gitlab.com/gitlab-org/cli)
- [PowerShell GitLab Module](https://www.powershellgallery.com/packages/PSGitLab)


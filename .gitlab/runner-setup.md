# Настройка GitLab Runner для Windows

Это руководство поможет настроить GitLab Runner на Windows для автоматической сборки Cedar Folder Size Analyzer.

## 📋 Требования

- Windows 10/11 или Windows Server 2016+
- PowerShell 5.1 или выше
- Права администратора
- Интернет доступ

## 🚀 Установка GitLab Runner

### Шаг 1: Скачивание GitLab Runner

Откройте PowerShell от имени администратора и выполните:

```powershell
# Создайте директорию для GitLab Runner
New-Item -ItemType Directory -Force -Path "C:\GitLab-Runner"
Set-Location "C:\GitLab-Runner"

# Скачайте последнюю версию GitLab Runner
Invoke-WebRequest -Uri "https://gitlab-runner-downloads.s3.amazonaws.com/latest/binaries/gitlab-runner-windows-amd64.exe" -OutFile "gitlab-runner.exe"
```

### Шаг 2: Регистрация Runner

```powershell
# Зарегистрируйте runner
.\gitlab-runner.exe register
```

Вам будут заданы следующие вопросы:

1. **GitLab instance URL:**
   ```
   https://gitlab.com/
   ```
   (или URL вашей GitLab инсталляции)

2. **Registration token:**
   - Получите токен в GitLab: Settings → CI/CD → Runners → Expand
   - Скопируйте Registration token

3. **Description:**
   ```
   Windows Runner for Cedar Folder Size Analyzer
   ```

4. **Tags:**
   ```
   windows
   ```
   ⚠️ **Важно:** Тег `windows` используется в `.gitlab-ci.yml`

5. **Executor:**
   ```
   shell
   ```

6. **Shell:**
   ```
   powershell
   ```

### Шаг 3: Установка как Windows Service

```powershell
# Установите GitLab Runner как службу
.\gitlab-runner.exe install

# Запустите службу
.\gitlab-runner.exe start

# Проверьте статус
.\gitlab-runner.exe status
```

### Шаг 4: Проверка установки

```powershell
# Проверьте, что runner зарегистрирован
.\gitlab-runner.exe list
```

Вы также можете проверить в GitLab:
- Settings → CI/CD → Runners
- Ваш runner должен появиться в списке с зелёной точкой

## ⚙️ Настройка Runner

### Редактирование конфигурации

Откройте файл `C:\GitLab-Runner\config.toml` в текстовом редакторе:

```toml
concurrent = 1
check_interval = 0

[session_server]
  session_timeout = 1800

[[runners]]
  name = "Windows Runner for Cedar Folder Size Analyzer"
  url = "https://gitlab.com/"
  token = "YOUR_TOKEN"
  executor = "shell"
  shell = "powershell"
  
  [runners.custom_build_dir]
  
  [runners.cache]
    [runners.cache.s3]
    [runners.cache.gcs]
    [runners.cache.azure]
```

### Рекомендуемые настройки

Для лучшей производительности добавьте:

```toml
[[runners]]
  # ... существующие настройки ...
  
  # Увеличьте таймаут для долгих сборок
  builds_dir = "C:/GitLab-Runner/builds"
  cache_dir = "C:/GitLab-Runner/cache"
  
  # Лимит одновременных задач
  limit = 1
  
  [runners.cache]
    Type = "local"
    Path = "C:/GitLab-Runner/cache"
    Shared = false
```

После изменений перезапустите службу:

```powershell
.\gitlab-runner.exe restart
```

## 🔧 Установка зависимостей (опционально)

Для ускорения сборки можно предустановить Rust:

```powershell
# Скачайте и установите Rust
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y --default-toolchain stable --profile minimal
Remove-Item rustup-init.exe

# Перезапустите PowerShell или обновите PATH
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# Проверьте установку
rustc --version
cargo --version
```

**Примечание:** GitLab Runner обычно запускается от имени SYSTEM, поэтому Rust нужно установить для этого пользователя или настроить в CI.

## 🐛 Отладка

### Проверка логов

```powershell
# Просмотр логов службы
Get-EventLog -LogName Application -Source gitlab-runner -Newest 50

# Или в реальном времени
.\gitlab-runner.exe run
```

### Распространённые проблемы

#### 1. Runner не запускается

```powershell
# Проверьте статус службы
Get-Service gitlab-runner

# Перезапустите службу
Restart-Service gitlab-runner
```

#### 2. Ошибки прав доступа

Убедитесь, что служба GitLab Runner запущена от имени пользователя с достаточными правами:

```powershell
# Остановите службу
.\gitlab-runner.exe stop

# Удалите службу
.\gitlab-runner.exe uninstall

# Установите заново с правильным пользователем
.\gitlab-runner.exe install --user ".\YourUsername" --password "YourPassword"

# Запустите службу
.\gitlab-runner.exe start
```

#### 3. Rust не найден

Если CI не может найти Rust, убедитесь что:
- Rust установлен для пользователя, под которым запущен GitLab Runner
- PATH настроен корректно
- Или используйте автоматическую установку в `.gitlab-ci.yml` (уже настроено)

## 🔒 Безопасность

### Ограничение доступа к Runner

Рекомендуется использовать **specific runners** вместо **shared runners**:

1. В GitLab: Settings → CI/CD → Runners
2. Найдите ваш runner
3. Нажмите кнопку **Edit**
4. Снимите галочку **"Run untagged jobs"**
5. Добавьте только ваш проект в **"Restrict projects for this Runner"**

### Защищённые переменные

Для хранения секретов используйте защищённые переменные:

1. Settings → CI/CD → Variables
2. Добавьте переменные с флагом **Protected**
3. Они будут доступны только для защищённых веток и тегов

## 📊 Мониторинг

### Проверка активности Runner

```powershell
# Статус службы
.\gitlab-runner.exe status

# Список запущенных задач
.\gitlab-runner.exe list

# Детальная информация
.\gitlab-runner.exe verify
```

### Логи в GitLab

Проверяйте логи выполнения в:
- CI/CD → Pipelines → Job logs

## 🔄 Обновление Runner

```powershell
# Остановите службу
.\gitlab-runner.exe stop

# Скачайте новую версию
Invoke-WebRequest -Uri "https://gitlab-runner-downloads.s3.amazonaws.com/latest/binaries/gitlab-runner-windows-amd64.exe" -OutFile "gitlab-runner.exe"

# Запустите службу
.\gitlab-runner.exe start

# Проверьте версию
.\gitlab-runner.exe --version
```

## 🆘 Дополнительная помощь

- [Официальная документация GitLab Runner](https://docs.gitlab.com/runner/)
- [GitLab Runner для Windows](https://docs.gitlab.com/runner/install/windows.html)
- [Troubleshooting](https://docs.gitlab.com/runner/faq/)

## 📝 Checklist настройки

- [ ] GitLab Runner скачан
- [ ] Runner зарегистрирован с тегом `windows`
- [ ] Executor установлен как `shell`
- [ ] Shell установлен как `powershell`
- [ ] Служба GitLab Runner установлена
- [ ] Служба запущена и работает
- [ ] Runner виден в GitLab как активный
- [ ] Тестовый pipeline запущен и выполнен успешно
- [ ] (Опционально) Rust предустановлен
- [ ] (Опционально) Настроено кэширование

После завершения настройки ваш GitLab Runner готов для автоматической сборки релизов! 🎉


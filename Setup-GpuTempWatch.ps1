#Requires -RunAsAdministrator
# Setup-GpuTempWatch.ps1 — Установка и автозапуск мониторинга температуры GPU

param(
    [switch]$Uninstall,
    [switch]$Force,
    [switch]$Console,  # Использовать консольную версию вместо tray
    [switch]$Tray      # Явно использовать tray версию (по умолчанию)
)

$TaskName = "GpuTempWatch"
$VBSWrapperPath = "$PSScriptRoot\StartGpuTempWatch.vbs"

# Выбор версии скрипта
if ($Console) {
    $ScriptPath = "$PSScriptRoot\GpuTempWatch.ps1"
    $Version = "Console"
    $VBSArgument = "console"
} else {
    $ScriptPath = "$PSScriptRoot\GpuTempWatch-Tray.ps1"
    $Version = "System Tray"
    $VBSArgument = ""
}

function Write-Status([string]$Message, [string]$Type = "Info") {
    $timestamp = Get-Date -Format "HH:mm:ss"
    $color = switch ($Type) {
        "Success" { "Green" }
        "Warning" { "Yellow" }
        "Error" { "Red" }
        default { "White" }
    }
    Write-Host "[$timestamp] " -NoNewline -ForegroundColor Gray
    Write-Host $Message -ForegroundColor $color
}

function Uninstall-GpuTempWatch {
    Write-Status "Удаление автозапуска GpuTempWatch..." "Warning"

    try {
        $task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
        if ($task) {
            Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
            Write-Status "✓ Scheduled Task '$TaskName' удален" "Success"
        } else {
            Write-Status "⚠ Scheduled Task '$TaskName' не найден" "Warning"
        }
    } catch {
        Write-Status "✗ Ошибка удаления: $($_.Exception.Message)" "Error"
        return $false
    }

    Write-Status "Удаление завершено" "Success"
    return $true
}

# Обработка параметра Uninstall
if ($Uninstall) {
    Uninstall-GpuTempWatch
    Read-Host "Нажмите Enter для выхода"
    exit
}

Write-Status "=== Установка GpuTempWatch ($Version) ===" "Success"

# Проверяем существование основных файлов
if (-not (Test-Path $ScriptPath)) {
    Write-Status "✗ Не найден скрипт: $ScriptPath" "Error"
    Write-Status "Убедитесь что скрипт находится в той же папке" "Error"
    Read-Host "Нажмите Enter для выхода"
    exit 1
}

if (-not (Test-Path $VBSWrapperPath)) {
    Write-Status "✗ Не найден VBS wrapper: $VBSWrapperPath" "Error"
    Write-Status "Убедитесь что VBScript файл находится в той же папке" "Error"
    Read-Host "Нажмите Enter для выхода"
    exit 1
}

Write-Status "✓ Найден скрипт: $ScriptPath" "Success"
Write-Status "✓ Найден VBS wrapper: $VBSWrapperPath" "Success"

# Настройка политики выполнения
Write-Status "Настройка ExecutionPolicy..." "Info"
try {
    Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force
    Write-Status "✓ ExecutionPolicy установлен: RemoteSigned" "Success"
} catch {
    Write-Status "✗ Ошибка настройки ExecutionPolicy: $($_.Exception.Message)" "Error"
}

# Установка BurntToast модуля
Write-Status "Проверка модуля BurntToast..." "Info"
$burntToast = Get-Module -ListAvailable -Name BurntToast
if ($burntToast) {
    Write-Status "✓ BurntToast уже установлен (версия $($burntToast.Version))" "Success"
} else {
    Write-Status "Установка BurntToast модуля..." "Info"
    try {
        Install-Module BurntToast -Force -Scope CurrentUser -Repository PSGallery
        Write-Status "✓ BurntToast модуль установлен" "Success"
    } catch {
        Write-Status "✗ Ошибка установки BurntToast: $($_.Exception.Message)" "Error"
        Write-Status "⚠ Будет использован fallback MessageBox" "Warning"
    }
}

# Проверяем существующую задачу
$existingTask = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
if ($existingTask) {
    if ($Force) {
        Write-Status "⚠ Scheduled Task '$TaskName' уже существует, принудительное пересоздание..." "Warning"
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        Write-Status "✓ Существующая задача удалена" "Success"
    } else {
        Write-Status "⚠ Scheduled Task '$TaskName' уже существует" "Warning"
        $choice = Read-Host "Пересоздать? (y/N)"
        if ($choice -ne 'y' -and $choice -ne 'Y') {
            Write-Status "Установка отменена" "Warning"
            Read-Host "Нажмите Enter для выхода"
            exit
        }
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        Write-Status "✓ Существующая задача удалена" "Success"
    }
}

# Создание Scheduled Task с VBScript wrapper
Write-Status "Создание Scheduled Task с VBScript wrapper..." "Info"
try {
    # Используем VBScript wrapper для полностью скрытого запуска
    if ($Console) {
        $action = New-ScheduledTaskAction -Execute "wscript.exe" -Argument "`"$VBSWrapperPath`" console"
    } else {
        $action = New-ScheduledTaskAction -Execute "wscript.exe" -Argument "`"$VBSWrapperPath`""
    }

    $trigger = New-ScheduledTaskTrigger -AtLogOn

    $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew -StartWhenAvailable

    $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited

    Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Description "Мониторинг температуры GPU с уведомлениями ($Version) - VBScript wrapper"

    Write-Status "✓ Scheduled Task '$TaskName' создан" "Success"
    Write-Status "  - Версия: $Version" "Info"
    Write-Status "  - Запуск: При входе в систему через VBScript" "Info"
    Write-Status "  - Пользователь: $env:USERNAME" "Info"
    Write-Status "  - Режим: Полностью скрытое выполнение" "Info"

} catch {
    Write-Status "✗ Ошибка создания Scheduled Task: $($_.Exception.Message)" "Error"
    Read-Host "Нажмите Enter для выхода"
    exit 1
}

# Проверка корректности задачи
Write-Status "Проверка созданной задачи..." "Info"
$task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
if ($task) {
    Write-Status "✓ Задача успешно зарегистрирована" "Success"
    Write-Status "  Состояние: $($task.State)" "Info"
} else {
    Write-Status "✗ Ошибка: задача не найдена после создания" "Error"
}

Write-Status "" "Info"
Write-Status "=== Установка завершена ===" "Success"
Write-Status "" "Info"
Write-Status "Что дальше:" "Info"
Write-Status "• Скрипт будет автоматически запускаться при входе в Windows" "Info"
Write-Status "• Запуск происходит полностью скрыто через VBScript wrapper" "Info"
if ($Version -eq "System Tray") {
    Write-Status "• Иконка появится в системном трее после запуска" "Info"
    Write-Status "• ПКМ на иконке для доступа к настройкам и управлению" "Info"
    Write-Status "• Консольное окно автоматически скрывается" "Info"
}
Write-Status "• Логи сохраняются в: .\Logs\GpuTempWatch.log" "Info"
Write-Status "• Для удаления: .\Setup-GpuTempWatch.ps1 -Uninstall" "Info"
Write-Status "• Для консольной версии: .\Setup-GpuTempWatch.ps1 -Console" "Info"
Write-Status "• Для ручного запуска: StartGpuTempWatch.vbs [console]" "Info"
Write-Status "" "Info"

$startNow = Read-Host "Запустить мониторинг сейчас? (Y/n)"
if ($startNow -ne 'n' -and $startNow -ne 'N') {
    Write-Status "Запуск мониторинга..." "Info"
    Start-ScheduledTask -TaskName $TaskName
    Write-Status "✓ Мониторинг запущен в фоне" "Success"
    Start-Sleep 2

    # Показываем последние логи
    $logPath = "$PSScriptRoot\Logs\GpuTempWatch.log"
    if (Test-Path $logPath) {
        Write-Status "Последние записи в логе:" "Info"
        Get-Content $logPath -Tail 3 | ForEach-Object { Write-Status "  $_" "Info" }
    }
}

Read-Host "Нажмите Enter для выхода"
#Requires -Version 5.1
# GpuTempWatch-Tray.ps1 — Системный трей для мониторинга температуры GPU

# Установка STA режима для Windows Forms
[System.Threading.Thread]::CurrentThread.SetApartmentState('STA')

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# Включаем визуальные стили
[System.Windows.Forms.Application]::EnableVisualStyles()
[System.Windows.Forms.Application]::SetCompatibleTextRenderingDefault($false)

# Конфигурация
$ThresholdC       = 60
$PollSeconds      = 20
$BaseCooldownSec  = 20
$LhmUrl           = "http://127.0.0.1:8085/data.json"
$GpuTempNamePatterns = @("*GPU*Core*", "*GPU*Hot*")

# Пути (абсолютные для работы из Scheduled Task)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$LogDir  = "$ScriptDir\Logs"
$LogFile = "$LogDir\GpuTempWatch.log"
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir | Out-Null }

# Глобальные переменные
$script:Running = $true
$script:Paused = $false
$script:CurrentTemp = 0
$script:LastStatus = "Инициализация..."
$script:StartTime = Get-Date
$script:lastNotified = (Get-Date).AddYears(-1)
$script:currentCooldown = $BaseCooldownSec
$script:wasOverheating = $false

# Низкий приоритет
try { (Get-Process -Id $PID).PriorityClass = 'Idle' } catch {}

function Write-Log([string]$msg) {
    $ts = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    "$ts $msg" | Out-File -FilePath $LogFile -Append -Encoding utf8
}

# Логирование запуска
Write-Log "INFO: =========================================="
Write-Log "INFO: GpuTempWatch-Tray.ps1 запущен"
Write-Log "INFO: PowerShell версия: $($PSVersionTable.PSVersion)"
Write-Log "INFO: Аргументы командной строки: $($MyInvocation.Line)"
Write-Log "INFO: Родительский процесс: $(try { (Get-WmiObject Win32_Process -Filter "ProcessId=$PID").ParentProcessId } catch { 'Unknown' })"
Write-Log "INFO: Пользователь: $env:USERNAME"
Write-Log "INFO: Рабочая директория: $(Get-Location)"

# Завершаем старые экземпляры
$currentPID = $PID
$scriptName = [System.IO.Path]::GetFileNameWithoutExtension($MyInvocation.MyCommand.Name)
$oldProcesses = Get-WmiObject Win32_Process | Where-Object {
    $_.CommandLine -like "*$scriptName*" -and $_.ProcessId -ne $currentPID
}
if ($oldProcesses.Count -gt 0) {
    $result = [System.Windows.Forms.MessageBox]::Show(
        "Обнаружены запущенные экземпляры GPU Temp Watch ($($oldProcesses.Count) шт.).`nЗавершить их?",
        "Дублирующие процессы",
        [System.Windows.Forms.MessageBoxButtons]::YesNo,
        [System.Windows.Forms.MessageBoxIcon]::Question
    )
    if ($result -eq [System.Windows.Forms.DialogResult]::Yes) {
        foreach ($proc in $oldProcesses) {
            try {
                Write-Log "INFO: Завершение старого процесса PID $($proc.ProcessId)"
                Stop-Process -Id $proc.ProcessId -Force
            } catch {
                Write-Log "WARN: Не удалось завершить процесс PID $($proc.ProcessId)"
            }
        }
    }
}

# Проверка и загрузка BurntToast - улучшенная инициализация
$BurntToastAvailable = $false
try {
    # Пытаемся принудительно импортировать модуль с разных путей
    $burntToastModule = Get-Module -ListAvailable -Name BurntToast | Select-Object -First 1
    if ($burntToastModule) {
        Import-Module BurntToast -ErrorAction Stop -Force -Global
        $BurntToastAvailable = $true
        Write-Log "INFO: BurntToast модуль v$($burntToastModule.Version) загружен из $($burntToastModule.ModuleBase)"
    } else {
        # Попытка установить модуль если его нет
        Write-Log "WARN: BurntToast модуль не найден, попытка установки..."
        try {
            Install-Module BurntToast -Force -Scope CurrentUser -Repository PSGallery -ErrorAction Stop
            Import-Module BurntToast -ErrorAction Stop -Force -Global
            $BurntToastAvailable = $true
            Write-Log "INFO: BurntToast модуль успешно установлен и загружен"
        } catch {
            Write-Log "ERROR: Не удалось установить BurntToast — $($_.Exception.Message)"
        }
    }
} catch {
    Write-Log "ERROR: Ошибка загрузки BurntToast — $($_.Exception.Message)"
    # Дополнительная попытка с прямым путем к модулю
    try {
        $userModulesPath = "$env:USERPROFILE\Documents\WindowsPowerShell\Modules\BurntToast"
        if (Test-Path $userModulesPath) {
            Import-Module $userModulesPath -Force -Global
            $BurntToastAvailable = $true
            Write-Log "INFO: BurntToast загружен по прямому пути"
        }
    } catch {
        Write-Log "ERROR: Последняя попытка загрузки BurntToast неудачна — $($_.Exception.Message)"
    }
}

function Parse-Temp([object]$valueStr) {
    if ($null -eq $valueStr) { return $null }
    $m = [regex]::Match([string]$valueStr, '[-+]?\d+(?:[.,]\d+)?')
    if (-not $m.Success) { return $null }
    $s = $m.Value.Replace(',', '.')
    try { return [double]$s } catch { return $null }
}

function Collect-GpuTemps([object]$node) {
    $temps = @()
    if ($null -eq $node) { return $temps }

    if ($node.PSObject.Properties.Name -contains 'Type' -and
        $node.Type -eq 'Temperature' -and
        $node.PSObject.Properties.Name -contains 'Text') {

        foreach ($pat in $GpuTempNamePatterns) {
            if ($node.Text -like $pat) {
                $val = Parse-Temp $node.Value
                if ($null -ne $val) { $temps += $val }
                break
            }
        }
    }

    if ($node.PSObject.Properties.Name -contains 'Children' -and $node.Children) {
        foreach ($ch in $node.Children) {
            $childTemps = Collect-GpuTemps $ch
            if ($null -ne $childTemps -and $childTemps.Length -gt 0) {
                $temps += $childTemps
            }
        }
    }
    return $temps
}

function Notify([double]$tc, [int]$threshold) {
    $message = "⚠ ПЕРЕГРЕВ ВИДЕОКАРТЫ! GPU: $tc °C (порог: $threshold °C)"
    Write-Log "ALERT: GPU $tc °C >= $threshold °C"

    # BurntToast уведомления
    if ($BurntToastAvailable) {
        try {
            New-BurntToastNotification -Text "⚠ Перегрев видеокарты", "GPU: $tc °C (порог: $threshold °C)" -Sound 'Alarm2'
            Write-Log "INFO: BurntToast уведомление отправлено"
            return
        } catch {
            Write-Log "ERROR: BurntToast ошибка — $($_.Exception.Message), переход на fallback"
        }
    }

    # MessageBox fallback
    try {
        [System.Windows.Forms.MessageBox]::Show(
            $message,
            "🔥 GPU Temperature Alert",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Warning,
            [System.Windows.Forms.MessageBoxDefaultButton]::Button1,
            [System.Windows.Forms.MessageBoxOptions]::DefaultDesktopOnly
        ) | Out-Null
        Write-Log "INFO: MessageBox уведомление отправлено"
    } catch {
        Write-Log "ERROR: MessageBox ошибка — $($_.Exception.Message)"
    }
}

function Get-TempIcon([double]$temp) {
    if ($temp -eq 0) { return "❓" }  # Нет данных
    if ($temp -ge $ThresholdC) { return "🔴" }  # Перегрев
    if ($temp -ge ($ThresholdC * 0.8)) { return "🟡" }  # Предупреждение
    return "🟢"  # Норма
}

function Update-TrayIcon($notifyIcon, [double]$temp) {
    $icon = Get-TempIcon $temp
    $tempText = if ($temp -gt 0) { "$temp°C" } else { "N/A" }

    $notifyIcon.Text = "GPU Temp: $tempText"

    # Создаем простую иконку из текста (эмуляция)
    $bitmap = New-Object System.Drawing.Bitmap(16, 16)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.Clear([System.Drawing.Color]::Transparent)

    $color = switch ($icon) {
        "🔴" { [System.Drawing.Color]::Red }
        "🟡" { [System.Drawing.Color]::Orange }
        "🟢" { [System.Drawing.Color]::LimeGreen }
        default { [System.Drawing.Color]::Gray }
    }

    $graphics.FillEllipse([System.Drawing.SolidBrush]::new($color), 2, 2, 12, 12)
    $graphics.Dispose()

    $notifyIcon.Icon = [System.Drawing.Icon]::FromHandle($bitmap.GetHicon())
}

function Show-SettingsDialog {
    $form = New-Object System.Windows.Forms.Form
    $form.Text = "Настройки GPU Temp Watch"
    $form.Size = New-Object System.Drawing.Size(300, 200)
    $form.StartPosition = "CenterScreen"
    $form.FormBorderStyle = "FixedDialog"
    $form.MaximizeBox = $false

    $label1 = New-Object System.Windows.Forms.Label
    $label1.Text = "Порог температуры (°C):"
    $label1.Location = New-Object System.Drawing.Point(10, 20)
    $label1.Size = New-Object System.Drawing.Size(150, 20)
    $form.Controls.Add($label1)

    $textBox1 = New-Object System.Windows.Forms.TextBox
    $textBox1.Text = $ThresholdC
    $textBox1.Location = New-Object System.Drawing.Point(170, 18)
    $textBox1.Size = New-Object System.Drawing.Size(60, 20)
    $form.Controls.Add($textBox1)

    $label2 = New-Object System.Windows.Forms.Label
    $label2.Text = "Интервал опроса (сек):"
    $label2.Location = New-Object System.Drawing.Point(10, 50)
    $label2.Size = New-Object System.Drawing.Size(150, 20)
    $form.Controls.Add($label2)

    $textBox2 = New-Object System.Windows.Forms.TextBox
    $textBox2.Text = $PollSeconds
    $textBox2.Location = New-Object System.Drawing.Point(170, 48)
    $textBox2.Size = New-Object System.Drawing.Size(60, 20)
    $form.Controls.Add($textBox2)

    $okButton = New-Object System.Windows.Forms.Button
    $okButton.Text = "OK"
    $okButton.Location = New-Object System.Drawing.Point(120, 120)
    $okButton.Size = New-Object System.Drawing.Size(75, 23)
    $okButton.DialogResult = [System.Windows.Forms.DialogResult]::OK
    $form.Controls.Add($okButton)

    $cancelButton = New-Object System.Windows.Forms.Button
    $cancelButton.Text = "Отмена"
    $cancelButton.Location = New-Object System.Drawing.Point(200, 120)
    $cancelButton.Size = New-Object System.Drawing.Size(75, 23)
    $cancelButton.DialogResult = [System.Windows.Forms.DialogResult]::Cancel
    $form.Controls.Add($cancelButton)

    if ($form.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
        try {
            $script:ThresholdC = [int]$textBox1.Text
            $script:PollSeconds = [int]$textBox2.Text
            Write-Log "INFO: Настройки изменены - порог: $ThresholdC°C, интервал: $PollSeconds сек"
        } catch {
            [System.Windows.Forms.MessageBox]::Show("Ошибка в настройках. Используются старые значения.", "Ошибка")
        }
    }
}

# Создание элементов трея и контекста приложения
Write-Log "INFO: Создание элементов системного трея..."

try {
    $applicationContext = New-Object System.Windows.Forms.ApplicationContext
    Write-Log "INFO: ✓ ApplicationContext создан"

    $notifyIcon = New-Object System.Windows.Forms.NotifyIcon
    Write-Log "INFO: ✓ NotifyIcon создан"

    $notifyIcon.Visible = $true
    Write-Log "INFO: ✓ NotifyIcon установлен как видимый"
} catch {
    Write-Log "ERROR: Ошибка при создании элементов трея: $($_.Exception.Message)"
    Write-Log "ERROR: StackTrace: $($_.Exception.StackTrace)"
    throw
}

# Контекстное меню
$contextMenu = New-Object System.Windows.Forms.ContextMenuStrip
$notifyIcon.ContextMenuStrip = $contextMenu

# Пункты меню
$statusItem = New-Object System.Windows.Forms.ToolStripMenuItem
$statusItem.Text = "Температура: Загрузка..."
$statusItem.Enabled = $false
$contextMenu.Items.Add($statusItem)

$contextMenu.Items.Add("-")  # Разделитель

$pauseItem = New-Object System.Windows.Forms.ToolStripMenuItem
$pauseItem.Text = "Приостановить"
$pauseItem.Add_Click({
    $script:Paused = -not $script:Paused
    $pauseItem.Text = if ($script:Paused) { "Возобновить" } else { "Приостановить" }
    $status = if ($script:Paused) { "ПРИОСТАНОВЛЕН" } else { "Мониторинг возобновлен" }
    $script:LastStatus = $status
    Write-Log "INFO: $status"
})
$contextMenu.Items.Add($pauseItem)

$settingsItem = New-Object System.Windows.Forms.ToolStripMenuItem
$settingsItem.Text = "Настройки..."
$settingsItem.Add_Click({ Show-SettingsDialog })
$contextMenu.Items.Add($settingsItem)

$logsItem = New-Object System.Windows.Forms.ToolStripMenuItem
$logsItem.Text = "Показать логи"
$logsItem.Add_Click({
    if (Test-Path $LogFile) {
        Start-Process notepad.exe -ArgumentList $LogFile
    } else {
        [System.Windows.Forms.MessageBox]::Show("Файл логов не найден:`n$LogFile", "Ошибка", [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Warning)
    }
})
$contextMenu.Items.Add($logsItem)

$contextMenu.Items.Add("-")  # Разделитель

$exitItem = New-Object System.Windows.Forms.ToolStripMenuItem
$exitItem.Text = "Выход"
$exitItem.Add_Click({
    $script:Running = $false
    $timer.Stop()
    $notifyIcon.Visible = $false
    Write-Log "INFO: Завершение работы по команде пользователя"
    $applicationContext.ExitThread()
})
$contextMenu.Items.Add($exitItem)

# Двойной клик для показа статуса
$notifyIcon.Add_DoubleClick({
    $uptime = (New-TimeSpan $script:StartTime (Get-Date)).ToString("hh\:mm\:ss")
    $status = "GPU Temp Watch`n`nСостояние: $script:LastStatus`nВремя работы: $uptime`nТекущая температура: $script:CurrentTemp°C"
    [System.Windows.Forms.MessageBox]::Show($status, "Статус мониторинга", [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Information)
})

# Инициализация иконки
Update-TrayIcon $notifyIcon 0

Write-Log "=== Запуск GPU Temp Watch Tray (порог $ThresholdC °C, интервал $PollSeconds c) ==="

# Таймер для мониторинга
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = $PollSeconds * 1000
$timer.Add_Tick({
    if ($script:Paused) { return }

    try {
        $json = Invoke-RestMethod -Uri $LhmUrl -TimeoutSec 2
        $temps = Collect-GpuTemps $json

        if ($temps.Count -gt 0) {
            $maxTemp = [math]::Round( ($temps | Measure-Object -Maximum).Maximum, 1)
            $script:CurrentTemp = $maxTemp

            Write-Log "INFO: GPU температура(ы) = $($temps -join ', ') °C; max=$maxTemp °C"

            $script:LastStatus = "GPU: $maxTemp°C"
            $statusItem.Text = "Температура: $maxTemp°C $(Get-TempIcon $maxTemp)"
            Update-TrayIcon $notifyIcon $maxTemp

            if ($maxTemp -ge $ThresholdC) {
                $elapsed = (New-TimeSpan $script:lastNotified (Get-Date)).TotalSeconds
                if ($elapsed -ge $script:currentCooldown) {
                    Notify $maxTemp $ThresholdC
                    $script:lastNotified = Get-Date
                    $script:currentCooldown = [math]::Min($script:currentCooldown * 2, 320)
                    Write-Log "INFO: Следующее уведомление через $script:currentCooldown сек"
                }
                $script:wasOverheating = $true
            } else {
                if ($script:wasOverheating) {
                    $script:currentCooldown = $BaseCooldownSec
                    $script:wasOverheating = $false
                    Write-Log "INFO: Температура нормализовалась, cooldown сброшен до $BaseCooldownSec сек"
                }
            }
        } else {
            $script:LastStatus = "Сенсоры не найдены"
            $statusItem.Text = "Температура: N/A ❓"
            Update-TrayIcon $notifyIcon 0
            Write-Log "WARN: подходящие сенсоры GPU не найдены (patterns: $($GpuTempNamePatterns -join '; '))"
        }
    } catch {
        $script:LastStatus = "Ошибка подключения"
        $statusItem.Text = "Ошибка: LHM недоступен ❓"
        Update-TrayIcon $notifyIcon 0
        Write-Log "ERROR: запрос/парсинг JSON — $($_.Exception.Message)"
    }
})

$timer.Start()

# Скрываем консольное окно - улучшенная версия
Add-Type -Name Window -Namespace Console -MemberDefinition '
[DllImport("Kernel32.dll")]
public static extern IntPtr GetConsoleWindow();

[DllImport("user32.dll")]
public static extern bool ShowWindow(IntPtr hWnd, Int32 nCmdShow);

[DllImport("user32.dll")]
public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int x, int y, int cx, int cy, uint uFlags);

[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr hWnd);
'

# Функция для надежного скрытия консоли
function Hide-ConsoleWindow {
    try {
        $consolePtr = [Console.Window]::GetConsoleWindow()
        if ($consolePtr -ne [System.IntPtr]::Zero) {
            # Проверяем, видимо ли окно
            $wasVisible = [Console.Window]::IsWindowVisible($consolePtr)
            Write-Log "INFO: Консольное окно найдено (видимо: $wasVisible)"

            # Пробуем скрыть окно несколькими способами
            $result1 = [Console.Window]::ShowWindow($consolePtr, 0) # SW_HIDE
            Start-Sleep -Milliseconds 100
            $result2 = [Console.Window]::ShowWindow($consolePtr, 6) # SW_MINIMIZE
            Start-Sleep -Milliseconds 100
            $result3 = [Console.Window]::ShowWindow($consolePtr, 0) # SW_HIDE снова

            # Проверяем результат
            $isStillVisible = [Console.Window]::IsWindowVisible($consolePtr)
            Write-Log "INFO: Скрытие консоли: результаты ($result1, $result2, $result3), видимо: $isStillVisible"

            if (-not $isStillVisible) {
                Write-Log "INFO: ✓ Консольное окно успешно скрыто"
            } else {
                Write-Log "WARN: ⚠ Консольное окно все еще видимо после попыток скрытия"
            }
        } else {
            Write-Log "INFO: Консольное окно не найдено (уже скрыто или запущено без консоли)"
        }
    } catch {
        Write-Log "ERROR: Ошибка при скрытии консоли: $($_.Exception.Message)"
    }
}

# Скрываем консоль
Hide-ConsoleWindow

Write-Log "INFO: Системный трей запущен"

# Запускаем основной цикл приложения с контекстом
try {
    [System.Windows.Forms.Application]::Run($applicationContext)
} catch {
    Write-Log "ERROR: Ошибка в главном цикле приложения: $($_.Exception.Message)"
} finally {
    Write-Log "INFO: Завершение главного цикла приложения"
}

# Очистка при выходе
$timer.Stop()
$notifyIcon.Dispose()
Write-Log "INFO: GPU Temp Watch Tray завершен"
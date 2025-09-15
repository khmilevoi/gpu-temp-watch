#Requires -Version 5.1
# GpuTempWatch.ps1 — LHM JSON (Type/Text), рекурсивный обход, лёгкие логи

$ThresholdC       = 60
$PollSeconds      = 20
$BaseCooldownSec  = 20
$LhmUrl           = "http://127.0.0.1:8085/data.json"

# Какие названия сенсоров считаем «температурой GPU»
$GpuTempNamePatterns = @("*GPU*Core*", "*GPU*Hot*")   # можно добавить "*GPU*Temperature*"

# Логи (абсолютные пути для работы из Scheduled Task)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$LogDir  = "$ScriptDir\Logs"
$LogFile = "$LogDir\GpuTempWatch.log"
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir | Out-Null }

# Завершаем старые экземпляры
$currentPID = $PID
$scriptName = [System.IO.Path]::GetFileNameWithoutExtension($MyInvocation.MyCommand.Name)
$oldProcesses = Get-WmiObject Win32_Process | Where-Object {
    $_.CommandLine -like "*$scriptName*" -and $_.ProcessId -ne $currentPID
}
foreach ($proc in $oldProcesses) {
    try {
        Write-Log "INFO: Завершение старого процесса PID $($proc.ProcessId)"
        Stop-Process -Id $proc.ProcessId -Force
    } catch {
        Write-Log "WARN: Не удалось завершить процесс PID $($proc.ProcessId)"
    }
}

# Низкий приоритет
try { (Get-Process -Id $PID).PriorityClass = 'Idle' } catch {}

function Write-Log([string]$msg) {
    $ts = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    "$ts $msg" | Out-File -FilePath $LogFile -Append -Encoding utf8
}

# Уведомления - улучшенная инициализация BurntToast
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

# Переменные для умного cooldown
$lastNotified = (Get-Date).AddYears(-1)
$currentCooldown = $BaseCooldownSec
$wasOverheating = $false

function Parse-Temp([object]$valueStr) {
    if ($null -eq $valueStr) { return $null }
    # Вытаскиваем число с возможной запятой (61,0 °C -> 61.0)
    $m = [regex]::Match([string]$valueStr, '[-+]?\d+(?:[.,]\d+)?')
    if (-not $m.Success) { return $null }
    $s = $m.Value.Replace(',', '.')
    try { return [double]$s } catch { return $null }
}

function Collect-GpuTemps([object]$node) {
    $temps = @()
    if ($null -eq $node) { return $temps }

    # Если это «лист» с температурой
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

    # Рекурсивно обходим Children
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

    # Всегда логируем предупреждение
    Write-Log "ALERT: GPU $tc °C >= $threshold °C"

    # 1. Приоритет: BurntToast (красивые Windows 11 toast)
    if ($BurntToastAvailable) {
        try {
            New-BurntToastNotification -Text "⚠ Перегрев видеокарты", "GPU: $tc °C (порог: $threshold °C)" -Sound 'Alarm2'
            Write-Log "INFO: BurntToast уведомление отправлено"
            return
        } catch {
            Write-Log "ERROR: BurntToast ошибка — $($_.Exception.Message), переход на fallback"
        }
    }

    # 2. Fallback: MessageBox (навязчивые диалоги)
    try {
        Add-Type -AssemblyName System.Windows.Forms
        [System.Windows.Forms.MessageBox]::Show(
            $message,
            "🔥 GPU Temperature Alert",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Warning,
            [System.Windows.Forms.MessageBoxDefaultButton]::Button1,
            [System.Windows.Forms.MessageBoxOptions]::DefaultDesktopOnly
        ) | Out-Null
        Write-Log "INFO: MessageBox уведомление отправлено"
        return
    } catch {
        Write-Log "ERROR: MessageBox ошибка — $($_.Exception.Message)"
    }

    # 3. Последний fallback: консольное предупреждение
    Write-Host $message -ForegroundColor Red -BackgroundColor Yellow
    Write-Log "INFO: Консольное уведомление отправлено"
}

Write-Log "=== Запуск мониторинга (порог $ThresholdC °C, интервал $PollSeconds c) ==="

while ($true) {
    try {
        $json = Invoke-RestMethod -Uri $LhmUrl -TimeoutSec 2

        # собираем все подходящие температуры
        $temps = Collect-GpuTemps $json

        if ($temps.Count -gt 0) {
            $maxTemp = [math]::Round( ($temps | Measure-Object -Maximum).Maximum, 1)
            Write-Log "INFO: GPU температура(ы) = $($temps -join ', ') °C; max=$maxTemp °C"

            if ($maxTemp -ge $ThresholdC) {
                # Температура превышает порог
                $elapsed = (New-TimeSpan $lastNotified (Get-Date)).TotalSeconds

                if ($elapsed -ge $currentCooldown) {
                    Notify $maxTemp $ThresholdC
                    $lastNotified = Get-Date

                    # Увеличиваем cooldown экспоненциально (20, 40, 80, 160, 320 сек)
                    $currentCooldown = [math]::Min($currentCooldown * 2, 320)
                    Write-Log "INFO: Следующее уведомление через $currentCooldown сек"
                }
                $wasOverheating = $true
            } else {
                # Температура в норме - сбрасываем прогрессию
                if ($wasOverheating) {
                    $currentCooldown = $BaseCooldownSec
                    $wasOverheating = $false
                    Write-Log "INFO: Температура нормализовалась, cooldown сброшен до $BaseCooldownSec сек"
                }
            }
        } else {
            Write-Log "WARN: подходящие сенсоры GPU не найдены (patterns: $($GpuTempNamePatterns -join '; '))"
        }
    } catch {
        Write-Log "ERROR: запрос/парсинг JSON — $($_.Exception.Message)"
    }

    Start-Sleep -Seconds $PollSeconds
}

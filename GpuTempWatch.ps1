#Requires -Version 5.1
# GpuTempWatch.ps1 — LHM JSON (Type/Text), рекурсивный обход, лёгкие логи

$ThresholdC       = 60
$PollSeconds      = 20
$BaseCooldownSec  = 20
$LhmUrl           = "http://127.0.0.1:8085/data.json"

# Какие названия сенсоров считаем «температурой GPU»
$GpuTempNamePatterns = @("*GPU*Core*", "*GPU*Hot*")   # можно добавить "*GPU*Temperature*"

# Логи
$LogDir  = ".\Logs"
$LogFile = "$LogDir\GpuTempWatch.log"
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir | Out-Null }

# Низкий приоритет
try { (Get-Process -Id $PID).PriorityClass = 'Idle' } catch {}

function Write-Log([string]$msg) {
    $ts = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    "$ts $msg" | Out-File -FilePath $LogFile -Append -Encoding utf8
}

# Уведомления
$BurntToastAvailable = $false
try {
    Import-Module BurntToast -ErrorAction Stop
    $BurntToastAvailable = $true
    Write-Log "INFO: BurntToast модуль загружен"
} catch {
    Write-Log "WARN: BurntToast модуль недоступен, используется fallback уведомления"
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

    # Показываем уведомление только если BurntToast доступен (не навязчиво)
    if ($BurntToastAvailable) {
        try {
            New-BurntToastNotification -Text "⚠ Перегрев видеокарты", "GPU: $tc °C (порог: $threshold °C)" -Sound 'Alarm2'
            Write-Log "INFO: Toast уведомление отправлено"
            return
        } catch {
            Write-Log "ERROR: BurntToast ошибка — $($_.Exception.Message)"
        }
    }

    # Консольное предупреждение без MessageBox (не навязчиво)
    Write-Host $message -ForegroundColor Red -BackgroundColor Yellow
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

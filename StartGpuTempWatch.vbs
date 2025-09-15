' StartGpuTempWatch.vbs
' VBScript-оболочка для запуска PowerShell скриптов без видимых окон
' Используется для полностью скрытого выполнения GpuTempWatch

Option Explicit

Dim objShell, scriptPath, command, consoleMode, trayMode

' Получаем объект Shell
Set objShell = CreateObject("WScript.Shell")

' Определяем путь к скрипту (в той же папке что и VBScript)
scriptPath = Left(WScript.ScriptFullName, InStrRev(WScript.ScriptFullName, "\"))

' Функция для логирования (для отладки)
Sub WriteLog(message)
    Dim logFile, fso, logPath
    Set fso = CreateObject("Scripting.FileSystemObject")
    logPath = scriptPath & "Logs\StartGpuTempWatch.log"

    ' Создаем папку Logs если её нет
    If Not fso.FolderExists(scriptPath & "Logs") Then
        fso.CreateFolder(scriptPath & "Logs")
    End If

    Set logFile = fso.OpenTextFile(logPath, 8, True) ' 8 = ForAppending
    logFile.WriteLine Now() & " [VBS] " & message
    logFile.Close
    Set logFile = Nothing
    Set fso = Nothing
End Sub

' Проверяем аргументы командной строки для выбора режима
consoleMode = False
trayMode = True ' По умолчанию используем tray версию

WriteLog "VBScript запущен"

' Анализируем аргументы
If WScript.Arguments.Count > 0 Then
    If LCase(WScript.Arguments(0)) = "console" Or LCase(WScript.Arguments(0)) = "-console" Then
        consoleMode = True
        trayMode = False
        WriteLog "Режим: Console"
    Else
        WriteLog "Режим: Tray (неизвестный аргумент: " & WScript.Arguments(0) & ")"
    End If
Else
    WriteLog "Режим: Tray (по умолчанию)"
End If

' Выбираем соответствующий PowerShell скрипт
If consoleMode Then
    command = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File """ & scriptPath & "GpuTempWatch.ps1"""
Else
    ' Для tray версии используем STA режим (необходим для Windows Forms)
    ' Добавляем -WindowStyle Hidden для гарантированного скрытия окна
    command = "powershell.exe -STA -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File """ & scriptPath & "GpuTempWatch-Tray.ps1"""
End If

' Запускаем PowerShell скрипт полностью скрыто
' Параметры: команда, стиль окна (0 = скрыто), ожидание завершения (False)
WriteLog "Запуск команды: " & command

On Error Resume Next
objShell.Run command, 0, False
If Err.Number <> 0 Then
    WriteLog "ОШИБКА запуска: " & Err.Description & " (код: " & Err.Number & ")"
Else
    WriteLog "Команда запущена успешно"
End If
On Error Goto 0

' Завершаем VBScript
Set objShell = Nothing
WriteLog "VBScript завершен"
WScript.Quit
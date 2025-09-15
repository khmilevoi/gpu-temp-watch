' StartGpuTempWatch.vbs
' VBScript-оболочка для запуска PowerShell скриптов без видимых окон
' Используется для полностью скрытого выполнения GpuTempWatch

Option Explicit

Dim objShell, scriptPath, command, consoleMode, trayMode

' Получаем объект Shell
Set objShell = CreateObject("WScript.Shell")

' Определяем путь к скрипту (в той же папке что и VBScript)
scriptPath = Left(WScript.ScriptFullName, InStrRev(WScript.ScriptFullName, "\"))

' Проверяем аргументы командной строки для выбора режима
consoleMode = False
trayMode = True ' По умолчанию используем tray версию

' Анализируем аргументы
If WScript.Arguments.Count > 0 Then
    If LCase(WScript.Arguments(0)) = "console" Or LCase(WScript.Arguments(0)) = "-console" Then
        consoleMode = True
        trayMode = False
    End If
End If

' Выбираем соответствующий PowerShell скрипт
If consoleMode Then
    command = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File """ & scriptPath & "GpuTempWatch.ps1"""
Else
    ' Для tray версии используем STA режим (необходим для Windows Forms)
    command = "powershell.exe -STA -NoProfile -ExecutionPolicy Bypass -File """ & scriptPath & "GpuTempWatch-Tray.ps1"""
End If

' Запускаем PowerShell скрипт полностью скрыто
' Параметры: команда, стиль окна (0 = скрыто), ожидание завершения (False)
objShell.Run command, 0, False

' Завершаем VBScript
Set objShell = Nothing
WScript.Quit
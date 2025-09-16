# Remediation Plan (Windows 11 Target)

## 0. Минималистичная библиотечная стратегия
- **Reasoning:** Использование только `windows` crate от Microsoft обеспечивает прямой доступ ко всем Windows API без дополнительных обертек. Это упрощает зависимости, улучшает производительность и уменьшает attack surface.
- **Actions:**
  1. **Основа**: `windows` crate с необходимыми features для всех Windows API
  2. **Веб-сервер**: `axum` (features: `ws`, `macros`) - современная замена `warp` + `tokio-tungstenite`
  3. **Логирование**: `tracing` + `tracing-subscriber` для structured logging
  4. **Удалить избыточные зависимости**: `winapi`, `tray-icon`, `winrt-notification`, `warp`, `tokio-tungstenite`, `env_logger`, `log`
  5. **Оставить необходимые**: `tokio`, `serde`, `serde_json`, `nvml-wrapper`, `image`, `chrono`

## 1. Системный трей через Windows API
- **Reasoning:** Прямое использование Windows API через `windows` crate обеспечивает полный контроль над системным треем без промежуточных библиотек.
- **Actions:**
  1. Использовать `Shell_NotifyIconW` через `windows::Win32::UI::Shell` для создания иконки в трее
  2. Event loop через `GetMessage`/`DispatchMessage` для обработки сообщений Windows
  3. Контекстное меню через `CreatePopupMenu`/`TrackPopupMenu` - только правый клик
  4. `tokio::sync::mpsc` bridge для передачи событий из Windows thread в async main
  5. Меню: открыть веб-интерфейс, открыть логи, открыть конфиг, выход

## 2. Конфигурация и согласованность температур
- **Reasoning:** Монитор хранит устаревший `Config`, максимальная температура может сбрасываться в ноль между уведомлениями.
- **Actions:**
  1. Shared config через `Arc<RwLock<Config>>` - все компоненты читают актуальные значения
  2. Отслеживание максимальных температур с `max()` - всегда публикуем последние показания
  3. После обновлений `/api/config` немедленно обновляем shared config и оповещаем всех клиентов
  4. Добавить файловый watcher для автоматической перезагрузки конфигурации
  5. Валидация конфигурации с типобезопасными обертками

## 3. Real-Time веб-интерфейс через `axum`
- **Reasoning:** REST polling дает устаревшие данные; WebSocket клиенты получают нули из-за отсутствия broadcast'а.
- **Actions:**
  1. WebSocket через `axum::extract::ws` с современными паттернами 2025 года
  2. `tokio::sync::broadcast` для push-уведомлений всем активным клиентам
  3. Concurrent read/write через `socket.split()` для производительности
  4. Graceful shutdown с `Message::Close` для корректного закрытия соединений
  5. `State<AppState>` для shared state между HTTP и WebSocket handlers
  6. `/health` endpoint для мониторинга состояния сервиса

## 4. Windows 11 Toast уведомления через WinRT
- **Reasoning:** Toast'ы падают из-за отсутствия AppUserModelID и неправильной COM инициализации.
- **Actions:**
  1. Прямое использование `windows::ApplicationModel::Background` для регистрации приложения
  2. `windows::UI::Notifications::ToastNotificationManager` вместо внешних библиотек
  3. COM инициализация через `windows::Win32::System::Com::CoInitializeEx(COINIT_APARTMENTTHREADED)`
  4. `tokio::task::spawn_blocking` для выполнения WinRT вызовов в STA thread
  5. Fallback chain: WinRT toast → message box → console output
  6. Structured error logging для диагностики проблем с уведомлениями

## 5. Structured Logging с `tracing`
- **Reasoning:** Переход на современное structured logging для лучшей observability и cloud-native совместимости.
- **Actions:**
  1. Замена `log` + `env_logger` на `tracing` + `tracing-subscriber`
  2. `#[tracing::instrument]` макросы на ключевых функциях для автоматического захвата контекста
  3. JSON формат логов для структурированного анализа
  4. Правильные уровни логирования (trace/debug/info/warn/error)
  5. Span tracking для отслеживания выполнения через async boundaries
  6. Performance metrics и health check'и

## 6. Итоговые преимущества модернизации
- **Минимальные зависимости**: только `windows`, `axum`, `tracing`, `tokio` + необходимые утилиты
- **Лучшая производительность**: прямые Windows API вызовы без промежуточных слоев
- **Меньший attack surface**: удаление избыточных зависимостей
- **Современные паттерны**: structured logging, WebSocket, типобезопасность
- **Windows 11 нативность**: полное использование современных Windows возможностей

## 7. Verification Checklist
- **Automated:** `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test`
- **Manual QA:**
  - Системный трей: меню только по правому клику, команды выполняются мгновенно
  - Веб-интерфейс: температуры обновляются в реальном времени через WebSocket
  - Конфигурация: изменения мгновенно применяются к монитору и трею
  - Уведомления: успешная доставка Windows toast или fallback в консоль
  - Логирование: structured JSON логи с полным контекстом

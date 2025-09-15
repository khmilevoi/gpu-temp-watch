# План создания GPU Temperature Monitor на Rust

## Текущая проблема с PowerShell версией
- PowerShell процессы запускаются, но быстро завершаются
- Проблемы с BurntToast модулем и execution policy
- Высокое потребление ресурсов (~20MB+ памяти)
- Нестабильная работа в Scheduled Tasks

## Решение: Нативное приложение на Rust

### 1. Архитектура приложения
- **Нативное Windows приложение** с минимальным потреблением ресурсов (~1MB исполняемый файл, <1% CPU)
- **System Tray интерфейс** с цветовой индикацией температуры (🟢🟡🔴)
- **Async HTTP клиент** для LibreHardwareMonitor JSON API (127.0.0.1:8085/data.json)
- **Windows toast notifications** при превышении порога

### 2. Техническая реализация

#### Зависимости для Cargo.toml:
```toml
[package]
name = "gpu-temp-watch"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
trayicon = "0.1"
winrt-notification = "0.5"

[profile.release]
opt-level = "s"
lto = true
panic = "abort"
strip = true
```

#### Структура проекта:
```
gpu-temp-watch/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point + system tray setup
│   ├── monitor.rs       # GPU monitoring logic
│   ├── notifications.rs # Toast notifications
│   └── config.rs        # Configuration
└── resources/
    └── icon.ico         # Tray icon
```

### 3. Ключевые компоненты

#### System Tray:
- Постоянная иконка в трее с цветовой индикацией
- Контекстное меню: настройки, пауза/запуск, выход
- Tooltip с текущей температурой

#### Monitoring Engine:
- Async polling LibreHardwareMonitor JSON API каждые 20 секунд
- Поиск GPU температурных сенсоров по паттернам (`*GPU*Core*`, `*GPU*Hot*`)
- Smart cooldown для уведомлений (20s → 40s → 80s → 160s)

#### Configuration:
- Порог температуры (по умолчанию 60°C)
- Интервал polling (по умолчанию 20s)
- Паттерны для поиска GPU сенсоров

### 4. Команды для разработки

#### Создание проекта:
```bash
cargo new gpu-temp-watch --bin
cd gpu-temp-watch
```

#### Разработка:
```bash
cargo run                    # Запуск в debug режиме
cargo build --release      # Сборка оптимизированной версии
```

#### Оптимизация размера:
```bash
cargo build --release
strip target/release/gpu-temp-watch.exe  # Удаление debug символов
# Финальный размер: ~150KB (без UPX), ~80KB (с UPX)
```

### 5. Установка и автозагрузка
- Создание Windows Service или Scheduled Task
- Автоматический запуск при логине пользователя
- Fallback detection если LibreHardwareMonitor не доступен

### 6. Преимущества над PowerShell версией
- ✅ Мгновенный запуск (нет PowerShell overhead)
- ✅ Минимальное потребление памяти (<2MB vs 20MB+)
- ✅ Нативная интеграция с Windows
- ✅ Самодостаточный исполняемый файл
- ✅ Надежная работа без dependency hell
- ✅ Нет проблем с execution policy
- ✅ Стабильная работа в Scheduled Tasks

### 7. План реализации (TODO)
1. ✅ Установка Rust toolchain
2. [ ] Создание проекта и настройка Cargo.toml
3. [ ] Реализация HTTP клиента для LHM API
4. [ ] Создание system tray интерфейса
5. [ ] Добавление Windows toast notifications
6. [ ] Настройка конфигурации
7. [ ] Создание async monitoring loop
8. [ ] Сборка и тестирование release версии
9. [ ] Создание скрипта установки/автозагрузки

### 8. Пример LibreHardwareMonitor JSON структуры
```json
{
  "Children": [
    {
      "Text": "AMD Radeon RX 6700 XT",
      "Children": [
        {
          "Text": "Temperatures",
          "Children": [
            {
              "Text": "GPU Core",
              "Value": "72.5 °C",
              "Min": "31.0 °C",
              "Max": "84.0 °C"
            }
          ]
        }
      ]
    }
  ]
}
```

Этот план решает все проблемы текущей PowerShell реализации и создает быстрое, надежное решение.
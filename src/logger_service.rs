use chrono::{DateTime, Local};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::SystemTime;
use crate::app_paths::AppPaths;

/// Уровни логирования в порядке важности
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            LogLevel::Trace => "🔍",
            LogLevel::Debug => "🐛",
            LogLevel::Info => "ℹ️",
            LogLevel::Warn => "⚠️",
            LogLevel::Error => "❌",
        }
    }

    pub fn color_code(&self) -> &'static str {
        match self {
            LogLevel::Trace => "\x1b[37m", // Белый
            LogLevel::Debug => "\x1b[36m", // Циан
            LogLevel::Info => "\x1b[32m",  // Зеленый
            LogLevel::Warn => "\x1b[33m",  // Желтый
            LogLevel::Error => "\x1b[31m", // Красный
        }
    }
}

/// Варианты вывода логов
#[derive(Debug, Clone, Copy)]
pub enum LogOutput {
    Console,
    File,
    Both,
}

/// Формат вывода логов
#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    Human,      // Человекочитаемый формат
    Json,       // JSON формат
    Structured, // Структурированный формат
}

/// Структура записи лога
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub context: Option<Value>,
    pub correlation_id: String,
    pub thread_id: String,
    pub process_id: u32,
}

impl LogEntry {
    pub fn new(level: LogLevel, module: &str, message: &str, context: Option<Value>) -> Self {
        Self {
            timestamp: Local::now(),
            level,
            module: module.to_string(),
            message: message.to_string(),
            context,
            correlation_id: generate_correlation_id(),
            thread_id: format!("{:?}", std::thread::current().id()),
            process_id: std::process::id(),
        }
    }

    /// Форматирование для консоли (человекочитаемый)
    pub fn format_human(&self, colored: bool) -> String {
        let timestamp = self.timestamp.format("%H:%M:%S%.3f");
        let level_str = if colored {
            format!("{}{}\x1b[0m", self.level.color_code(), self.level.as_str())
        } else {
            self.level.as_str().to_string()
        };

        let context_str = if let Some(ref ctx) = self.context {
            format!(" | {}", ctx)
        } else {
            String::new()
        };

        format!("{} {} [{}] {}: {}{}",
                timestamp,
                self.level.emoji(),
                level_str,
                self.module,
                self.message,
                context_str)
    }

    /// Форматирование для файла (JSON)
    pub fn format_json(&self) -> String {
        let entry = json!({
            "timestamp": self.timestamp.to_rfc3339(),
            "level": self.level.as_str(),
            "module": self.module,
            "message": self.message,
            "context": self.context.clone().unwrap_or(json!({})),
            "correlation_id": self.correlation_id,
            "thread_id": self.thread_id,
            "process_id": self.process_id
        });
        entry.to_string()
    }

    /// Структурированный формат для файла
    pub fn format_structured(&self) -> String {
        let timestamp = self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        let context_str = if let Some(ref ctx) = self.context {
            format!(" | context: {}", ctx)
        } else {
            String::new()
        };

        format!("[{}] {} [{}:{}] {}{}",
                timestamp,
                self.level.as_str(),
                self.module,
                self.thread_id,
                self.message,
                context_str)
    }
}

/// Конфигурация логгера
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    pub min_level: LogLevel,
    pub output: LogOutput,
    pub console_format: LogFormat,
    pub file_format: LogFormat,
    pub file_path: Option<PathBuf>,
    pub max_file_size: Option<u64>,  // Максимальный размер файла в байтах
    pub max_files: Option<u32>,      // Максимальное количество файлов ротации
    pub colored_output: bool,
    pub enabled: bool,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            output: LogOutput::Both,
            console_format: LogFormat::Human,
            file_format: LogFormat::Json,
            file_path: Some(
                AppPaths::get_log_file_path()
                    .unwrap_or_else(|_| AppPaths::get_fallback_log_path())
            ),
            max_file_size: Some(10 * 1024 * 1024), // 10MB
            max_files: Some(5),
            colored_output: true,
            enabled: true,
        }
    }
}

/// Основной сервис логирования
pub struct LoggerService {
    config: LoggerConfig,
}

impl LoggerService {
    pub fn new(config: LoggerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let service = Self { config };

        // Создать директорию для логов если нужно
        if let Some(ref path) = service.config.file_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        Ok(service)
    }

    /// Основная функция логирования
    pub fn log(&self, level: LogLevel, module: &str, message: &str, context: Option<Value>) {
        if !self.config.enabled || level < self.config.min_level {
            return;
        }

        let entry = LogEntry::new(level, module, message, context);

        match self.config.output {
            LogOutput::Console => self.log_to_console(&entry),
            LogOutput::File => self.log_to_file(&entry),
            LogOutput::Both => {
                self.log_to_console(&entry);
                self.log_to_file(&entry);
            }
        }
    }

    /// Вывод в консоль
    fn log_to_console(&self, entry: &LogEntry) {
        let formatted = match self.config.console_format {
            LogFormat::Human => entry.format_human(self.config.colored_output),
            LogFormat::Json => entry.format_json(),
            LogFormat::Structured => entry.format_structured(),
        };

        match entry.level {
            LogLevel::Error => eprintln!("{}", formatted),
            _ => println!("{}", formatted),
        }
    }

    /// Вывод в файл
    fn log_to_file(&self, entry: &LogEntry) {
        if let Some(ref file_path) = self.config.file_path {
            if let Err(e) = self.write_to_file(file_path, entry) {
                eprintln!("Logger: Failed to write to file {}: {}", file_path.display(), e);
            }
        }
    }

    /// Запись в файл с обработкой ротации
    fn write_to_file(&self, file_path: &Path, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        // Проверить размер файла для ротации
        if let Some(max_size) = self.config.max_file_size {
            if file_path.exists() {
                let metadata = fs::metadata(file_path)?;
                if metadata.len() >= max_size {
                    self.rotate_logs(file_path)?;
                }
            }
        }

        let formatted = match self.config.file_format {
            LogFormat::Human => entry.format_human(false),
            LogFormat::Json => entry.format_json(),
            LogFormat::Structured => entry.format_structured(),
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        writeln!(file, "{}", formatted)?;
        Ok(())
    }

    /// Ротация логов
    fn rotate_logs(&self, file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(max_files) = self.config.max_files {
            let base_path = file_path.with_extension("");
            let extension = file_path.extension().unwrap_or_default();

            // Сдвинуть существующие файлы
            for i in (1..max_files).rev() {
                let old_path = if i == 1 {
                    file_path.to_path_buf()
                } else {
                    base_path.with_extension(format!("{}.{}", i, extension.to_string_lossy()))
                };

                let new_path = base_path.with_extension(format!("{}.{}", i + 1, extension.to_string_lossy()));

                if old_path.exists() {
                    if i == max_files - 1 {
                        // Удалить самый старый файл
                        fs::remove_file(old_path)?;
                    } else {
                        // Переименовать файл
                        fs::rename(old_path, new_path)?;
                    }
                }
            }

            // Переименовать текущий файл
            let rotated_path = base_path.with_extension(format!("1.{}", extension.to_string_lossy()));
            if file_path.exists() {
                fs::rename(file_path, rotated_path)?;
            }
        }

        Ok(())
    }

    /// Принудительная запись всех буферизованных данных
    pub fn flush(&self) {
        // В текущей реализации буферизация не используется
        // Но метод оставлен для будущих улучшений
    }

    /// Обновить конфигурацию
    pub fn update_config(&mut self, config: LoggerConfig) -> Result<(), Box<dyn std::error::Error>> {
        // Создать директорию для нового пути если нужно
        if let Some(ref path) = config.file_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        self.config = config;
        Ok(())
    }

    pub fn get_config(&self) -> &LoggerConfig {
        &self.config
    }
}

/// Глобальный экземпляр логгера
static GLOBAL_LOGGER: OnceLock<Arc<Mutex<LoggerService>>> = OnceLock::new();

/// Инициализация глобального логгера
pub fn init_logger(config: LoggerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let logger = LoggerService::new(config)?;

    GLOBAL_LOGGER
        .set(Arc::new(Mutex::new(logger)))
        .map_err(|_| "Logger already initialized")?;

    Ok(())
}

/// Получить ссылку на глобальный логгер
pub fn get_logger() -> &'static Arc<Mutex<LoggerService>> {
    GLOBAL_LOGGER.get().expect("Logger not initialized")
}

/// Генерация correlation ID для трассировки
fn generate_correlation_id() -> String {
    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Удобные функции для логирования
pub fn log_trace(module: &str, message: &str, context: Option<Value>) {
    if let Ok(logger) = get_logger().lock() {
        logger.log(LogLevel::Trace, module, message, context);
    }
}

pub fn log_debug(module: &str, message: &str, context: Option<Value>) {
    if let Ok(logger) = get_logger().lock() {
        logger.log(LogLevel::Debug, module, message, context);
    }
}

pub fn log_info(module: &str, message: &str, context: Option<Value>) {
    if let Ok(logger) = get_logger().lock() {
        logger.log(LogLevel::Info, module, message, context);
    }
}

pub fn log_warn(module: &str, message: &str, context: Option<Value>) {
    if let Ok(logger) = get_logger().lock() {
        logger.log(LogLevel::Warn, module, message, context);
    }
}

pub fn log_error(module: &str, message: &str, context: Option<Value>) {
    if let Ok(logger) = get_logger().lock() {
        logger.log(LogLevel::Error, module, message, context);
    }
}

/// Flush всех логов
pub fn flush_logs() {
    if let Ok(logger) = get_logger().lock() {
        logger.flush();
    }
}

/// Специальные функции для событий приложения
pub fn log_startup(version: &str, args: &[String]) {
    log_info("startup", &format!("🚀 GPU Temperature Monitor {} starting up", version),
             Some(json!({
                 "version": version,
                 "args": args,
                 "startup_time": Local::now().to_rfc3339()
             })));
}

pub fn log_shutdown(reason: &str) {
    log_info("shutdown", &format!("🚪 Application shutting down: {}", reason),
             Some(json!({
                 "reason": reason,
                 "shutdown_time": Local::now().to_rfc3339()
             })));
}

pub fn log_temperature(sensor: &str, temp: f32, threshold: f32) {
    let level = if temp > threshold { LogLevel::Warn } else { LogLevel::Info };
    let status = if temp > threshold { "HOT" } else { "OK" };

    if let Ok(logger) = get_logger().lock() {
        logger.log(level, "temperature",
                  &format!("{}: {:.1}°C ({})", sensor, temp, status),
                  Some(json!({
                      "sensor": sensor,
                      "temperature": temp,
                      "threshold": threshold,
                      "status": status
                  })));
    }
}

/// Макросы для удобного использования
#[macro_export]
macro_rules! log_error {
    ($msg:expr) => {
        $crate::logger_service::log_error(module_path!(), $msg, None)
    };
    ($msg:expr, $context:expr) => {
        $crate::logger_service::log_error(module_path!(), $msg, Some($context))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => {
        $crate::logger_service::log_warn(module_path!(), $msg, None)
    };
    ($msg:expr, $context:expr) => {
        $crate::logger_service::log_warn(module_path!(), $msg, Some($context))
    };
}

#[macro_export]
macro_rules! log_info {
    ($msg:expr) => {
        $crate::logger_service::log_info(module_path!(), $msg, None)
    };
    ($msg:expr, $context:expr) => {
        $crate::logger_service::log_info(module_path!(), $msg, Some($context))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => {
        $crate::logger_service::log_debug(module_path!(), $msg, None)
    };
    ($msg:expr, $context:expr) => {
        $crate::logger_service::log_debug(module_path!(), $msg, Some($context))
    };
}

#[macro_export]
macro_rules! log_trace {
    ($msg:expr) => {
        $crate::logger_service::log_trace(module_path!(), $msg, None)
    };
    ($msg:expr, $context:expr) => {
        $crate::logger_service::log_trace(module_path!(), $msg, Some($context))
    };
}

/// Специальные макросы для событий
#[macro_export]
macro_rules! log_startup {
    ($version:expr, $args:expr) => {
        $crate::logger_service::log_startup($version, $args)
    };
}

#[macro_export]
macro_rules! log_shutdown {
    ($reason:expr) => {
        $crate::logger_service::log_shutdown($reason)
    };
}

#[macro_export]
macro_rules! log_temperature {
    ($sensor:expr, $temp:expr, $threshold:expr) => {
        $crate::logger_service::log_temperature($sensor, $temp, $threshold)
    };
}
use chrono::Local;
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::{debug, error, info, warn};

/// Универсальный логгер для одновременного вывода в консоль и файл
pub struct UniversalLogger {
    file_path: Option<PathBuf>,
    enabled: bool,
}

impl UniversalLogger {
    pub fn new(file_path: Option<&str>, enabled: bool) -> Self {
        Self {
            file_path: file_path.map(PathBuf::from),
            enabled,
        }
    }

    pub fn log(
        &self,
        level: LogLevel,
        module: &str,
        message: &str,
        context: Option<serde_json::Value>,
    ) {
        if !self.enabled {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let correlation_id = generate_correlation_id();

        // Structured log entry
        let log_entry = json!({
            "timestamp": timestamp.to_string(),
            "level": level.as_str(),
            "module": module,
            "message": message,
            "correlation_id": correlation_id,
            "context": context.unwrap_or(json!({})),
            "pid": std::process::id(),
            "thread": format!("{:?}", std::thread::current().id())
        });

        // Console output (через tracing)
        match level {
            LogLevel::Error => error!("{}: {}", module, message),
            LogLevel::Warn => warn!("{}: {}", module, message),
            LogLevel::Info => info!("{}: {}", module, message),
            LogLevel::Debug => debug!("{}: {}", module, message),
        }

        // File output (JSON format)
        if let Some(ref file_path) = self.file_path {
            if let Err(e) = self.write_to_file(file_path, &log_entry) {
                error!("Failed to write to log file {}: {}", file_path.display(), e);
            }
        }
    }

    fn write_to_file(
        &self,
        file_path: &PathBuf,
        log_entry: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        writeln!(file, "{}", log_entry.to_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
        }
    }
}

pub static GLOBAL_LOGGER: OnceLock<Arc<Mutex<UniversalLogger>>> = OnceLock::new();

/// Initialize the global logger
pub fn init_logger(file_path: Option<&str>, enabled: bool) {
    let logger = UniversalLogger::new(file_path, enabled);
    GLOBAL_LOGGER
        .set(Arc::new(Mutex::new(logger)))
        .map_err(|_| "Logger already initialized")
        .unwrap();
}

/// Get reference to global logger
pub fn get_logger() -> &'static Arc<Mutex<UniversalLogger>> {
    GLOBAL_LOGGER.get().expect("Logger not initialized")
}

/// Generate unique correlation ID for request tracing
fn generate_correlation_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Macro for convenient logging with automatic module detection
#[macro_export]
macro_rules! log_both {
    (error, $msg:expr) => {
        log_both!(error, $msg, None)
    };
    (error, $msg:expr, $context:expr) => {
        if let Some(logger) = $crate::universal_logger::GLOBAL_LOGGER.get() {
            if let Ok(logger) = logger.lock() {
                logger.log(
                    $crate::universal_logger::LogLevel::Error,
                    module_path!(),
                    $msg,
                    $context,
                );
            }
        }
    };

    (warn, $msg:expr) => {
        log_both!(warn, $msg, None)
    };
    (warn, $msg:expr, $context:expr) => {
        if let Some(logger) = $crate::universal_logger::GLOBAL_LOGGER.get() {
            if let Ok(logger) = logger.lock() {
                logger.log(
                    $crate::universal_logger::LogLevel::Warn,
                    module_path!(),
                    $msg,
                    $context,
                );
            }
        }
    };

    (info, $msg:expr) => {
        log_both!(info, $msg, None)
    };
    (info, $msg:expr, $context:expr) => {
        if let Some(logger) = $crate::universal_logger::GLOBAL_LOGGER.get() {
            if let Ok(logger) = logger.lock() {
                logger.log(
                    $crate::universal_logger::LogLevel::Info,
                    module_path!(),
                    $msg,
                    $context,
                );
            }
        }
    };

    (debug, $msg:expr) => {
        log_both!(debug, $msg, None)
    };
    (debug, $msg:expr, $context:expr) => {
        if let Some(logger) = $crate::universal_logger::GLOBAL_LOGGER.get() {
            if let Ok(logger) = logger.lock() {
                logger.log(
                    $crate::universal_logger::LogLevel::Debug,
                    module_path!(),
                    $msg,
                    $context,
                );
            }
        }
    };
}

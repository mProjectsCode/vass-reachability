use std::{
    fmt::Display,
    fs::File,
    io::{BufWriter, Write},
    str::FromStr,
    sync::Mutex,
};

use chrono::Local;
use colored::{ColoredString, Colorize};
use serde::{Deserialize, Serialize};

use crate::config::LoggerConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn to_string(&self) -> ColoredString {
        match self {
            LogLevel::Debug => "DBG".bright_cyan(),
            LogLevel::Info => "INF".bright_green(),
            LogLevel::Warn => "WAR".yellow(),
            LogLevel::Error => "ERR".bright_red(),
        }
    }

    pub fn to_string_no_color(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DBG",
            LogLevel::Info => "INF",
            LogLevel::Warn => "WAR",
            LogLevel::Error => "ERR",
        }
    }

    pub fn show(&self, other: &LogLevel) -> bool {
        match self {
            LogLevel::Debug => *other == LogLevel::Debug,
            LogLevel::Info => *other == LogLevel::Debug || *other == LogLevel::Info,
            LogLevel::Warn => *other != LogLevel::Error,
            LogLevel::Error => true,
        }
    }
}

impl FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" | "dbg" => Ok(LogLevel::Debug),
            "info" | "inf" => Ok(LogLevel::Info),
            "warn" | "warning" | "war" => Ok(LogLevel::Warn),
            "error" | "err" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "Debug"),
            LogLevel::Info => write!(f, "Info"),
            LogLevel::Warn => write!(f, "Warn"),
            LogLevel::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug)]
pub struct Logger {
    level: LogLevel,
    file: Option<Mutex<BufWriter<File>>>,
    debug_prefix: String,
    info_prefix: String,
    warn_prefix: String,
    error_prefix: String,
    debug_prefix_no_color: String,
    info_prefix_no_color: String,
    warn_prefix_no_color: String,
    error_prefix_no_color: String,
}

impl Logger {
    pub fn new(level: LogLevel, name: String, log_file_path: Option<String>) -> Self {
        let n = format!("{name}:").dimmed();
        let n_no_color = format!("{name}:");
        let file = log_file_path.map(|path| {
            let file = File::create(path).unwrap();
            Mutex::new(BufWriter::new(file))
        });

        Logger {
            level,
            file,
            debug_prefix: format!("[{}] {}", LogLevel::Debug.to_string(), n),
            info_prefix: format!("[{}] {}", LogLevel::Info.to_string(), n),
            warn_prefix: format!("[{}] {}", LogLevel::Warn.to_string(), n),
            error_prefix: format!("[{}] {}", LogLevel::Error.to_string(), n),
            debug_prefix_no_color: format!(
                "[{}] {}",
                LogLevel::Debug.to_string_no_color(),
                n_no_color
            ),
            info_prefix_no_color: format!(
                "[{}] {}",
                LogLevel::Info.to_string_no_color(),
                n_no_color
            ),
            warn_prefix_no_color: format!(
                "[{}] {}",
                LogLevel::Warn.to_string_no_color(),
                n_no_color
            ),
            error_prefix_no_color: format!(
                "[{}] {}",
                LogLevel::Error.to_string_no_color(),
                n_no_color
            ),
        }
    }

    pub fn from_config(config: &LoggerConfig, name: String) -> Option<Self> {
        if !*config.get_enabled() {
            return None;
        }

        let log_file_path = if *config.get_log_file() {
            Some(format!(
                "./logs/solver_run_{}.txt",
                Local::now().format("%Y-%m-%d_%H-%M-%S")
            ))
        } else {
            None
        };

        Some(Logger::new(*config.get_log_level(), name, log_file_path))
    }

    pub fn get_prefix(&self, level: &LogLevel) -> &str {
        match level {
            LogLevel::Debug => &self.debug_prefix,
            LogLevel::Info => &self.info_prefix,
            LogLevel::Warn => &self.warn_prefix,
            LogLevel::Error => &self.error_prefix,
        }
    }

    pub fn get_prefix_no_color(&self, level: &LogLevel) -> &str {
        match level {
            LogLevel::Debug => &self.debug_prefix_no_color,
            LogLevel::Info => &self.info_prefix_no_color,
            LogLevel::Warn => &self.warn_prefix_no_color,
            LogLevel::Error => &self.error_prefix_no_color,
        }
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        let msg = format!("{} {}", self.get_prefix(&level), message);
        let msg_no_color = format!("{} {}", self.get_prefix_no_color(&level), message);

        self.writeln_to_file(&msg_no_color);
        if level.show(&self.level) {
            eprintln!("{}", msg);
        }
    }

    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    pub fn warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }

    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    pub fn empty(&self, level: LogLevel) {
        self.writeln_to_file("");
        if level.show(&self.level) {
            eprintln!();
        }
    }

    fn writeln_to_file(&self, string: &str) {
        if let Some(file) = &self.file {
            let mut f = file.lock().unwrap();

            f.write_all(string.as_bytes()).unwrap();
            f.write_all(b"\n").unwrap();
        }
    }

    pub fn object<'a>(&'a self, name: &'a str) -> ObjectBuilder<'a> {
        ObjectBuilder::new(name, self)
    }
}

// impl Drop for Logger {
//     fn drop(&mut self) {
//         if let Some(file) = &self.file {
//             file.borrow_mut().flush().unwrap();
//         }
//     }
// }

#[derive(Debug, Clone)]
pub struct ObjectBuilder<'a> {
    logger: &'a Logger,
    name: &'a str,
    fields: Vec<(&'a str, &'a str)>,
}

impl<'a> ObjectBuilder<'a> {
    fn new(name: &'a str, logger: &'a Logger) -> Self {
        ObjectBuilder {
            logger,
            name,
            fields: vec![],
        }
    }

    pub fn add_field(mut self, name: &'a str, value: &'a str) -> Self {
        self.fields.push((name, value));

        self
    }

    fn build(&self) -> String {
        let mut result = format!("{} {{", self.name);
        for (name, value) in &self.fields {
            result.push_str(&format!("\n  {}: {}", name, value));
        }
        result.push_str("\n}");
        result
    }

    pub fn log(&self, level: LogLevel) {
        self.logger.log(level, &self.build());
    }
}

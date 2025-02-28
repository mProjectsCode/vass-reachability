use std::{
    cell::RefCell,
    fs::File,
    io::{BufWriter, Write},
};

use colored::{ColoredString, Colorize};

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct Logger {
    level: LogLevel,
    file: Option<RefCell<BufWriter<File>>>,
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
            RefCell::new(BufWriter::new(file))
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
            println!("{}", msg);
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
            println!();
        }
    }

    fn writeln_to_file(&self, string: &str) {
        if let Some(file) = &self.file {
            let mut f = file.borrow_mut();

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

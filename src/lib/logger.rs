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

    pub fn show(&self, other: &LogLevel) -> bool {
        match self {
            LogLevel::Debug => *other == LogLevel::Debug,
            LogLevel::Info => *other == LogLevel::Debug || *other == LogLevel::Info,
            LogLevel::Warn => *other != LogLevel::Error,
            LogLevel::Error => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logger {
    level: LogLevel,
    name: String,
    debug_prefix: String,
    info_prefix: String,
    warn_prefix: String,
    error_prefix: String,
}

impl Logger {
    pub fn new(level: LogLevel, name: String) -> Self {
        let n = format!("{name}:").dimmed();

        Logger {
            level,
            name,
            debug_prefix: format!("[{}] {}", LogLevel::Debug.to_string(), n),
            info_prefix: format!("[{}] {}", LogLevel::Info.to_string(), n),
            warn_prefix: format!("[{}] {}", LogLevel::Warn.to_string(), n),
            error_prefix: format!("[{}] {}", LogLevel::Error.to_string(), n),
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

    pub fn log(&self, level: LogLevel, message: &str) {
        if level.show(&self.level) {
            println!("{} {}", self.get_prefix(&level), message);
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
        if level.show(&self.level) {
            println!();
        }
    }

    pub fn object<'a>(&'a self, name: &'a str) -> ObjectBuilder<'a> {
        ObjectBuilder::new(name, self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

use std::path::Path;

use crate::logger::LogLevel;

/// Define a config struct.
/// The first parameter is the name of the config struct.
/// Any further parameter is a tuple describing a config field.
macro_rules! config {
    ($struct_name:ident, $( ($field:ident, $field_type:ty, $partial_field_type:ty, $default:expr) ),*) => {
        paste::paste! {
            #[derive(Clone, serde::Serialize)]
            pub struct $struct_name {
                $(
                    $field: $field_type,
                )*
            }

            #[derive(Clone, serde::Deserialize)]
            pub struct [<Partial$struct_name>] {
                $(
                    $field: $partial_field_type,
                )*
            }

            impl $struct_name {

                pub fn from_partial(partial: [<Partial$struct_name>]) -> Self {
                    Self {
                        $(
                            $field: partial.$field.into_or($default),
                        )*
                    }
                }

                pub fn from_file<P: AsRef<Path>>(file: P) -> anyhow::Result<Self> {
                    
                }

                $(
                    pub fn [<with_$field>](mut self, $field: $field_type) -> Self {
                        self.$field = $field;
                        self
                    }

                    pub fn [<set_$field>](&mut self, $field: $field_type) {
                        self.$field = $field;
                    }

                    pub fn [<get_$field>](&self) -> &$field_type {
                        &self.$field
                    }
                )*
            }

            impl Default for $struct_name {
                fn default() -> Self {
                    $struct_name {
                        $(
                            $field: $default,
                        )*
                    }
                }
            }

            impl IntoOr<$struct_name> for Option<[<Partial$struct_name>]> {
                fn into_or(self, or: $struct_name) -> $struct_name {
                    match self {
                        Some(t) => $struct_name::from_partial(t),
                        None => or
                    } 
                }
            }
        }
    };
}

pub trait IntoOr<T> {
    fn into_or(self, or: T) -> T;
}

impl<T> IntoOr<Option<T>> for Option<T> {
    fn into_or(self, or: Option<T>) -> Option<T> {
        match self {
            Some(t) => Some(t),
            None => or
        }
    }
}

impl<T> IntoOr<T> for Option<T> {
    fn into_or(self, or: T) -> T {
        self.unwrap_or(or)
    }
}

pub trait GeneralConfig {
    fn logger(&self) -> &LoggerConfig;
}

config!(LoggerConfig, 
    (enabled, bool, Option<bool>, false),
    (log_file, Option<String>, Option<String>, None),
    (log_level, LogLevel, Option<LogLevel>, LogLevel::Warn)
);

config!(VASSReachConfig,
    (timeout, Option<std::time::Duration>, Option<std::time::Duration>, None),
    (max_iterations, Option<u64>, Option<u64>, None),
    (logger, LoggerConfig, Option<PartialLoggerConfig>, LoggerConfig::default())
);

impl GeneralConfig for VASSReachConfig {
    fn logger(&self) -> &LoggerConfig {
        &self.logger
    }
}


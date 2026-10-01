use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub struct ConfigErrorItem {
    pub path: String,
    pub field: String,
    pub error: ConfigError,
}

impl fmt::Display for ConfigErrorItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.path, self.field, self.error)
    }
}

#[derive(Debug)]
pub struct ConfigErrorList(pub Vec<ConfigErrorItem>);

impl fmt::Display for ConfigErrorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "found {} config error(s):", self.0.len())?;
        for (i, e) in self.0.iter().enumerate() {
            write!(f, "  {}. {e}", i + 1)?;
            if i + 1 < self.0.len() {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

impl<T> From<std::sync::PoisonError<T>> for PortalError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        PortalError::LockPoisoned
    }
}

#[derive(thiserror::Error, Debug)]
pub enum PortalError {
    // config
    #[error("Failed to parse config: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("Failed to serialize: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Invalid config: {0}")]
    InvalidConfigMulti(ConfigErrorList),
    #[error("Invalid picker command: {0}")]
    InvalidPickerCmd(String),

    // environment
    #[error("Environment error: {0}")]
    Environment(String),

    // system io
    #[error("invalid path: {0:?}")]
    InvalidPath(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("failed to persist state file: {0}")]
    Persist(#[from] tempfile::PersistError),
    #[error("internal state lock was poisoned")]
    LockPoisoned,

    // picker output
    #[error("invalid picker output, {0}")]
    InvalidPickerOutput(String),
    #[error("no picker output")]
    NoPickerOutput,
    #[error("empty picker output")]
    EmptyPickerOutput,

    // backend
    #[error("Invalid request: {0}")]
    InvalidRequest(&'static str),
    #[error("Failed to generate response: {0}")]
    Response(String),
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("invalid conditional token: too many branch separators (:)")]
    TooManyBranchSeparators,
    #[error("cannot close '}}': an unclosed '\"' block is still open and must be closed first")]
    UnclosedQuoteBeforeBrace,
    #[error("stray '}}' detected in template")]
    StrayCloseBrace,
    #[error("invalid runaway token: {{{0}")]
    RunawayToken(String),
    #[error("unknown or invalid token '{{{0}}}' found in config")]
    UnknownToken(String),
    #[error("conditional token '{{{0}}}' is missing the ':' delimiter")]
    MissingConditionalDelimiter(String),
    #[error("unmatched '{{' detected in template")]
    UnmatchedOpenBrace,
    #[error("unmatched '\"' detected in template")]
    UnmatchedQuote,
    #[error("unmatched '\\'' detected in template")]
    UnmatchedSingleQuote,
    #[error("must be an absolute path")]
    NotAbsolute,
    #[error("is not a directory")]
    NotADirectory,
}

impl PortalError {
    pub fn exit_code(&self) -> i32 {
        match self {
            // EX_CONFIG (78): bad configuration — user error
            Self::TomlDe(_)
            | Self::TomlSer(_)
            | Self::InvalidConfig(_)
            | Self::InvalidConfigMulti(_)
            | Self::InvalidPickerCmd(_) => 78,
            // EX_OSFILE (72): broken environment — admin error
            Self::Environment(_) => 72,
            // transient error
            _ => 1,
        }
    }
}

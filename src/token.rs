use std::path::PathBuf;
use std::{ffi::OsStr, ffi::OsString};

use crate::context::FileChooserContext;
use crate::error::{ConfigError, PortalError};
use crate::options::FileChooserOptions;

macro_rules! define_tokens {
    ($($name:ident),* $(,)?) => {
        $(
            pub const $name: &str = stringify!($name);
        )*

        pub const VALID_TOKENS: &[&str] = &[
            $(
                $name,
            )*
        ];
    };
}

/// Bound for runaway token
pub const MAX_TOKEN_LEN: usize = 16;

define_tokens!(
    METHOD,
    APP_ID,
    WINDOW_ID,
    TITLE,
    ACCEPT_LABEL,
    MODAL,
    MULTIPLE,
    DIRECTORY,
    CURRENT_NAME,
    CURRENT_FOLDER,
    CURRENT_FILE,
    REQ_PATH,
    OUT_PATH,
    IN_PATH,
    IN_PATH_DIR,
    IN_PATH_NAME,
    HOME_DIR,
    DEFAULT_DIR,
    LAST_DIR,
    APP_LAST_DIR,
    SUGGESTED_PATH,
    LAST_PATH,
    APP_LAST_PATH,
);

pub struct TokenResolver<'a> {
    ctx: &'a FileChooserContext,
    opts: &'a FileChooserOptions,
}

impl<'a> TokenResolver<'a> {
    pub fn new(ctx: &'a FileChooserContext, opts: &'a FileChooserOptions) -> Self {
        Self { ctx, opts }
    }

    /// Resolve a token
    pub fn resolve(&self, name: &str, use_defaults: bool) -> Result<OsString, PortalError> {
        let resolve_bool = |val: Option<bool>, default_val: bool| match val
            .or(use_defaults.then_some(default_val))
        {
            Some(true) => OsString::from("true"),
            Some(false) => OsString::from("false"),
            None => OsString::new(),
        };

        let result = match name {
            METHOD => self.ctx.method.clone().into(),
            APP_ID => self.ctx.app_id.clone().into(),
            WINDOW_ID => self.ctx.window_id.clone().into(),
            TITLE => self.ctx.title.clone().into(),
            ACCEPT_LABEL => self.opts.accept_label.clone().unwrap_or_default().into(),
            MODAL => resolve_bool(self.opts.modal, true),
            MULTIPLE => resolve_bool(self.opts.multiple, false),
            DIRECTORY => resolve_bool(self.opts.directory, false),
            CURRENT_NAME => self.opts.current_name.clone().unwrap_or_default().into(),
            CURRENT_FOLDER => self
                .opts
                .current_folder_path()
                .unwrap_or_default()
                .into_os_string(),
            CURRENT_FILE => self
                .opts
                .current_file_path()
                .unwrap_or_default()
                .into_os_string(),
            REQ_PATH => self.ctx.req_path.clone().into_os_string(),
            OUT_PATH => self.ctx.out_path.clone().into_os_string(),
            IN_PATH => self
                .ctx
                .in_path
                .clone()
                .unwrap_or_default()
                .into_os_string(),
            IN_PATH_DIR => self
                .ctx
                .suggested_path
                .parent()
                .map(|p| p.as_os_str().to_os_string())
                .unwrap_or_default(),
            IN_PATH_NAME => self
                .ctx
                .suggested_path
                .file_name()
                .map(OsStr::to_os_string)
                .unwrap_or_default(),
            HOME_DIR => self.ctx.home_dir.clone().into_os_string(),
            DEFAULT_DIR => self.ctx.default_dir.clone().into_os_string(),
            LAST_DIR => self.ctx.last_dir.clone().into_os_string(),
            APP_LAST_DIR => self.ctx.app_last_dir.clone().into_os_string(),
            SUGGESTED_PATH => self.ctx.suggested_path.clone().into_os_string(),
            LAST_PATH => self.ctx.last_path.clone().into_os_string(),
            APP_LAST_PATH => self.ctx.app_last_path.clone().into_os_string(),
            _ => {
                return Err(PortalError::InvalidPickerCmd(format!(
                    "invalid token: {{{name}}}"
                )));
            }
        };

        Ok(result)
    }

    /// Expand a config template
    fn expand_template(&self, raw: &str, split: bool) -> Result<Vec<OsString>, PortalError> {
        let resolved = self.resolve_condition(raw)?;

        let parts: Vec<String> = if split {
            shlex::split(&resolved)
                .ok_or_else(|| PortalError::InvalidPickerCmd(format!("can't split '{resolved}'")))?
        } else {
            vec![resolved]
        };

        parts
            .into_iter()
            .map(|token| {
                let expanded = shellexpand::full_with_context(
                    &token,
                    || {
                        Some(
                            self.resolve(HOME_DIR, true)
                                .expect("home dir defined")
                                .into_string()
                                .expect("home dir valid"),
                        )
                    },
                    |var| {
                        Ok::<_, std::convert::Infallible>(Some(
                            std::env::var(var).unwrap_or_default(),
                        ))
                    },
                )
                .map_err(|e| {
                    PortalError::InvalidPickerCmd(format!(
                        "failed to expand '{}' in: {token}",
                        e.var_name
                    ))
                })?;

                self.parse_token(expanded.as_ref())
            })
            .collect()
    }

    /// Parse a command/args config string
    pub fn parse_cmd(&self, raw: &str) -> Result<Vec<OsString>, PortalError> {
        self.expand_template(raw, true)
    }

    /// Parse a single path config value
    pub fn parse_path(&self, raw: &str) -> Result<Option<PathBuf>, PortalError> {
        let paths = self.expand_template(raw, false)?;

        match paths.into_iter().next().map(PathBuf::from) {
            Some(path) if path.is_absolute() => Ok(Some(path)),
            Some(path) => Err(PortalError::InvalidPickerCmd(format!(
                "path is not absolute: {path:?}"
            ))),
            None => Ok(None),
        }
    }

    /// Parse a single string config value
    pub fn parse_value(&self, raw: &str) -> Result<Option<OsString>, PortalError> {
        let parsed = self.expand_template(raw, false)?;

        Ok(parsed.into_iter().next())
    }

    fn resolve_condition(&self, raw: &str) -> Result<String, PortalError> {
        let mut chars = raw.chars();
        let mut result = String::with_capacity(raw.len());

        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(next_c) = chars.next() {
                        result.push('\\');
                        result.push(next_c);
                    } else {
                        result.push('\\');
                    }
                }
                '{' => {
                    self.parse_condition(&mut chars, &mut result)?;
                }
                c => result.push(c),
            }
        }
        Ok(result)
    }

    fn parse_condition(
        &self,
        chars: &mut std::str::Chars,
        out: &mut String,
    ) -> Result<(), PortalError> {
        let mut inner = String::new();

        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    inner.push('\\');
                    if let Some(next_char) = chars.next() {
                        inner.push(next_char);
                    }
                }
                '{' => {
                    self.parse_condition(chars, &mut inner)?;
                }
                '}' => {
                    if let Some(rest) = inner.strip_prefix('?') {
                        self.evaluate_condition('?', rest, out);
                    } else if let Some(rest) = inner.strip_prefix('#') {
                        self.evaluate_condition('#', rest, out);
                    } else {
                        out.push('{');
                        out.push_str(&inner);
                        out.push('}');
                    }
                    return Ok(());
                }
                c => inner.push(c),
            }
        }

        unreachable!("unclosed '{{' in template should have been caught by validate_all")
    }

    fn evaluate_condition(&self, op: char, rest: &str, out: &mut String) {
        let mut parts = rest.splitn(3, ':');
        let key = parts.next().unwrap_or("");
        let if_true = parts.next().unwrap_or_else(|| {
            unreachable!("conditional without ':' should have been caught by validate_all");
        });
        let if_false = parts.next().unwrap_or("");

        let use_defaults = op == '?';

        let value = self.resolve(key, use_defaults).unwrap_or_default();

        let condition_met = match op {
            '?' => !value.is_empty() && value != "false",
            '#' => !value.is_empty(),
            _ => unreachable!(),
        };

        if condition_met {
            out.push_str(if_true);
        } else {
            out.push_str(if_false);
        }
    }

    fn parse_token(&self, expanded: &str) -> Result<OsString, PortalError> {
        let mut result = OsString::new();
        let mut chars = expanded.chars();
        let mut current_literal = String::new();

        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(next_char) = chars.next() {
                        current_literal.push(next_char);
                    } else {
                        // trailing backslash
                        current_literal.push('\\');
                    }
                }
                '{' => {
                    if !current_literal.is_empty() {
                        result.push(&current_literal);
                        current_literal.clear();
                    }

                    let mut token = String::new();
                    let mut closed = false;
                    for inner_c in chars.by_ref() {
                        if inner_c == '}' {
                            closed = true;
                            break;
                        }
                        token.push(inner_c);
                    }

                    if !closed {
                        return Err(PortalError::InvalidPickerCmd(format!(
                            "unclosed '{{' in {token}"
                        )));
                    }

                    let resolved_token = self.resolve(&token, false)?;
                    result.push(&resolved_token);
                }
                c => {
                    current_literal.push(c);
                }
            }
        }

        if !current_literal.is_empty() {
            result.push(&current_literal);
        }

        Ok(result)
    }
}

/// Static config validation
pub fn validate_template(template: &str) -> Result<(), ConfigError> {
    let mut chars = template.chars().peekable();

    let mut stack: Vec<(char, u8)> = Vec::new();

    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
            continue;
        }

        if c == '"' {
            if stack.last().map(|&(d, _)| d) == Some('"') {
                stack.pop();
            } else {
                stack.push(('"', 0));
            }
            continue;
        }

        if c == '\'' {
            if stack.last().map(|&(d, _)| d) == Some('\'') {
                stack.pop();
            } else {
                stack.push(('\'', 0));
            }
            continue;
        }

        if c == ':' {
            if let Some((delim, colons)) = stack.last_mut()
                && *delim == '{'
            {
                *colons += 1;
                if *colons > 2 {
                    return Err(ConfigError::TooManyBranchSeparators);
                }
            }
            continue;
        }

        if c == '}' {
            match stack.pop() {
                Some(('{', _)) => {}
                Some(('"', _)) => {
                    return Err(ConfigError::UnclosedQuoteBeforeBrace);
                }
                None => {
                    return Err(ConfigError::StrayCloseBrace);
                }
                Some(_) => unreachable!(),
            }
            continue;
        }

        if c == '{' {
            stack.push(('{', 0));
            let is_conditional = match chars.peek() {
                Some(&'?') | Some(&'#') => {
                    chars.next();
                    true
                }
                _ => false,
            };
            let expected_delimiter = if is_conditional { ':' } else { '}' };
            let mut key = String::new();
            let mut token_len = 0;
            while let Some(&next_c) = chars.peek() {
                if next_c == expected_delimiter || next_c == '}' {
                    break;
                }
                if token_len >= MAX_TOKEN_LEN {
                    return Err(ConfigError::RunawayToken(key));
                }
                key.push(chars.next().unwrap());
                token_len += 1;
            }
            let key = key.trim();
            if key.is_empty() || !VALID_TOKENS.contains(&key) {
                return Err(ConfigError::UnknownToken(key.to_string()));
            }
            if is_conditional && chars.peek() != Some(&':') {
                return Err(ConfigError::MissingConditionalDelimiter(key.to_string()));
            }
        }
    }

    match stack.last() {
        Some(&('{', _)) => Err(ConfigError::UnmatchedOpenBrace),
        Some(&('"', _)) => Err(ConfigError::UnmatchedQuote),
        Some(&('\'', _)) => Err(ConfigError::UnmatchedSingleQuote),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_string() {
        assert!(validate_template("").is_ok());
        assert!(validate_template("Just a normal string").is_ok());
        assert!(validate_template("'Quoted string'").is_ok());
        assert!(validate_template("\"Quoted string\"").is_ok());
        // Escaped braces are ignored for token reolution
        assert!(validate_template("\\{not_a_token\\}").is_ok());
        // Escaped quote
        assert!(validate_template("Escaped \\' quote").is_ok());
        // Escaped double quote
        assert!(validate_template("Escaped \\\" quote").is_ok());
        // Escaped slash
        assert!(validate_template("Escaped \\\\").is_ok());
    }

    #[test]
    fn test_invalid_string() {
        // Unclosed quotes
        assert_eq!(
            validate_template("\"unclosed string"),
            Err(ConfigError::UnmatchedQuote)
        );
        assert_eq!(
            validate_template("'unclosed string"),
            Err(ConfigError::UnmatchedSingleQuote)
        );
        // Unmatched quotes
        assert_eq!(
            validate_template("Stray quote\""),
            Err(ConfigError::UnmatchedQuote)
        );
        assert_eq!(
            validate_template("Stray 'Quote"),
            Err(ConfigError::UnmatchedSingleQuote)
        );
    }

    #[test]
    fn test_valid_token() {
        assert!(validate_template("Hello {APP_ID}!").is_ok());
        assert!(validate_template("{TITLE} {REQ_PATH}").is_ok());
    }

    #[test]
    fn test_invalid_tokens() {
        // Empty token
        assert_eq!(
            validate_template("{}"),
            Err(ConfigError::UnknownToken(String::new()))
        );
        // Unkwown token
        assert_eq!(
            validate_template("{INVALID}"),
            Err(ConfigError::UnknownToken("INVALID".to_string()))
        );
        // Unclosed token
        assert_eq!(
            validate_template("{APP_ID"),
            Err(ConfigError::UnmatchedOpenBrace)
        );
        // Stray } brace
        assert_eq!(
            validate_template("stray } brace"),
            Err(ConfigError::StrayCloseBrace)
        );
        // Runaway token (> MAX_TOKEN_LEN)
        assert!(matches!(
            validate_template("{TOKEN_HAS_NO_ENDING_CURLY_BRACE"),
            Err(ConfigError::RunawayToken(_))
        ));
    }

    #[test]
    fn test_valid_conditionals() {
        // Standard two-branch
        assert!(validate_template("{#MODAL:true:false}").is_ok());
        // Single branch (implicit false)
        assert!(validate_template("{?DIRECTORY:true}").is_ok());
        // Empty branches
        assert!(validate_template("{#MULTIPLE::}").is_ok());
        // Spaces in tokens
        assert!(validate_template("{#WINDOW_ID : yes : no}").is_ok());
        // Token inside a conditional branch
        assert!(validate_template("{#MODAL:{APP_ID}:Guest}").is_ok());
        // Nested conditionals
        assert!(validate_template("{#DIRECTORY:{?MULTIPLE:Many}:Single}").is_ok());
        // Escaping special characters
        assert!(validate_template("{#MODAL:{?DIRECTORY:\\{{TITLE}\\}:{ACCEPT_LABEL}}}").is_ok());
        // Quotes protect conditional colons
        assert!(validate_template("{#MODAL:\"http://example.com\":None}").is_ok());
    }

    #[test]
    fn test_invalid_conditional() {
        // Missing conditional delimiter
        assert_eq!(
            validate_template("{#MODAL}"),
            Err(ConfigError::MissingConditionalDelimiter(
                "MODAL".to_string()
            ))
        );
        // Too Many branch separators
        assert_eq!(
            validate_template("{#MODAL:a:b:c}"),
            Err(ConfigError::TooManyBranchSeparators)
        );
    }
}

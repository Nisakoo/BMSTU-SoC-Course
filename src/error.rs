use std::fmt;

/// Ошибка ассемблера: номер строки (с 1) и сообщение.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl Error {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "строка {}: {}", self.line, self.message)
    }
}

impl std::error::Error for Error {}

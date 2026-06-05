use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Error {
    message: String,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<syn::Error> for Error {
    fn from(value: syn::Error) -> Self {
        Self::new(value.to_string())
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

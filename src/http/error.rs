use crate::http::fields::ToWasiHeaderError;
use std::fmt;

/// The `http` result type.
pub type Result<T> = std::result::Result<T, Error>;

/// The `http` error type.
pub struct Error {
    variant: ErrorVariant,
    context: Vec<String>,
}

pub use http::header::{InvalidHeaderName, InvalidHeaderValue};
pub use http::method::InvalidMethod;
pub use wasip2::http::types::{ErrorCode as WasiHttpErrorCode, HeaderError as WasiHttpHeaderError};

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.context.iter() {
            writeln!(f, "in {c}:")?;
        }
        match &self.variant {
            ErrorVariant::WasiHttp(e) => write!(f, "wasi http error: {e:?}"),
            ErrorVariant::WasiHeader(e) => write!(f, "wasi header error: {e:?}"),
            ErrorVariant::HeaderName(e) => write!(f, "header name error: {e:?}"),
            ErrorVariant::HeaderValue(e) => write!(f, "header value error: {e:?}"),
            ErrorVariant::Method(e) => write!(f, "method error: {e:?}"),
            ErrorVariant::BodyIo(e) => write!(f, "body error: {e:?}"),
            ErrorVariant::Other(e) => write!(f, "{e}"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variant {
            ErrorVariant::WasiHttp(e) => write!(f, "wasi http error: {e}"),
            ErrorVariant::WasiHeader(e) => write!(f, "wasi header error: {e}"),
            ErrorVariant::HeaderName(e) => write!(f, "header name error: {e}"),
            ErrorVariant::HeaderValue(e) => write!(f, "header value error: {e}"),
            ErrorVariant::Method(e) => write!(f, "method error: {e}"),
            ErrorVariant::BodyIo(e) => write!(f, "body error: {e}"),
            ErrorVariant::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn variant(&self) -> &ErrorVariant {
        &self.variant
    }
    pub(crate) fn other(s: impl Into<String>) -> Self {
        ErrorVariant::Other(s.into()).into()
    }
    pub(crate) fn context(self, s: impl Into<String>) -> Self {
        let mut context = self.context;
        context.push(s.into());
        Self {
            variant: self.variant,
            context,
        }
    }
}

impl From<ErrorVariant> for Error {
    fn from(variant: ErrorVariant) -> Error {
        Error {
            variant,
            context: Vec::new(),
        }
    }
}

impl From<WasiHttpErrorCode> for Error {
    fn from(e: WasiHttpErrorCode) -> Error {
        ErrorVariant::WasiHttp(e).into()
    }
}

impl From<ToWasiHeaderError> for Error {
    fn from(error: ToWasiHeaderError) -> Error {
        Error {
            variant: ErrorVariant::WasiHeader(error.error),
            context: vec![error.context],
        }
    }
}

impl From<InvalidHeaderValue> for Error {
    fn from(e: InvalidHeaderValue) -> Error {
        ErrorVariant::HeaderValue(e).into()
    }
}

impl From<InvalidHeaderName> for Error {
    fn from(e: InvalidHeaderName) -> Error {
        ErrorVariant::HeaderName(e).into()
    }
}

impl From<InvalidMethod> for Error {
    fn from(e: InvalidMethod) -> Error {
        ErrorVariant::Method(e).into()
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        ErrorVariant::BodyIo(e).into()
    }
}

#[derive(Debug)]
pub enum ErrorVariant {
    WasiHttp(WasiHttpErrorCode),
    WasiHeader(WasiHttpHeaderError),
    HeaderName(InvalidHeaderName),
    HeaderValue(InvalidHeaderValue),
    Method(InvalidMethod),
    BodyIo(std::io::Error),
    Other(String),
}

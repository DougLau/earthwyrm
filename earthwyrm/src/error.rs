// Copyright (C) 2026  Douglas Lau
//
//! Error module

/// EarthWyrm errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Other error
    #[error("err: {0}")]
    Other(&'static str),

    /// JavaScript error
    #[error("JS {0}")]
    JsValue(String),

    /// Fetch request error
    #[error("Fetch req: {0}")]
    FetchReq(String),

    /// HTTP "Unauthorized 401"
    #[error("Unauthorized 401")]
    HttpUnauthorized(),

    /// HTTP "Forbidden 403"
    #[error("Forbidden 403")]
    HttpForbidden(),

    /// HTTP "Not Found 404"
    #[error("Not Found 404")]
    HttpNotFound(),

    /// HTTP "Conflict 409"
    #[error("Conflict 409")]
    HttpConflict(),

    /// HTTP other error
    #[error("Status code {0}")]
    HttpOther(u16),
}

/// EarthWyrm result
pub type Result<T> = std::result::Result<T, Error>;

impl From<wasm_bindgen::JsValue> for Error {
    fn from(err: wasm_bindgen::JsValue) -> Self {
        Self::JsValue(err.as_string().unwrap_or(String::from("Unknown")))
    }
}

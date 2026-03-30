// Copyright (C) 2026  Douglas Lau
//
//! Error module

/// EarthWyrm errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Web-sys error
    #[error("web-sys: {0}")]
    WebSys(&'static str),

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

    /// Other error
    #[error("err: {0}")]
    Other(&'static str),
}

/// EarthWyrm result
pub type Result<T> = std::result::Result<T, Error>;

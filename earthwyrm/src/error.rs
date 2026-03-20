// Copyright (C) 2026  Minnesota Department of Transportation
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//

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

/// Bulb result
pub type Result<T> = std::result::Result<T, Error>;

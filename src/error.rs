// error.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use mvt;
use postgres;
use r2d2;
use std::net::AddrParseError;
use std::{fmt, io};
use toml;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Pg(postgres::Error),
    R2D2(r2d2::Error),
    Mvt(mvt::Error),
    TileEmpty(),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{}", e.to_string()),
            Error::Pg(e) => write!(f, "{}", e.to_string()),
            Error::R2D2(e) => write!(f, "{}", e.to_string()),
            Error::Mvt(e) => write!(f, "{}", e.to_string()),
            Error::TileEmpty() => write!(f, "Tile empty"),
            Error::Other(s) => write!(f, "Error {}", s),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Pg(e) => Some(e),
            Error::R2D2(e) => Some(e),
            Error::Mvt(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<postgres::Error> for Error {
    fn from(e: postgres::Error) -> Self {
        Error::Pg(e)
    }
}

impl From<r2d2::Error> for Error {
    fn from(e: r2d2::Error) -> Self {
        Error::R2D2(e)
    }
}

impl From<mvt::Error> for Error {
    fn from(e: mvt::Error) -> Self {
        Error::Mvt(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<AddrParseError> for Error {
    fn from(e: AddrParseError) -> Self {
        Error::Other(e.to_string())
    }
}

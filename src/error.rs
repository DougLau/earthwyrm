// error.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use std::net::AddrParseError;
use std::num::{ParseIntError, TryFromIntError};
use std::{fmt, io};

/// Earthwyrm error types
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// Duplicate tag pattern
    DuplicatePattern(String),
    /// Invalid network address error
    InvalidAddress(AddrParseError),
    /// I/O error
    Io(io::Error),
    /// MuON deserializing error
    Muon(muon_rs::Error),
    /// MVT error
    Mvt(mvt::Error),
    /// Parse int error
    ParseInt(ParseIntError),
    /// PostgreSQL error
    Pg(postgres::Error),
    /// Tile empty
    TileEmpty(),
    /// TryFrom conversion error
    TryFromInt(TryFromIntError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DuplicatePattern(v) => write!(f, "Duplicate patterm: {}", v),
            Error::InvalidAddress(e) => e.fmt(f),
            Error::Io(e) => e.fmt(f),
            Error::Muon(e) => e.fmt(f),
            Error::Mvt(e) => e.fmt(f),
            Error::ParseInt(e) => e.fmt(f),
            Error::Pg(e) => e.fmt(f),
            Error::TileEmpty() => write!(f, "Tile empty"),
            Error::TryFromInt(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::DuplicatePattern(_) => None,
            Error::InvalidAddress(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Muon(e) => Some(e),
            Error::Mvt(e) => Some(e),
            Error::ParseInt(e) => Some(e),
            Error::Pg(e) => Some(e),
            Error::TileEmpty() => None,
            Error::TryFromInt(e) => Some(e),
        }
    }
}

impl From<AddrParseError> for Error {
    fn from(e: AddrParseError) -> Self {
        Error::InvalidAddress(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<muon_rs::Error> for Error {
    fn from(e: muon_rs::Error) -> Self {
        Error::Muon(e)
    }
}

impl From<mvt::Error> for Error {
    fn from(e: mvt::Error) -> Self {
        Error::Mvt(e)
    }
}

impl From<ParseIntError> for Error {
    fn from(e: ParseIntError) -> Self {
        Error::ParseInt(e)
    }
}

impl From<postgres::Error> for Error {
    fn from(e: postgres::Error) -> Self {
        Error::Pg(e)
    }
}

impl From<TryFromIntError> for Error {
    fn from(e: TryFromIntError) -> Self {
        Error::TryFromInt(e)
    }
}

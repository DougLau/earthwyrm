use mvt;
use postgres;
use r2d2;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Pg(postgres::Error),
    R2D2(r2d2::Error),
    Mvt(mvt::Error),
    TileEmpty(),
    Other(String),
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

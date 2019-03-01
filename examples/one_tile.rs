// one_tile.rs
//
// Copyright (c) 2019 Minnesota Department of Transportation
//
#[macro_use] extern crate log;
use postgres::{self, Connection, TlsMode};
use earthwyrm::{Error, TileMaker};

fn main() {
    env_logger::Builder::from_default_env()
                        .default_format_timestamp(false)
                        .init();
    write_tile().unwrap();
}

fn write_tile() -> Result<(), Error> {
    if let Some(username) = users::get_current_username() {
        let maker = TileMaker::new("tiles").build()?;
        // build path for unix domain socket
        let mut db_url = "postgres://".to_string();
        db_url.push_str(&username);
        // not worth using percent_encode
        db_url.push_str("@%2Frun%2Fpostgresql/osm");
        let conn = Connection::connect(db_url, TlsMode::None)?;
        maker.write_tile(&conn, 246, 368, 10)?;
    } else {
        error!("User name lookup error");
    }
    Ok(())
}

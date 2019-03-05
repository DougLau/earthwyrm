// earthwyrm.rs
//
// Copyright (c) 2019 Minnesota Department of Transportation
//
#[macro_use]
extern crate log;
use earthwyrm::{Error, TableCfg, TileMaker};
use postgres::{self, Connection};
use r2d2::Pool;
use r2d2_postgres::{PostgresConnectionManager, TlsMode};
use serde_derive::Deserialize;
use std::fs;
use std::net::SocketAddr;
use warp::{self, filters, path, reject::not_found, Filter};

#[derive(Debug, Deserialize)]
struct Config {
    bind_address: String,
    rules_path: String,
    document_root: Option<String>,
    table: Vec<TableCfg>,
}

fn main() {
    env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .init();
    if let Err(e) = do_main() {
        error!("{:?}", e);
    }
}

fn do_main() -> Result<(), Error> {
    let username = users::get_current_username()
        .ok_or(Error::Other("User name lookup error".to_string()))?;
    let config = read_config("./earthwyrm.toml")?;
    let sock_addr: SocketAddr = config.bind_address.parse()?;
    let builder = TileMaker::new("tiles")
        .rules_path(&config.rules_path)
        .tables(config.table);
    let maker = builder.build()?;
    // build path for unix domain socket
    let mut db_url = "postgres://".to_string();
    db_url.push_str(&username);
    // not worth using percent_encode
    db_url.push_str("@%2Frun%2Fpostgresql/earthwyrm");
    let manager = PostgresConnectionManager::new(db_url, TlsMode::None)?;
    let pool = r2d2::Pool::new(manager)?;
    run_server(sock_addr, maker, pool);
    Ok(())
}

fn read_config(fname: &str) -> Result<Config, Error> {
    let config: Config = toml::from_str(&fs::read_to_string(fname)?)?;
    Ok(config)
}

fn run_server(
    sock_addr: SocketAddr,
    maker: TileMaker,
    pool: Pool<PostgresConnectionManager>)
{
    let html = warp::get2()
        .and(path!("map.html"))
        .and(warp::fs::file("map.html"));
    let css = warp::get2()
        .and(path!("map.css"))
        .and(warp::fs::file("map.css"));
    let js = warp::get2()
        .and(path!("map.js"))
        .and(warp::fs::file("map.js"));
    let tile = warp::get2()
        .and(warp::addr::remote())
        .and(path!("tile" / u32 / u32))
        .and(warp::path::tail())
        .and_then(move |host, z, x, tail| {
            debug!("request from {:?}", host);
            let pool = pool.clone();
            if let Some(conn) = pool.try_get() {
                generate_tile(&maker, &conn, z, x, tail)
            } else {
                // FIXME: respond with 503 (service unavailable) status
                Err(not_found())
            }
        });
    let routes = html.or(css).or(js).or(tile);
    warp::serve(routes).run(sock_addr);
}

fn generate_tile(
    maker: &TileMaker,
    conn: &Connection,
    z: u32,
    x: u32,
    tail: filters::path::Tail,
) -> Result<Vec<u8>, warp::reject::Rejection> {
    let mut sp = tail.as_str().splitn(2, '.');
    if let (Some(y), Some("mvt")) = (sp.next(), sp.next()) {
        if let Ok(y) = y.parse::<u32>() {
            return match maker.write_buf(conn, x, y, z) {
                Ok(buf) => Ok(buf),
                // FIXME: respond with 500 (internal server error)
                Err(Error::Pg(_)) => Err(not_found()),
                // FIXME: respond with 500 (internal server error)
                Err(Error::Mvt(_)) => Err(not_found()),
                // FIXME: respond with 503 (service unavaliable)
                Err(Error::R2D2(_)) => Err(not_found()),
                // FIXME: respond with 204 (no content)
                Err(Error::TileEmpty()) => Err(not_found()),
                // FIXME: respond with 400 (bad request) or 404 (not found)
                Err(_) => Err(not_found()),
            };
        }
    }
    Err(not_found())
}

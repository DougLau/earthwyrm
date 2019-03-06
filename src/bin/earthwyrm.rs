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
use serde_derive::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use warp::{self, filters, path, reject::custom, reject::not_found, Filter};
use warp::{Rejection, Reply};
use warp::http::StatusCode;

#[derive(Debug, Deserialize)]
struct Config {
    bind_address: String,
    rules_path: String,
    document_root: Option<String>,
    table: Vec<TableCfg>,
}

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

fn main() {
    env_logger::Builder::from_default_env()
        .default_format_timestamp(false)
        .init();
    let res = do_main();
    if let Err(e) = &res {
        error!("{:?}", e);
        res.unwrap();
    }
}

fn do_main() -> Result<(), Error> {
    let username = users::get_current_username()
        .ok_or(Error::Other("User name lookup error".to_string()))?;
    let config = read_config("/etc/earthwyrm/earthwyrm.toml")?;
    let sock_addr: SocketAddr = config.bind_address.parse()?;
    let document_root = config.document_root;
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
    run_server(document_root, sock_addr, maker, pool);
    Ok(())
}

fn read_config(fname: &str) -> Result<Config, Error> {
    let config: Config = toml::from_str(&fs::read_to_string(fname)?)?;
    Ok(config)
}

fn run_server(
    document_root: Option<String>,
    sock_addr: SocketAddr,
    maker: TileMaker,
    pool: Pool<PostgresConnectionManager>)
{
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
                Err(custom(Error::Other("DB connection failed".to_string())))
            }
        });
    let root = document_root.unwrap_or("/var/lib/earthwyrm".to_string());
    let map = warp::path("map.html")
        .and(warp::fs::file(root.to_string() + "/map.html"));
    let files = warp::path("static").and(warp::fs::dir(root));
    let routes = tile.or(map).or(files).recover(customize_error);
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
                Err(e) => Err(custom(e)),
            };
        }
    }
    Err(not_found())
}

fn customize_error(err: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(ref err) = err.find_cause::<Error>() {
        let code = match err {
            Error::Pg(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Mvt(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::R2D2(_) => StatusCode::SERVICE_UNAVAILABLE,
            Error::TileEmpty() => StatusCode::NO_CONTENT,
            Error::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        let msg = err.to_string();
        warn!("request err: {}", msg);
        let json = warp::reply::json(&ErrorMessage {
            code: code.as_u16(),
            message: msg,
        });
        Ok(warp::reply::with_status(json, code))
    } else {
        Err(err)
    }
}

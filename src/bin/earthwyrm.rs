// earthwyrm.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use earthwyrm::{Error, TileMaker, TomlCfg};
use log::{debug, error, warn};
use postgres::config::Config;
use postgres::{Client, NoTls};
use r2d2_postgres::PostgresConnectionManager;
use serde_derive::Serialize;
use std::net::SocketAddr;
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::reject::{custom, not_found};
use warp::{filters, Filter, Rejection, Reply};

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

fn main() {
    env_logger::builder().format_timestamp(None).init();
    let res = do_main("/etc/earthwyrm/earthwyrm.toml");
    if let Err(e) = &res {
        error!("{:?}", e);
        res.unwrap();
    }
}

fn do_main(file: &str) -> Result<(), Error> {
    let config = TomlCfg::from_file(file).expect(file);
    let sock_addr: SocketAddr = config.bind_address().parse()?;
    let document_root = config.document_root().to_string();
    let makers = config.into_tile_makers()?;
    let username = whoami::username();
    // Format path for unix domain socket -- not worth using percent_encode
    let uds = format!("postgres://{:}@%2Frun%2Fpostgresql/earthwyrm", username);
    let config = uds.parse()?;
    run_server(document_root, sock_addr, makers, config);
    Ok(())
}

fn run_server(
    document_root: String,
    sock_addr: SocketAddr,
    makers: Vec<TileMaker>,
    config: Config,
) {
    let tiles = tile_route(makers, config);
    let map = warp::path("map.html")
        .and(warp::fs::file(document_root.to_string() + "/map.html"));
    let files = warp::path("static").and(warp::fs::dir(document_root));
    let routes = tiles.or(map).or(files).recover(customize_error);
    warp::serve(routes).run(sock_addr);
}

fn tile_route(
    makers: Vec<TileMaker>,
    config: Config,
) -> BoxedFilter<(impl Reply,)> {
    let manager = PostgresConnectionManager::new(config, NoTls);
    let pool = r2d2::Pool::new(manager).unwrap();
    warp::get2()
        .and(warp::addr::remote())
        .and(warp::path::param::<String>())
        .and(warp::path::param::<u32>())
        .and(warp::path::param::<u32>())
        .and(warp::path::tail())
        .and_then(move |host, base_name, z, x, tail| {
            debug!("request from {:?}", host);
            if let Some(mut conn) = pool.try_get() {
                generate_tile(&makers[..], &mut conn, base_name, z, x, tail)
            } else {
                Err(custom(Error::Other("DB connection failed".to_string())))
            }
        })
        .boxed()
}

fn generate_tile(
    makers: &[TileMaker],
    conn: &mut Client,
    base_name: String,
    z: u32,
    x: u32,
    tail: filters::path::Tail,
) -> Result<Vec<u8>, Rejection> {
    for maker in makers {
        if base_name == maker.base_name() {
            let mut sp = tail.as_str().splitn(2, '.');
            if let (Some(y), Some("mvt")) = (sp.next(), sp.next()) {
                if let Ok(y) = y.parse::<u32>() {
                    return match maker.write_buf(conn, x, y, z) {
                        Ok(buf) => Ok(buf),
                        Err(e) => Err(custom(e)),
                    };
                }
            }
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

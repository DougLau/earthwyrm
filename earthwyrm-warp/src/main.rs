// earthwyrm.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use earthwyrm::{Error, LayerGroup, TileId, WyrmCfg};
use log::{debug, error, warn};
use postgres::config::Config;
use postgres::{Client, NoTls};
use r2d2_postgres::PostgresConnectionManager;
use serde_derive::Serialize;
use std::fs;
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
    let res = do_main("/etc/earthwyrm/earthwyrm.muon");
    if let Err(e) = &res {
        error!("{:?}", e);
        res.unwrap();
    }
}

fn do_main(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let wyrm: WyrmCfg = muon_rs::from_str(&fs::read_to_string(file)?)?;
    let sock_addr: SocketAddr = wyrm.bind_address().parse()?;
    let document_root = wyrm.document_root().to_string();
    let groups = wyrm.into_layer_groups()?;
    let username = whoami::username();
    // Format path for unix domain socket -- not worth using percent_encode
    let uds = format!("postgres://{:}@%2Frun%2Fpostgresql/earthwyrm", username);
    let config = uds.parse()?;
    run_server(document_root, sock_addr, groups, config);
    Ok(())
}

fn run_server(
    document_root: String,
    sock_addr: SocketAddr,
    groups: Vec<LayerGroup>,
    config: Config,
) {
    let tiles = tile_route(groups, config);
    let map = warp::path("index.html")
        .and(warp::fs::file(document_root.to_string() + "/index.html"));
    let files = warp::path("static")
        .and(warp::fs::dir(document_root.to_string() + "/static"));
    let routes = tiles.or(map).or(files).recover(customize_error);
    warp::serve(routes).run(sock_addr);
}

fn tile_route(
    groups: Vec<LayerGroup>,
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
        .and_then(move |host, name, z, x, tail| {
            debug!("request from {:?}", host);
            match pool.get() {
                Ok(mut conn) => {
                    let tid = parse_tile_id(z, x, tail)?;
                    generate_tile(&groups[..], &mut conn, name, tid)
                }
                Err(e) => Err(custom(e)),
            }
        })
        .boxed()
}

fn parse_tile_id(
    z: u32,
    x: u32,
    tail: filters::path::Tail,
) -> Result<TileId, Rejection> {
    let mut sp = tail.as_str().splitn(2, '.');
    if let (Some(y), Some("mvt")) = (sp.next(), sp.next()) {
        if let Ok(y) = y.parse::<u32>() {
            if let Ok(tid) = TileId::new(x, y, z) {
                return Ok(tid);
            }
        }
    }
    Err(not_found())
}

fn generate_tile(
    groups: &[LayerGroup],
    conn: &mut Client,
    name: String,
    tid: TileId,
) -> Result<Vec<u8>, Rejection> {
    for group in groups {
        if name == group.name() {
            let mut out = vec![];
            return match group.write_tile(&mut out, conn, tid) {
                Ok(()) => Ok(out),
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
            Error::TileEmpty() => StatusCode::NO_CONTENT,
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

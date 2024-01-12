// main.rs
//
// Copyright (c) 2021-2024  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use anyhow::{anyhow, Context, Result};
use argh::FromArgs;
use axum::{
    extract::{Path as AxumPath, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use earthwyrm::{TileId, Wyrm, WyrmCfg};
use mvt::{WebMercatorPos, Wgs84Pos};
use pointy::BBox;
use serde::Deserialize;
use std::fs::{DirEntry, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Get path to the newest OSM file
fn osm_newest() -> Result<PathBuf> {
    let path = Path::new("osm");
    path.read_dir()
        .with_context(|| format!("reading directory: {path:?}"))?
        .filter_map(Result::ok)
        .filter(is_pbf_file)
        .max_by_key(|de| de.metadata().unwrap().modified().unwrap())
        .map(|de| path.join(de.file_name()))
        .ok_or_else(|| anyhow!("no OSM file found"))
}

/// Check if a directory entry is a PBF file
fn is_pbf_file(de: &DirEntry) -> bool {
    match de.file_type() {
        Ok(ft) if ft.is_file() => {
            let name = de.file_name();
            let path: &Path = name.as_ref();
            path.extension().unwrap_or_default() == "pbf"
        }
        _ => false,
    }
}

/// Command-line arguments
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    #[argh(subcommand)]
    cmd: Command,
}

/// Sub-commands
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Command {
    /// Initialize earthwyrm configuration
    Init(InitCommand),

    /// Dig loam layers from OSM file
    Dig(DigCommand),

    /// Query a map layer
    Query(QueryCommand),

    /// Serve tiles with http
    Serve(ServeCommand),
}

/// Initialize earthwyrm configuration
#[derive(Clone, Copy, FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "init")]
struct InitCommand {}

/// Dig loam layers from OSM file
#[derive(Clone, Copy, FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "dig")]
struct DigCommand {}

/// Query a map layer
#[derive(Clone, Copy, FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "query")]
struct QueryCommand {
    #[argh(positional)]
    lat: f64,

    #[argh(positional)]
    lon: f64,
}

/// Serve tiles using http
#[derive(Clone, Copy, FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "serve")]
struct ServeCommand {
    /// include leaflet map for testing
    #[argh(switch, short = 'l')]
    leaflet: bool,
}

impl InitCommand {
    /// Initialize earthwyrm configuration
    fn init(self) -> Result<()> {
        let osm_path = Path::new("osm");
        std::fs::create_dir_all(osm_path)?;
        let loam_path = Path::new("loam");
        std::fs::create_dir_all(loam_path)?;
        write_file(
            Path::new("earthwyrm.muon"),
            include_bytes!("../res/earthwyrm.muon"),
        )?;
        write_file(
            Path::new("earthwyrm.service"),
            include_bytes!("../res/earthwyrm.service"),
        )?;
        Ok(())
    }
}

/// Write a file to specified path
fn write_file<P>(path: P, contents: &[u8]) -> Result<()>
where
    P: AsRef<Path> + core::fmt::Debug,
{
    println!("Writing file: {path:?}");
    let mut file = File::options().create_new(true).write(true).open(&path)?;
    Ok(file.write_all(contents)?)
}

impl DigCommand {
    /// Dig loam layers from OSM file
    fn dig(self, cfg: WyrmCfg) -> Result<()> {
        let osm = osm_newest()?;
        Ok(cfg.extract_osm(osm)?)
    }
}

impl QueryCommand {
    /// Query a lat/lon position
    fn query(&self, cfg: WyrmCfg) -> Result<()> {
        let wyrm = Wyrm::try_from(&cfg)?;
        let pos = Wgs84Pos::new(self.lat, self.lon);
        let pos = WebMercatorPos::from(pos);
        let bbox = BBox::new([pos]);
        wyrm.query_features(bbox)?;
        Ok(())
    }
}

impl ServeCommand {
    /// Serve tiles using http
    fn serve(&self, cfg: WyrmCfg) -> Result<()> {
        let wyrm = Arc::new(Wyrm::try_from(&cfg)?);
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut app = Router::new();
            if self.leaflet {
                app = app.merge(index_html()).merge(map_css()).merge(map_js());
            }
            app = app.merge(tile_mvt(wyrm));
            let listener = TcpListener::bind(cfg.bind_address).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });
        Ok(())
    }
}

/// Router for `indexl.html`
fn index_html() -> Router {
    async fn handler() -> impl IntoResponse {
        (
            [(header::CONTENT_TYPE, "text/html")],
            include_str!("../res/index.html"),
        )
    }
    Router::new()
        .route("/", get(handler))
        .route("/index.html", get(handler))
}

/// Router for `map.css`
fn map_css() -> Router {
    async fn handler() -> impl IntoResponse {
        ([(header::CONTENT_TYPE, "text/css")], include_str!("../res/map.css"))
    }
    Router::new().route("/map.css", get(handler))
}

/// Router for `map.js`
fn map_js() -> Router {
    async fn handler() -> impl IntoResponse {
        (
            [(header::CONTENT_TYPE, "text/javascript")],
            include_str!("../res/map.js"),
        )
    }
    Router::new().route("/map.js", get(handler))
}

/// Get a tile `.mvt` as response
fn tile_mvt(wyrm: Arc<Wyrm>) -> Router {
    async fn handler(
        AxumPath(params): AxumPath<TileParams>,
        State(state): State<Arc<Wyrm>>,
    ) -> impl IntoResponse {
        log::debug!(
            "req: {}/{}/{}/{}",
            &params.group,
            params.z,
            params.x,
            params.tail
        );
        let Ok(tid) = TileId::try_from(&params) else {
            return (StatusCode::NOT_FOUND, "Not Found".into_response());
        };
        let mut out = vec![];
        match state.fetch_tile(&mut out, &params.group, tid) {
            Ok(()) => (StatusCode::OK, out.into_response()),
            Err(earthwyrm::Error::TileEmpty()) => {
                (StatusCode::NOT_FOUND, "Not Found".into_response())
            }
            Err(err) => {
                log::warn!("fetch_tile: {err:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Error".into_response(),
                )
            }
        }
    }
    Router::new()
        .route("/:group/:z/:x/:tail", get(handler))
        .with_state(wyrm)
}

/// Tile route parameters
#[derive(Deserialize)]
struct TileParams {
    group: String,
    z: u32,
    x: u32,
    tail: String,
}

impl TryFrom<&TileParams> for TileId {
    type Error = mvt::Error;

    fn try_from(params: &TileParams) -> Result<Self, Self::Error> {
        if let Some(y) = params.tail.strip_suffix(".mvt") {
            if let Ok(y) = y.parse::<u32>() {
                return TileId::new(params.x, y, params.z);
            }
        }
        Err(mvt::Error::InvalidTid())
    }
}

impl Args {
    /// Run selected command
    fn run(self) -> Result<()> {
        match &self.cmd {
            Command::Init(cmd) => cmd.init(),
            Command::Dig(cmd) => cmd.dig(WyrmCfg::load()?),
            Command::Query(cmd) => cmd.query(WyrmCfg::load()?),
            Command::Serve(cmd) => cmd.serve(WyrmCfg::load()?),
        }
    }
}

/// Main entry point
fn main() -> Result<()> {
    env_logger::builder().format_timestamp(None).init();
    let args: Args = argh::from_env();
    args.run()?;
    Ok(())
}

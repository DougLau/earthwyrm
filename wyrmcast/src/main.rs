// main.rs
//
// Copyright (c) 2021-2026  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use anyhow::{Context, Result, anyhow};
use argh::FromArgs;
use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use pointy::BBox;
use serde::Deserialize;
use squarepeg::{Peg, WebMercatorPos, Wgs84Pos};
use std::fmt;
use std::fs::{DirEntry, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;
use wyrmcast::{CasterCfg, CasterDef};

/// Path to configuration file
const CFG_PATH: &str = "wyrmcast.muon";

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
    /// Initialize wyrmcast configuration
    Init(InitCommand),

    /// Dig loam layers from OSM file
    Dig(DigCommand),

    /// Query a map layer
    Query(QueryCommand),

    /// Serve tiles with http
    Serve(ServeCommand),
}

/// Initialize wyrmcast configuration
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
    /// Initialize wyrmcast configuration
    fn init(self) -> Result<()> {
        let home_path = Path::new(".");
        // Set home directory permissions: `drwxr-x---`
        std::fs::set_permissions(home_path, PermissionsExt::from_mode(0o750))?;
        let osm_path = Path::new("osm");
        std::fs::create_dir_all(osm_path)?;
        let loam_path = Path::new("loam");
        std::fs::create_dir_all(loam_path)?;
        // Set loam directory permissions: `drwxrwxr-x`
        std::fs::set_permissions(loam_path, PermissionsExt::from_mode(0o775))?;
        write_file(
            Path::new(CFG_PATH),
            include_bytes!("../res/wyrmcast.muon"),
        )?;
        write_file(
            Path::new("wyrmcast.service"),
            include_bytes!("../res/wyrmcast.service"),
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
    fn dig(self, cfg: CasterCfg) -> Result<()> {
        let osm = osm_newest()?;
        cfg.extract_osm(osm)
    }
}

impl QueryCommand {
    /// Query a lat/lon position
    fn query(&self, cfg: CasterCfg) -> Result<()> {
        let caster = CasterDef::try_from(&cfg)?;
        let pos = Wgs84Pos::new(self.lat, self.lon);
        let pos = WebMercatorPos::from(pos);
        let bbox = BBox::new([pos]);
        caster.query_features(bbox)?;
        Ok(())
    }
}

impl ServeCommand {
    /// Serve tiles using http
    fn serve(&self, cfg: CasterCfg) -> Result<()> {
        let caster = Arc::new(CasterDef::try_from(&cfg)?);
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut app = Router::new();
            if self.leaflet {
                app = app.merge(index_html()).merge(map_css()).merge(map_js());
            }
            app = app.merge(tile_routes(caster));
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

/// Router for `.wyrm` or `.mvt` tiles
fn tile_routes(caster: Arc<CasterDef>) -> Router {
    async fn handler(
        AxumPath(params): AxumPath<TileParams>,
        State(caster): State<Arc<CasterDef>>,
    ) -> (StatusCode, Response<Body>) {
        log::debug!("req: {params:?}");
        match (params.peg(), params.ext()) {
            (Some(peg), Some("mvt")) => tile_mvt(&caster, &params.group, peg),
            (Some(peg), Some("wyrm")) => tile_wyrm(&caster, &params.group, peg),
            _ => (StatusCode::NOT_FOUND, "Not Found".into_response()),
        }
    }
    Router::new()
        .route("/{group}/{z}/{x}/{tail}", get(handler))
        .with_state(caster)
}

/// Get a tile `.mvt` as response
fn tile_mvt(
    caster: &CasterDef,
    group: &str,
    peg: Peg,
) -> (StatusCode, Response<Body>) {
    let mut out = vec![];
    match caster.fetch_mvt(&mut out, group, peg) {
        Ok(true) => (StatusCode::OK, out.into_response()),
        Ok(false) => (StatusCode::NOT_FOUND, "Not Found".into_response()),
        Err(err) => {
            log::warn!("tile_mvt: {err:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Error".into_response(),
            )
        }
    }
}

/// Get a tile `.wyrm` as response
fn tile_wyrm(
    _caster: &CasterDef,
    _group: &str,
    _peg: Peg,
) -> (StatusCode, Response<Body>) {
    todo!()
}

/// Tile route parameters
#[derive(Deserialize)]
struct TileParams {
    group: String,
    z: u32,
    x: u32,
    tail: String,
}

impl fmt::Debug for TileParams {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}/{}/{}", &self.group, self.z, self.x, self.tail)
    }
}

impl TileParams {
    /// Parse Y parameter
    fn y(&self) -> Option<u32> {
        self.tail
            .split_once('.')
            .and_then(|(y, _ext)| y.parse::<u32>().ok())
    }

    /// Get `Peg` (tile ID)
    fn peg(&self) -> Option<Peg> {
        self.y().and_then(|y| Peg::new(self.x, y, self.z))
    }

    /// Get file extension
    fn ext(&self) -> Option<&str> {
        self.tail.split_once('.').map(|(_y, ext)| ext)
    }
}

impl Args {
    /// Run selected command
    fn run(self) -> Result<()> {
        match &self.cmd {
            Command::Init(cmd) => cmd.init(),
            Command::Dig(cmd) => cmd.dig(CasterCfg::load(CFG_PATH)?),
            Command::Query(cmd) => cmd.query(CasterCfg::load(CFG_PATH)?),
            Command::Serve(cmd) => cmd.serve(CasterCfg::load(CFG_PATH)?),
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

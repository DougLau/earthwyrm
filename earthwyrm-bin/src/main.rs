// main.rs
//
// Copyright (c) 2021-2023  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use anyhow::{anyhow, Context, Result};
use argh::FromArgs;
use axum::{
    http::header,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use earthwyrm::{Wyrm, WyrmCfg};
use pointy::BBox;
use std::fs::{DirEntry, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::net::TcpListener;

/// Get path to the newest OSM file
fn osm_newest(cfg: &WyrmCfg) -> Result<PathBuf> {
    let path = Path::new(cfg.base_dir()).join("osm");
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
    /// base directory
    #[argh(option, short = 'b')]
    base: Option<PathBuf>,

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
    lat: f32,

    #[argh(positional)]
    lon: f32,
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
    fn init<P>(self, base: Option<P>) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let base = WyrmCfg::base_path(base);
        let osm_path = Path::new(&base).join("osm");
        std::fs::create_dir_all(osm_path)?;
        let loam_path = Path::new(&base).join("loam");
        std::fs::create_dir_all(loam_path)?;
        write_file(
            Path::new(&base).join("earthwyrm.muon"),
            include_bytes!("../res/earthwyrm.muon"),
        )?;
        write_file(
            Path::new(&base).join("earthwyrm.service"),
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
        let osm = osm_newest(&cfg)?;
        Ok(cfg.extract_osm(osm)?)
    }
}

impl QueryCommand {
    /// Query a lat/lon position
    fn query(&self, cfg: WyrmCfg) -> Result<()> {
        let wyrm = Wyrm::try_from(&cfg)?;
        let bbox = BBox::new([(self.lon, self.lat)]);
        wyrm.query_features(bbox)?;
        Ok(())
    }
}

impl ServeCommand {
    /// Serve tiles using http
    fn serve(&self, cfg: WyrmCfg) -> Result<()> {
        let wyrm = Wyrm::try_from(&cfg)?;
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut app = Router::new();
            if self.leaflet {
                app = app
                    .route("/index.html", get(index_html))
                    .route("/map.css", get(map_css))
                    .route("/map.js", get(map_js));
            }
            let listener = TcpListener::bind(cfg.bind_address).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });
        Ok(())
    }
}

/// Get `indexl.html` as response
async fn index_html() -> impl IntoResponse {
    Html(include_str!("../res/index.html"))
}

/// Get `map.css` as response
async fn map_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../res/map.css"))
}

/// Get `map.js` as response
async fn map_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../res/map.js"))
}

impl Args {
    /// Read the configuration file into a string
    fn read_config(&self) -> Result<WyrmCfg> {
        let base = WyrmCfg::base_path(self.base.as_ref());
        WyrmCfg::from_dir(&base)
            .with_context(|| format!("config {:?}", &self.base))
    }

    /// Run selected command
    fn run(self) -> Result<()> {
        match &self.cmd {
            Command::Init(cmd) => cmd.init(self.base.as_ref()),
            Command::Dig(cmd) => cmd.dig(self.read_config()?),
            Command::Query(cmd) => cmd.query(self.read_config()?),
            Command::Serve(cmd) => cmd.serve(self.read_config()?),
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

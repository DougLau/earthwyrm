// main.rs
//
// Copyright (c) 2021-2023  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use anyhow::{anyhow, bail, Context, Result};
use argh::FromArgs;
use axum::{
    http::header,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use earthwyrm::{Wyrm, WyrmCfg};
use pointy::BBox;
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Get path to the OSM file
fn osm_path<P>(base: P) -> Result<PathBuf>
where
    P: AsRef<Path>,
{
    let path = Path::new(base.as_ref().as_os_str()).join("osm");
    let mut paths = path
        .read_dir()
        .with_context(|| format!("reading directory: {path:?}"))?
        .filter_map(|f| f.ok())
        .filter_map(|f| match f.file_type() {
            Ok(ft) if ft.is_file() => {
                let path = path.join(f.file_name());
                if path.extension().unwrap_or_default() == "pbf" {
                    Some(path)
                } else {
                    None
                }
            }
            _ => None,
        });
    let osm = paths.next();
    if paths.next().is_some() {
        bail!("multiple OSM files found: {path:?}");
    }
    osm.ok_or_else(|| anyhow!("no OSM file found: {path:?}"))
}

/// Command-line arguments
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// base directory
    #[argh(option, short = 'b')]
    base: Option<OsString>,

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
        let base =
            base.ok_or_else(|| anyhow!("no base directory specified"))?;
        let osm_path = Path::new(base.as_ref().as_os_str()).join("osm");
        std::fs::create_dir_all(osm_path)?;
        let loam_path = Path::new(base.as_ref().as_os_str()).join("loam");
        std::fs::create_dir_all(loam_path)?;
        write_file(
            Path::new(base.as_ref().as_os_str()).join("earthwyrm.muon"),
            include_bytes!("../res/earthwyrm.muon"),
        )?;
        write_file(
            Path::new(base.as_ref().as_os_str()).join("earthwyrm.service"),
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
        let osm = osm_path(cfg.base_dir())?;
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
        let sock_addr = cfg.bind_address.parse()?;
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut app = Router::new();
            if self.leaflet {
                app = app
                    .route("/index.html", get(index_html))
                    .route("/map.css", get(map_css))
                    .route("/map.js", get(map_js));
            }
            axum::Server::bind(&sock_addr)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });
        Ok(())
    }
}

async fn index_html() -> impl IntoResponse {
    Html(include_str!("../res/index.html"))
}

async fn map_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../res/map.css"))
}

async fn map_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../res/map.js"))
}

impl Args {
    /// Read the configuration file into a string
    fn read_config(&self) -> Result<WyrmCfg> {
        WyrmCfg::from_dir(self.base.as_ref())
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

fn main() -> Result<()> {
    env_logger::builder().format_timestamp(None).init();
    let args: Args = argh::from_env();
    args.run()?;
    Ok(())
}

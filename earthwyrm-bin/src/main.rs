// main.rs
//
// Copyright (c) 2021-2022  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use anyhow::{anyhow, bail, Context, Result};
use argh::FromArgs;
use earthwyrm::WyrmCfg;
use pointy::BBox;
use rosewood::{Geometry, Polygon, RTree};
use std::ffi::OsString;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Default path for configuration file
const CONFIG_PATH: &str = "/etc/earthwyrm/earthwyrm.muon";

/// Get path to the OSM file
fn osm_path<P>(dir: P) -> Result<PathBuf>
where
    P: AsRef<Path>,
{
    let path = Path::new(dir.as_ref().as_os_str()).join("osm");
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
    /// configuration file (MuON)
    #[argh(option, short = 'c', default = "OsString::from(CONFIG_PATH)")]
    config: OsString,

    #[argh(subcommand)]
    cmd: Command,
}

/// Sub-commands
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Command {
    /// Dig loam layers from OSM file
    Dig(DigCommand),

    /// Query a map layer
    Query(QueryCommand),
}

/// Dig loam layers from OSM file
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "dig")]
struct DigCommand {}

/// Query a map layer
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "query")]
struct QueryCommand {
    #[argh(positional)]
    loam: OsString,

    #[argh(positional)]
    lat: f32,

    #[argh(positional)]
    lon: f32,
}

impl QueryCommand {
    /// Query a map layer
    fn query_layer(&self) -> Result<()> {
        let rtree = RTree::<f32, Polygon<f32, String>>::new(&self.loam)?;
        let bbox = BBox::new([(-self.lon, self.lat)]);
        for poly in rtree.query(bbox) {
            let poly = poly?;
            println!("found: {}", poly.data());
        }
        Ok(())
    }
}

impl DigCommand {
    /// Dig loam layers from OSM file
    fn dig(self, cfg: String) -> Result<()> {
        let cfg: WyrmCfg =
            muon_rs::from_str(&cfg).context("deserializing configuration")?;
        let osm = osm_path(&cfg.base_dir)?;
        let loam_dir = cfg.base_dir.join("loam");
        Ok(cfg.extract_osm(osm, loam_dir)?)
    }
}

impl Args {
    /// Read the configuration file into a string
    fn read_config(&self) -> Result<String> {
        let path = &self.config;
        read_to_string(path)
            .with_context(|| format!("reading config: {path:?}"))
    }

    fn run(self) -> Result<()> {
        let cfg = self.read_config()?;
        match self.cmd {
            Command::Dig(cmd) => cmd.dig(cfg),
            Command::Query(cmd) => cmd.query_layer(),
        }
    }
}

fn main() -> Result<()> {
    env_logger::builder().format_timestamp(None).init();
    let args: Args = argh::from_env();
    args.run()?;
    Ok(())
}

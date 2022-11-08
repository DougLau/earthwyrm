// main.rs
//
// Copyright (c) 2021-2022  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use anyhow::{anyhow, bail, Context, Result};
use argh::FromArgs;
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

impl InitCommand {
    /// Initialize earthwyrm configuration
    fn init<P>(self, base: Option<P>) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let base =
            base.ok_or_else(|| anyhow!("no base directory specified"))?;
        let static_path = Path::new(base.as_ref().as_os_str()).join("static");
        std::fs::create_dir_all(&static_path)?;
        let osm_path = Path::new(base.as_ref().as_os_str()).join("osm");
        std::fs::create_dir_all(&osm_path)?;
        let loam_path = Path::new(base.as_ref().as_os_str()).join("loam");
        std::fs::create_dir_all(&loam_path)?;
        write_file(
            Path::new(base.as_ref().as_os_str()).join("earthwyrm.muon"),
            include_bytes!("../static/earthwyrm.muon"),
        )?;
        write_file(
            Path::new(base.as_ref().as_os_str()).join("earthwyrm.service"),
            include_bytes!("../static/earthwyrm.service"),
        )?;
        write_file(
            Path::new(&static_path).join("index.html"),
            include_bytes!("../static/index.html"),
        )?;
        write_file(
            Path::new(&static_path).join("map.js"),
            include_bytes!("../static/map.js"),
        )?;
        write_file(
            Path::new(&static_path).join("map.css"),
            include_bytes!("../static/map.css"),
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
        let bbox = BBox::new([(-self.lon, self.lat)]);
        wyrm.query_features(bbox)?;
        Ok(())
    }
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
        }
    }
}

fn main() -> Result<()> {
    env_logger::builder().format_timestamp(None).init();
    let args: Args = argh::from_env();
    args.run()?;
    Ok(())
}

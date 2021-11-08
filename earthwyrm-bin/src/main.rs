// main.rs
//
// Copyright (c) 2021  Minnesota Department of Transportation
//
#![forbid(unsafe_code)]

use argh::FromArgs;
use earthwyrm::{make_layer, Error, WyrmCfg};
use pointy::BBox;
use rosewood::{Geometry, Polygon, RTree};
use std::ffi::OsString;

const LOAM: &str = &"cities.loam";

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
    /// Make a layer
    Make(MakeCommand),

    /// Query the layer
    Query(QueryCommand),
}

/// Make a layer
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "make")]
struct MakeCommand {
    #[argh(positional)]
    config: OsString,

    #[argh(positional)]
    osm: OsString,
}

/// Query the layer
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "query")]
struct QueryCommand {
    #[argh(positional)]
    lat: f32,

    #[argh(positional)]
    lon: f32,
}

/// Query a map layer
fn query_layer(lat: f32, lon: f32) -> Result<(), Error> {
    let rtree = RTree::<f32, Polygon<f32, String>>::new(LOAM)?;
    let bbox = BBox::new([(-lon, lat)]);
    for poly in rtree.query(bbox) {
        let poly = poly?;
        println!("found: {}", poly.data());
    }
    Ok(())
}

impl MakeCommand {
    fn make(self) -> Result<(), Error> {
        let cfg = std::fs::read_to_string(&self.config)?;
        let cfg: WyrmCfg = muon_rs::from_str(&cfg)?;
        Ok(make_layer(cfg.osm)?)
    }
}

impl Args {
    fn run(self) -> Result<(), Error> {
        match self.cmd {
            Command::Make(cmd) => cmd.make(),
            Command::Query(cmd) => query_layer(cmd.lat, cmd.lon),
        }
    }
}

fn main() -> Result<(), Error> {
    env_logger::builder().format_timestamp(None).init();
    let args: Args = argh::from_env();
    args.run()?;
    Ok(())
}

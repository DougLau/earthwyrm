use argh::FromArgs;
use pointy::BBox;
use rosewood::{Geometry, Polygon, RTree};
use std::error::Error;
use std::ffi::OsString;
use earthwyrm::make_layer;

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
fn query_layer(lat: f32, lon: f32) -> Result<(), Box<dyn Error>> {
    let rtree = RTree::<f32, Polygon<f32, String>>::new(LOAM)?;
    let bbox = BBox::new([(-lon, lat)]);
    for poly in rtree.query(bbox) {
        let poly = poly?;
        println!("found: {}", poly.data());
    }
    Ok(())
}

impl Args {
    fn run(self) -> Result<(), Box<dyn Error>> {
        match self.cmd {
            Command::Make(cmd) => Ok(make_layer(cmd.osm)?),
            Command::Query(cmd) => query_layer(cmd.lat, cmd.lon),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder().format_timestamp(None).init();
    let args: Args = argh::from_env();
    args.run()?;
    Ok(())
}

use argh::FromArgs;
use earthwyrm::Error;
use pointy::BBox;
use rosewood::{Geometry, Polygon, RTree};

/// Query arguments
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    #[argh(positional)]
    loam: String,
    #[argh(positional)]
    lat: f32,
    #[argh(positional)]
    lon: f32,
}

impl Args {
    fn run(self) -> Result<(), Error> {
        let rtree = RTree::<f32, Polygon<f32, String>>::new(&self.loam)?;
        let bbox = BBox::new([(-self.lon, self.lat)]);
        for poly in rtree.query(bbox) {
            let poly = poly?;
            println!("found: {}", poly.data());
        }
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    env_logger::builder().format_timestamp(None).init();
    let args: Args = argh::from_env();
    args.run()
}

// one_tile.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use squarepeg::Peg;
use std::env;
use std::fs::File;
use wyrmcast::config::WyrmCfg;
use wyrmcast::error::Error;
use wyrmcast::tile::Wyrm;

const MUON: &str = &r#"
bind_address:
tile_extent: 256
layer_group: tile
  osm: true
  layer: city
    geom_type: polygon
    zoom: 1+
    tags: ?name ?population boundary=administrative admin_level=8
"#;

fn write_tile(
    x: u32,
    y: u32,
    z: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let wyrm_cfg: WyrmCfg = muon_rs::from_str(MUON)?;
    let wyrm = Wyrm::try_from(&wyrm_cfg)?;
    let mut file = File::create("./one_tile.mvt")?;
    let peg = Peg::new(x, y, z).ok_or(Error::InvalidZoomLevel(z))?;
    wyrm.fetch_tile(&mut file, "tile", peg)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();
    args.next().unwrap();
    let x = args.next().expect("missing x").parse()?;
    let y = args.next().expect("missing y").parse()?;
    let z = args.next().expect("missing z").parse()?;
    write_tile(x, y, z)?;
    Ok(())
}

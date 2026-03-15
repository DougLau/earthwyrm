// fetch_mvt.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use anyhow::{Context, Result, anyhow};
use squarepeg::Peg;
use std::env;
use std::fs::File;
use wyrmcast::{CasterCfg, CasterDef};

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

fn write_tile(x: u32, y: u32, z: u32) -> Result<()> {
    let cfg: CasterCfg = muon_rs::from_str(MUON)?;
    let caster = CasterDef::try_from(&cfg)?;
    let mut file = File::create("./tile.mvt").context("creating ./tile.mvt")?;
    let peg = Peg::new(x, y, z).ok_or(anyhow!("Invalid zoom level {z}"))?;
    caster.fetch_mvt(&mut file, "tile", peg)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = env::args();
    args.next().unwrap();
    let x = args.next().unwrap_or(String::from("3946")).parse()?;
    let y = args.next().unwrap_or(String::from("5895")).parse()?;
    let z = args.next().unwrap_or(String::from("14")).parse()?;
    write_tile(x, y, z)?;
    Ok(())
}

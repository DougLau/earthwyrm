// make_mvt.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use anyhow::{Context, Result, anyhow};
use squarepeg::Peg;
use std::env;
use std::fs::File;
use wyrmcast::{CasterCfg, CasterDef};

/// Path to configuration file
const CFG_PATH: &str = "wyrmcast.muon";

fn write_tile(z: u32, x: u32, y: u32) -> Result<()> {
    let cfg = CasterCfg::load(CFG_PATH)?;
    let caster = CasterDef::try_from(&cfg)?;
    let mut file = File::create("./tile.mvt").context("creating ./tile.mvt")?;
    let peg = Peg::new(z, x, y).ok_or(anyhow!("Invalid zoom level {z}"))?;
    caster.fetch_mvt(&mut file, "tile", peg)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = env::args();
    args.next().unwrap();
    let z = args.next().unwrap_or(String::from("14")).parse()?;
    let x = args.next().unwrap_or(String::from("3946")).parse()?;
    let y = args.next().unwrap_or(String::from("5895")).parse()?;
    write_tile(z, x, y)?;
    Ok(())
}

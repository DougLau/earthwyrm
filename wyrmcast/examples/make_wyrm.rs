// make_wyrm.rs
//
// Copyright (c) 2026  Minnesota Department of Transportation
//
use anyhow::{Result, anyhow};
use squarepeg::Peg;
use std::env;
use wyrmcast::{CasterCfg, CasterDef};

/// Path to configuration file
const CFG_PATH: &str = "wyrmcast.muon";

fn write_tile(x: u32, y: u32, z: u32) -> Result<()> {
    let cfg = CasterCfg::load(CFG_PATH)?;
    let caster = CasterDef::try_from(&cfg)?;
    let peg = Peg::new(x, y, z).ok_or(anyhow!("Invalid zoom level {z}"))?;
    let wyrm = caster.fetch_wyrm("tile", peg)?;
    if let Some(wyrm) = wyrm {
        println!("{wyrm}");
    }
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

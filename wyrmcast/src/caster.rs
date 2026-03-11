// caster.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use crate::group::{LayerGroupCfg, LayerGroupDef};
use crate::tile::TileCfg;
use anyhow::Result;
use pointy::BBox;
use serde::Deserialize;
use squarepeg::{MapGrid, Peg};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Configuration for Caster tile layers
#[derive(Debug, Deserialize)]
pub struct CasterCfg {
    /// Address to bind server
    pub bind_address: String,

    /// Tile extent; width and height
    pub tile_extent: u32,

    /// Configuration for all layer groups
    pub layer_group: Vec<LayerGroupCfg>,
}

/// Caster definition.
///
/// To create:
/// * Use `serde` to deserialize a [CasterCfg]
/// * `let caster = Caster::try_from(cfg)?;`
///
/// [CasterCfg]: struct.CasterCfg.html
pub struct CasterDef {
    /// Map grid configuration
    grid: MapGrid,

    /// Tile extent; width and height in pixels
    tile_extent: u32,

    /// Tile layer groups
    groups: Vec<LayerGroupDef>,
}

impl CasterCfg {
    /// Read the configuration file
    pub fn load<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let cfg = read_to_string(path.as_ref())?;
        let cfg: Self = muon_rs::from_str(&cfg)?;
        Ok(cfg)
    }

    /// Get path to a layer `.loam` file
    pub fn loam_path(&self, name: &str) -> PathBuf {
        let mut path = PathBuf::new();
        path.push("loam");
        path.push(format!("{}.loam", name));
        path
    }
}

impl TryFrom<&CasterCfg> for CasterDef {
    type Error = anyhow::Error;

    fn try_from(cfg: &CasterCfg) -> Result<Self> {
        // Only Web Mercator supported for now
        let grid = MapGrid::default();
        let mut groups = vec![];
        for group in &cfg.layer_group {
            groups.push(LayerGroupDef::new(group, cfg)?);
        }
        Ok(CasterDef {
            grid,
            tile_extent: cfg.tile_extent,
            groups,
        })
    }
}

impl CasterDef {
    /// Get layer groups
    pub fn groups(&self) -> &[LayerGroupDef] {
        &self.groups
    }

    /// Query features in a bounding box
    pub fn query_features(&self, bbox: BBox<f64>) -> Result<()> {
        for group in &self.groups {
            log::debug!("query_features group: {}", group.name());
            for layer in group.layers() {
                layer.query_features(bbox)?;
            }
        }
        Ok(())
    }

    /// Create tile config for a `Peg` (tile ID)
    pub fn tile_cfg(&self, peg: Peg) -> TileCfg {
        TileCfg::new(&self.grid, peg, self.tile_extent)
    }
}

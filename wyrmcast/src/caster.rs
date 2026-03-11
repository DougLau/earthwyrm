// config.rs
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

/// Configuration for WyrmCast tile layers.
#[derive(Debug, Deserialize)]
pub struct WyrmCastCfg {
    /// Address to bind server
    pub bind_address: String,

    /// Tile extent; width and height
    pub tile_extent: u32,

    /// Configuration for all layer groups
    pub layer_group: Vec<LayerGroupCfg>,
}

/// WyrmCast definition.
///
/// To create:
/// * Use `serde` to deserialize a [WyrmCastCfg]
/// * `let caster = WyrmCast::try_from(cfg)?;`
///
/// [WyrmCastCfg]: struct.WyrmCastCfg.html
pub struct WyrmCastDef {
    /// Map grid configuration
    grid: MapGrid,

    /// Tile extent; width and height in pixels
    tile_extent: u32,

    /// Tile layer groups
    groups: Vec<LayerGroupDef>,
}

impl WyrmCastCfg {
    /// Read the configuration file
    pub fn load<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let cfg = read_to_string(path.as_ref())?;
        let cfg: Self = muon_rs::from_str(&cfg)?;
        Ok(cfg)
    }

    /// Get path to a layer .loam file
    pub fn loam_path(&self, name: &str) -> PathBuf {
        let mut path = PathBuf::new();
        path.push("loam");
        path.push(format!("{}.loam", name));
        path
    }
}

impl TryFrom<&WyrmCastCfg> for WyrmCastDef {
    type Error = anyhow::Error;

    fn try_from(cfg: &WyrmCastCfg) -> Result<Self> {
        // Only Web Mercator supported for now
        let grid = MapGrid::default();
        let mut groups = vec![];
        for group in &cfg.layer_group {
            groups.push(LayerGroupDef::new(group, cfg)?);
        }
        Ok(WyrmCastDef {
            grid,
            tile_extent: cfg.tile_extent,
            groups,
        })
    }
}

impl WyrmCastDef {
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

    /// Create tile config for a Peg (tile ID)
    pub fn tile_config(&self, peg: Peg) -> TileCfg {
        let tile_extent = self.tile_extent;
        let mut bbox = self.grid.bbox_peg(peg);
        // increase bounding box by edge extent
        let edge = zoom_edge(peg);
        let edge_x = edge * (bbox.x_max() - bbox.x_min());
        let edge_y = edge * (bbox.y_max() - bbox.y_min());
        bbox.extend([
            (bbox.x_min() - edge_x, bbox.y_min() - edge_y),
            (bbox.x_max() + edge_x, bbox.y_max() + edge_y),
        ]);
        let ts = f64::from(tile_extent);
        let transform = self.grid.transform_peg(peg).scale(ts, ts);
        TileCfg::new(tile_extent, peg, bbox, transform)
    }
}

/// Calculate edge ratio based on tile zoom
///
/// Edge must be larger for higher zoom levels to prevent corrupt polygons.
fn zoom_edge(peg: Peg) -> f64 {
    match peg.z() {
        0..=12 => 1.0 / 32.0,
        13 => 1.0 / 16.0,
        14 => 1.0 / 8.0,
        15 => 1.0 / 4.0,
        16 => 1.0 / 2.0,
        _ => 1.0,
    }
}

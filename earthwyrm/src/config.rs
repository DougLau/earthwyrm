// config.rs
//
// Copyright (c) 2019-2024  Minnesota Department of Transportation
//
use crate::error::Result;
use serde_derive::Deserialize;
use std::fmt;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Configuration for Earthwyrm tile layers.
#[derive(Debug, Deserialize)]
pub struct WyrmCfg {
    /// Address to bind server
    pub bind_address: String,

    /// Tile extent; width and height
    pub tile_extent: u32,

    /// Extent outside tile edges
    pub edge_extent: u32,

    /// Configuration for all layer groups
    pub layer_group: Vec<LayerGroupCfg>,
}

/// Layer Group configuration
#[derive(Debug, Deserialize)]
pub struct LayerGroupCfg {
    /// Layer group name
    pub name: String,

    /// Layers in group
    pub layer: Vec<LayerCfg>,
}

/// Layer configuration
#[derive(Debug, Deserialize)]
pub struct LayerCfg {
    /// Layer name
    pub name: String,

    /// Data source (`osm`, `json`)
    pub source: String,

    /// Type for geometry (`point`, `linestring` or `polygon`)
    pub geom_type: String,

    /// Zoom range
    pub zoom: String,

    /// Tag patterns
    pub tags: Vec<String>,
}

impl fmt::Display for LayerGroupCfg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:", self.name)?;
        for layer in &self.layer {
            write!(f, " {}", layer.name)?;
        }
        Ok(())
    }
}

impl WyrmCfg {
    /// Read the configuration file
    pub fn load() -> Result<Self> {
        let path = Path::new("earthwyrm.muon");
        let cfg = read_to_string(path)?;
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

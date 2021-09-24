// config.rs
//
// Copyright (c) 2019-2021  Minnesota Department of Transportation
//
use serde_derive::Deserialize;
use std::fmt;

/// Configuration for Earthwyrm tile layers.
#[derive(Debug, Deserialize)]
pub struct WyrmCfg {
    /// Address to bind server
    pub bind_address: String,

    /// Root path to server static documents
    pub root_path: String,

    /// Tile extent; width and height
    pub tile_extent: u32,

    /// Extent outside tile edges
    pub edge_extent: u32,

    /// Limit of rows per query
    pub query_limit: u32,

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

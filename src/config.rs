// config.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use serde_derive::Deserialize;
use std::fmt;

/// Configuration for Earthwyrm tile layers.
#[derive(Debug, Deserialize)]
pub struct WyrmCfg {
    /// Address to bind server
    pub bind_address: String,
    /// Document root to server static documents
    pub document_root: String,
    /// Tile extent; width and height
    pub tile_extent: u32,
    /// Extent outside tile edges
    pub edge_extent: u32,
    /// Limit of rows per query
    pub query_limit: u32,
    /// Configuration for all database tables
    pub table: Vec<TableCfg>,
    /// Configuration for all layer groups
    pub layer_group: Vec<LayerGroupCfg>,
}

/// Database table configuration (names of columns, etc).
#[derive(Debug, Deserialize)]
pub struct TableCfg {
    /// Name (used by layers)
    pub name: String,
    /// DB table
    pub db_table: String,
    /// Column for unique ID
    pub id_column: String,
    /// Column for PostGIS geometry
    pub geom_column: String,
    /// Type for PostGIS geometry (`polygon`, `linestring` or `point`)
    pub geom_type: String,
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
    /// Table name
    pub table: String,
    /// Zoom range
    pub zoom: String,
    /// Tag patterns
    pub tags: Vec<String>,
}

impl fmt::Display for LayerGroupCfg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: ", self.name)?;
        for layer in &self.layer {
            write!(f, "{} ", layer.name)?;
        }
        Ok(())
    }
}

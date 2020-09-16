// config.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::error::Error;
use crate::map::LayerGroup;
use crate::rules::LayerDef;
use serde_derive::Deserialize;
use std::fs;

/// Configuration for Earthwyrm tile layers.
#[derive(Debug, Deserialize)]
pub struct WyrmCfg {
    /// Address to bind server
    bind_address: String,
    /// Document root to server static documents
    document_root: String,
    /// Extent of tiles for MVT coordinates
    tile_extent: Option<u32>,
    /// Pixels for snap-to-grid tolerance
    pixels: Option<u32>,
    /// Buffer pixels outside tile border
    buffer_pixels: Option<u32>,
    /// Limit of rows per query
    query_limit: Option<u32>,
    /// Table configuration
    table: Vec<TableCfg>,
    /// Layer group configuration
    layer_group: Vec<LayerGroupCfg>,
}

/// Database table configuration (names of columns, etc).
#[derive(Debug, Deserialize)]
pub struct TableCfg {
    /// Name (used by layers)
    name: String,
    /// DB table
    db_table: String,
    /// Column for unique ID
    id_column: String,
    /// Column for PostGIS geometry
    geom_column: String,
    /// Type for PostGIS geometry (polygon, linestring or point)
    geom_type: String,
}

/// Layer Group configuration
#[derive(Debug, Deserialize)]
pub struct LayerGroupCfg {
    /// Base layer group name
    base_name: String,
    /// Layers in group
    layer: Vec<LayerCfg>,
}

/// Layer configuration
#[derive(Debug, Deserialize)]
pub struct LayerCfg {
    /// Layer name
    name: String,
    /// Table name
    table: String,
    /// Zoom range
    zoom: String,
    /// Tag patterns
    tags: Vec<String>,
}

impl WyrmCfg {
    /// Parse from string
    pub fn from_str(cfg: &str) -> Result<Self, Error> {
        Ok(muon_rs::from_str(cfg)?)
    }

    /// Read from file
    pub fn from_file(fname: &str) -> Result<Self, Error> {
        WyrmCfg::from_str(&fs::read_to_string(fname)?)
    }

    /// Get the bind address
    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }

    /// Get the document root
    pub fn document_root(&self) -> &str {
        &self.document_root
    }

    /// Get the layer group configurations
    pub fn layer_groups(&self) -> &[LayerGroupCfg] {
        &self.layer_group
    }

    /// Convert into a `Vec` of `LayerGroup`s
    pub fn into_layer_groups(self) -> Result<Vec<LayerGroup>, Error> {
        let mut groups = vec![];
        for group in self.layer_groups() {
            groups.push(self.layer_group(group)?);
        }
        Ok(groups)
    }

    /// Build a `LayerGroup`
    fn layer_group(&self, group: &LayerGroupCfg) -> Result<LayerGroup, Error> {
        LayerGroup::builder()
            .with_tile_extent(self.tile_extent)
            .with_pixels(self.pixels)
            .with_buffer_pixels(self.buffer_pixels)
            .with_query_limit(self.query_limit)
            .build(&self.table, group)
    }
}

impl TableCfg {
    /// Get table name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get ID column
    pub fn id_column(&self) -> &str {
        &self.id_column
    }

    /// Get geom type
    pub fn geom_type(&self) -> &str {
        &self.geom_type
    }

    /// Build SQL query
    pub fn build_query_sql(&self, tags: &Vec<String>) -> String {
        let mut sql = "SELECT ".to_string();
        // id_column must be first (#0)
        sql.push_str(&self.id_column);
        sql.push_str(",ST_Multi(ST_SimplifyPreserveTopology(ST_SnapToGrid(");
        // geom_column must be second (#1)
        sql.push_str(&self.geom_column);
        sql.push_str(",$1),$1))");
        for tag in tags {
            sql.push_str(",\"");
            sql.push_str(tag);
            sql.push('"');
        }
        sql.push_str(" FROM ");
        sql.push_str(&self.db_table);
        sql.push_str(" WHERE ");
        sql.push_str(&self.geom_column);
        sql.push_str(" && ST_Buffer(ST_MakeEnvelope($2,$3,$4,$5,3857),$6)");
        sql
    }
}

impl LayerGroupCfg {
    /// Get base name
    pub fn base_name(&self) -> &str {
        &self.base_name
    }

    /// Convert to layer defs
    pub fn to_layer_defs(&self) -> Result<Vec<LayerDef>, Error> {
        let mut layers = vec![];
        for layer in &self.layer {
            let layer_def = LayerDef::new(
                &layer.name,
                &layer.table,
                &layer.zoom,
                &layer.tags[..],
            )?;
            layers.push(layer_def);
        }
        let mut names = String::new();
        for layer in &self.layer {
            names.push_str(&layer.name);
            names.push_str(" ");
        }
        log::info!("{} layers loaded:{}", layers.len(), names);
        Ok(layers)
    }
}

// config.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::error::Error;
use crate::map::{LayerGroup, Wyrm};
use crate::rules::LayerDef;
use serde_derive::Deserialize;

/// Configuration for Earthwyrm tile layers.
#[derive(Debug, Deserialize)]
pub struct WyrmCfg {
    /// Address to bind server
    bind_address: String,
    /// Document root to server static documents
    document_root: String,
    /// Tile extent; width and height
    tile_extent: u32,
    /// Extent outside tile edges
    edge_extent: u32,
    /// Limit of rows per query
    query_limit: u32,
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
    /// Layer group name
    name: String,
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
    /// Get the bind address
    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }

    /// Get the document root
    pub fn document_root(&self) -> &str {
        &self.document_root
    }

    /// Get the layer group configurations
    fn layer_groups(&self) -> &[LayerGroupCfg] {
        &self.layer_group
    }

    /// Convert into a `Wyrm`
    pub fn into_wyrm(self) -> Result<Wyrm, Error> {
        let mut groups = vec![];
        for group in self.layer_groups() {
            groups.push(LayerGroup::from_cfg(group, &self.table)?);
        }
        Ok(Wyrm::new(
            self.tile_extent,
            self.edge_extent,
            self.query_limit,
            groups,
        ))
    }
}

impl TableCfg {
    /// Get table name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get DB table
    pub fn db_table(&self) -> &str {
        &self.db_table
    }

    /// Get ID column
    pub fn id_column(&self) -> &str {
        &self.id_column
    }

    /// Get geom column
    pub fn geom_column(&self) -> &str {
        &self.geom_column
    }

    /// Get geom type
    pub fn geom_type(&self) -> &str {
        &self.geom_type
    }
}

impl LayerGroupCfg {
    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
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

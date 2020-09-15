// config.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::error::Error;
use crate::map::TileMaker;
use serde_derive::Deserialize;
use std::fs;

/// Base TOML configuration data
#[derive(Debug, Deserialize)]
pub struct TomlCfg {
    bind_address: String,
    document_root: String,
    tile_extent: Option<u32>,
    pixels: Option<u32>,
    buffer_pixels: Option<u32>,
    query_limit: Option<u32>,
    table: Vec<TableCfg>,
    layer_group: Vec<LayerGroupCfg>,
}

/// Table configuration (names of columns, etc).
#[derive(Debug, Deserialize)]
pub struct TableCfg {
    name: String,
    db_table: String,
    id_column: String,
    geom_column: String,
    geom_type: String,
}

/// Layer Group configuration
#[derive(Debug, Deserialize)]
pub struct LayerGroupCfg {
    base_name: String,
    rules_path: String,
}

impl TomlCfg {
    /// Parse from string
    pub fn from_str(cfg: &str) -> Result<Self, Error> {
        Ok(toml::from_str(cfg)?)
    }

    /// Read from file
    pub fn from_file(fname: &str) -> Result<Self, Error> {
        TomlCfg::from_str(&fs::read_to_string(fname)?)
    }

    /// Get the bind address
    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }

    /// Get the document root
    pub fn document_root(&self) -> &str {
        &self.document_root
    }

    /// Get the table configurations
    pub fn tables(&self) -> &[TableCfg] {
        &self.table
    }

    /// Get the layer group configurations
    pub fn layer_groups(&self) -> &[LayerGroupCfg] {
        &self.layer_group
    }

    /// Convert into a `Vec` of `TileMaker`s (one for each layer group)
    pub fn into_tile_makers(self) -> Result<Vec<TileMaker>, Error> {
        let mut makers = Vec::new();
        for group in self.layer_groups() {
            makers.push(self.tile_maker(group)?);
        }
        Ok(makers)
    }

    /// Build a `TileMaker`
    fn tile_maker(&self, group: &LayerGroupCfg) -> Result<TileMaker, Error> {
        let mut builder = TileMaker::builder();
        if let Some(tile_extent) = self.tile_extent {
            builder.set_tile_extent(tile_extent);
        }
        if let Some(pixels) = self.pixels {
            builder.set_pixels(pixels);
        }
        if let Some(buffer_pixels) = self.buffer_pixels {
            builder.set_buffer_pixels(buffer_pixels);
        }
        if let Some(query_limit) = self.query_limit {
            builder.set_query_limit(query_limit);
        }
        builder.build(self.tables(), group)
    }
}

impl TableCfg {
    /// Create a new table configuration
    pub fn new(
        name: &str,
        db_table: &str,
        id_column: &str,
        geom_column: &str,
        geom_type: &str,
    ) -> Self {
        let name = name.to_string();
        let db_table = db_table.to_string();
        let id_column = id_column.to_string();
        let geom_column = geom_column.to_string();
        let geom_type = geom_type.to_string();
        TableCfg {
            name,
            db_table,
            id_column,
            geom_column,
            geom_type,
        }
    }

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
    /// Create a new layer group configuration
    pub fn new(base_name: &str, rules_path: &str) -> Self {
        let base_name = base_name.to_string();
        let rules_path = rules_path.to_string();
        LayerGroupCfg {
            base_name,
            rules_path,
        }
    }

    /// Get base name
    pub fn base_name(&self) -> &str {
        &self.base_name
    }

    /// Get rules path
    pub fn rules_path(&self) -> &str {
        &self.rules_path
    }
}

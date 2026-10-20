// group.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use crate::layer::{LayerCfg, LayerDef, LayerTree};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt;
use std::path::PathBuf;

/// Layer Group configuration
#[derive(Debug, Deserialize)]
pub struct LayerGroupCfg {
    /// Layer group name
    pub name: String,

    /// OpenStreetMap data source
    pub osm: bool,

    /// Base path where loam files are stored
    pub base_path: Option<String>,

    /// Layers in group
    pub layer: Vec<LayerCfg>,
}

/// Group of layers for making tiles
pub struct LayerGroupDef {
    /// Name of group
    name: String,

    /// OpenStreetMap data source
    osm: bool,

    /// Layer definitions / trees
    layers: Vec<LayerTree>,
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

impl LayerGroupCfg {
    /// Get path to a layer `.loam` file
    pub fn loam_path(&self, name: &str) -> PathBuf {
        let mut path = PathBuf::new();
        if let Some(bp) = &self.base_path {
            path.push(bp);
        }
        path.push("loam");
        path.push(format!("{name}.loam"));
        path
    }
}

impl LayerGroupDef {
    /// Create a new layer group
    pub fn new(group: &LayerGroupCfg) -> Result<Self> {
        let name = group.name.to_string();
        let mut layers = vec![];
        for layer_cfg in &group.layer {
            let layer_def = LayerDef::try_from(layer_cfg)?;
            let path = group.loam_path(layer_def.name());
            let tree = LayerTree::new(layer_def, &path)
                .with_context(|| format!("path: {:?}", path))?;
            layers.push(tree);
        }
        log::info!("{} layers in {group}", layers.len());
        let osm = group.osm;
        Ok(LayerGroupDef { name, osm, layers })
    }

    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get OSM flag
    pub fn osm(&self) -> bool {
        self.osm
    }

    /// Get layers in the group
    pub fn layers(&self) -> &[LayerTree] {
        &self.layers
    }
}

// group.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use crate::caster::CasterCfg;
use crate::layer::{LayerCfg, LayerDef, LayerTree};
use anyhow::Result;
use serde::Deserialize;
use std::fmt;

/// Layer Group configuration
#[derive(Debug, Deserialize)]
pub struct LayerGroupCfg {
    /// Layer group name
    pub name: String,

    /// OpenStreetMap data source
    pub osm: bool,

    /// Layers in group
    pub layer: Vec<LayerCfg>,
}

/// Group of layers for making tiles
pub struct LayerGroupDef {
    /// Name of group
    name: String,

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

impl LayerGroupDef {
    /// Create a new layer group
    pub fn new(group: &LayerGroupCfg, cfg: &CasterCfg) -> Result<Self> {
        let name = group.name.to_string();
        let mut layers = vec![];
        for layer_cfg in &group.layer {
            let layer_def = LayerDef::try_from(layer_cfg)?;
            let path = cfg.loam_path(layer_def.name());
            layers.push(LayerTree::new(layer_def, path)?);
        }
        log::info!("{} layers in {group}", layers.len());
        Ok(LayerGroupDef { name, layers })
    }

    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get layers in the group
    pub fn layers(&self) -> &[LayerTree] {
        &self.layers
    }
}

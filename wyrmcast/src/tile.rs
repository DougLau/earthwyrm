// tile.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use crate::config::{LayerGroupCfg, WyrmCastCfg};
use crate::geom::GeomTree;
use crate::layer::LayerDef;
use anyhow::Result;
use pointy::{BBox, Transform};
use squarepeg::{MapGrid, Peg};

/// Tile configuration
pub struct TileCfg {
    /// Tile extent; width and height in pixels
    tile_extent: u32,

    /// Peg (tile ID)
    peg: Peg,

    /// Bounding box of tile (including edge extent)
    bbox: BBox<f64>,

    /// Transform from spatial to tile coordinates
    transform: Transform<f64>,
}

/// Layer tree
pub struct LayerTree {
    /// Layer definition
    layer_def: LayerDef,

    /// R-Tree of geometry
    tree: GeomTree,
}

/// Group of layers for making tiles
pub struct LayerGroup {
    /// Name of group
    name: String,

    /// Layer definitions / trees
    layers: Vec<LayerTree>,
}

/// WyrmCast tile fetcher.
///
/// To create:
/// * Use `serde` to deserialize a [WyrmCastCfg]
/// * `let caster = WyrmCast::try_from(cfg)?;`
///
/// [WyrmCastCfg]: struct.WyrmCastCfg.html
pub struct WyrmCast {
    /// Map grid configuration
    grid: MapGrid,

    /// Tile extent; width and height in pixels
    tile_extent: u32,

    /// Tile layer groups
    groups: Vec<LayerGroup>,
}

impl TileCfg {
    /// Get the tile extent
    pub fn tile_extent(&self) -> u32 {
        self.tile_extent
    }

    /// Get the tile Peg
    pub fn peg(&self) -> Peg {
        self.peg
    }

    /// Get the zoom level
    pub fn zoom(&self) -> u32 {
        self.peg.z()
    }

    /// Get the bounding box (including edge extent)
    pub fn bbox(&self) -> BBox<f64> {
        self.bbox
    }

    /// Get the tile transform
    pub fn transform(&self) -> Transform<f64> {
        self.transform
    }
}

impl LayerGroup {
    /// Create a new layer group
    fn new(group: &LayerGroupCfg, cfg: &WyrmCastCfg) -> Result<Self> {
        let name = group.name.to_string();
        let mut layers = vec![];
        for layer_cfg in &group.layer {
            let layer_def = LayerDef::try_from(layer_cfg)?;
            layers.push(LayerTree::new(layer_def, cfg)?);
        }
        log::info!("{} layers in {group}", layers.len());
        Ok(LayerGroup { name, layers })
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

impl TryFrom<&WyrmCastCfg> for WyrmCast {
    type Error = anyhow::Error;

    fn try_from(cfg: &WyrmCastCfg) -> Result<Self> {
        // Only Web Mercator supported for now
        let grid = MapGrid::default();
        let mut groups = vec![];
        for group in &cfg.layer_group {
            groups.push(LayerGroup::new(group, cfg)?);
        }
        Ok(WyrmCast {
            grid,
            tile_extent: cfg.tile_extent,
            groups,
        })
    }
}

impl WyrmCast {
    /// Get layer groups
    pub fn groups(&self) -> &[LayerGroup] {
        &self.groups
    }

    /// Query features in a bounding box
    pub fn query_features(&self, bbox: BBox<f64>) -> Result<()> {
        for group in &self.groups {
            log::debug!("query_features group: {:?}", group.name);
            for layer in &group.layers {
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
        TileCfg {
            tile_extent,
            peg,
            bbox,
            transform,
        }
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

impl LayerTree {
    /// Create a new layer tree
    fn new(layer_def: LayerDef, cfg: &WyrmCastCfg) -> Result<Self> {
        let loam = cfg.loam_path(layer_def.name());
        let tree = GeomTree::new(layer_def.geom_tp(), loam)?;
        Ok(LayerTree { layer_def, tree })
    }

    /// Get layer definition
    pub fn layer_def(&self) -> &LayerDef {
        &self.layer_def
    }

    /// Get geometry tree
    pub fn tree(&self) -> &GeomTree {
        &self.tree
    }

    /// Query layer features in a bounding box
    fn query_features(&self, bbox: BBox<f64>) -> Result<()> {
        self.tree.query_features(&self.layer_def, bbox)
    }
}

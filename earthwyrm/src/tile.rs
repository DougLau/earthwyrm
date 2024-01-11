// tile.rs
//
// Copyright (c) 2019-2024  Minnesota Department of Transportation
//
use crate::config::{LayerGroupCfg, WyrmCfg};
use crate::error::{Error, Result};
use crate::geom::GeomTree;
use crate::layer::LayerDef;
use mvt::{Layer, MapGrid, Tile, TileId};
use pointy::{BBox, Transform};
use std::io::Write;
use std::time::Instant;

/// Tile configuration
pub struct TileCfg {
    /// Tile extent; width and height in pixels
    tile_extent: u32,

    /// Tile ID
    tid: TileId,

    /// Bounding box of tile (including edge extent)
    bbox: BBox<f64>,

    /// Transform from spatial to tile coordinates
    transform: Transform<f64>,
}

/// Layer tree
struct LayerTree {
    /// Layer definition
    layer_def: LayerDef,

    /// R-Tree of geometry
    tree: GeomTree,
}

/// Group of layers for making tiles
struct LayerGroup {
    /// Name of group
    name: String,

    /// Layer definitions / trees
    layers: Vec<LayerTree>,
}

/// Wyrm tile fetcher.
///
/// To create:
/// * Use `serde` to deserialize a [WyrmCfg]
/// * `let wyrm = Wyrm::try_from(wyrm_cfg)?;`
///
/// [WyrmCfg]: struct.WyrmCfg.html
pub struct Wyrm {
    /// Map grid configuration
    grid: MapGrid,

    /// Tile extent; width and height in pixels
    tile_extent: u32,

    /// Extent outside tile edges (pixels)
    edge_extent: u32,

    /// Tile layer groups
    groups: Vec<LayerGroup>,
}

impl TileCfg {
    /// Get the zoom level
    pub fn zoom(&self) -> u32 {
        self.tid.z()
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
    fn new(group: &LayerGroupCfg, wyrm: &WyrmCfg) -> Result<Self> {
        let name = group.name.to_string();
        let mut layers = vec![];
        for layer_cfg in &group.layer {
            let layer_def = LayerDef::try_from(layer_cfg)?;
            layers.push(LayerTree::new(layer_def, wyrm)?);
        }
        log::info!("{} layers in {group}", layers.len());
        Ok(LayerGroup { name, layers })
    }

    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Fetch a tile
    fn fetch_tile(&self, tile_cfg: &TileCfg) -> Result<Tile> {
        let t = Instant::now();
        let tile = self.query_tile(tile_cfg)?;
        log::info!(
            "{}/{}, fetched {} bytes in {:.2?}",
            self.name(),
            tile_cfg.tid,
            tile.compute_size(),
            t.elapsed()
        );
        Ok(tile)
    }

    /// Query one tile from trees
    fn query_tile(&self, tile_cfg: &TileCfg) -> Result<Tile> {
        let mut tile = Tile::new(tile_cfg.tile_extent);
        for layer_tree in &self.layers {
            let layer = layer_tree.query_tile(&tile, tile_cfg)?;
            if layer.num_features() > 0 {
                tile.add_layer(layer)?;
            }
        }
        Ok(tile)
    }

    /// Write group layers to a tile
    fn write_tile<W: Write>(
        &self,
        out: &mut W,
        tile_cfg: TileCfg,
    ) -> Result<()> {
        let tile = self.fetch_tile(&tile_cfg)?;
        if tile.num_layers() > 0 {
            tile.write_to(out)?;
            Ok(())
        } else {
            log::debug!("tile {} empty (no layers)", tile_cfg.tid);
            Err(Error::TileEmpty())
        }
    }
}

impl TryFrom<&WyrmCfg> for Wyrm {
    type Error = Error;

    fn try_from(wyrm_cfg: &WyrmCfg) -> Result<Self> {
        // Only Web Mercator supported for now
        let grid = MapGrid::default();
        let mut groups = vec![];
        for group in &wyrm_cfg.layer_group {
            groups.push(LayerGroup::new(group, wyrm_cfg)?);
        }
        Ok(Wyrm {
            grid,
            tile_extent: wyrm_cfg.tile_extent,
            edge_extent: wyrm_cfg.edge_extent,
            groups,
        })
    }
}

impl Wyrm {
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

    /// Fetch one tile.
    ///
    /// * `out` Writer to write MVT data.
    /// * `group_name` Name of layer group.
    /// * `tid` Tile ID.
    pub fn fetch_tile<W: Write>(
        &self,
        out: &mut W,
        group_name: &str,
        tid: TileId,
    ) -> Result<()> {
        for group in &self.groups {
            if group_name == group.name() {
                let tile_cfg = self.tile_config(tid);
                return group.write_tile(out, tile_cfg);
            }
        }
        log::debug!("unknown group name: {}", group_name);
        Err(Error::UnknownGroupName())
    }

    /// Create tile config for a tile ID
    fn tile_config(&self, tid: TileId) -> TileCfg {
        let tile_extent = self.tile_extent;
        let mut bbox = self.grid.tile_bbox(tid);
        // increase bounding box by edge extent
        let edge = f64::from(self.edge_extent) / f64::from(tile_extent);
        let edge_x = edge * (bbox.x_max() - bbox.x_min());
        let edge_y = edge * (bbox.y_max() - bbox.y_min());
        bbox.extend([
            (bbox.x_min() - edge_x, bbox.y_min() - edge_y),
            (bbox.x_max() + edge_x, bbox.y_max() + edge_y),
        ]);
        let ts = f64::from(tile_extent);
        let transform = self.grid.tile_transform(tid).scale(ts, ts);
        TileCfg {
            tile_extent,
            tid,
            bbox,
            transform,
        }
    }
}

impl LayerTree {
    /// Create a new layer tree
    fn new(layer_def: LayerDef, wyrm: &WyrmCfg) -> Result<Self> {
        let loam = wyrm.loam_path(layer_def.name());
        let tree = GeomTree::new(layer_def.geom_tp(), loam)?;
        Ok(LayerTree { layer_def, tree })
    }

    /// Query layer features in a bounding box
    fn query_features(&self, bbox: BBox<f64>) -> Result<()> {
        self.tree.query_features(&self.layer_def, bbox)
    }

    /// Query tile features
    fn query_tile(&self, tile: &Tile, tile_cfg: &TileCfg) -> Result<Layer> {
        let layer = tile.create_layer(self.layer_def.name());
        if self.layer_def.check_zoom(tile_cfg.zoom()) {
            self.tree.query_tile(&self.layer_def, layer, tile_cfg)
        } else {
            Ok(layer)
        }
    }
}

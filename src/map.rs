// map.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::config::{LayerGroupCfg, TableCfg};
use crate::geom::{lookup_geom_type, GeomRow};
use crate::rules::LayerDef;
use crate::Error;
use log::{debug, info, warn};
use mvt::{BBox, GeomType, Layer, MapGrid, Tile, TileId, Transform};
use postgres::fallible_iterator::FallibleIterator;
use postgres::types::ToSql;
use postgres::Client;
use postgres::Row;
use std::io::Write;
use std::time::Instant;

/// Table definition (tags, sql query, etc)
#[derive(Clone, Debug)]
struct TableDef {
    /// Table name
    name: String,
    /// ID column
    id_column: String,
    /// Geometry type
    geom_type: GeomType,
    /// Tag patterns
    tags: Vec<String>,
    /// SQL query string
    sql: String,
}

/// Tile configuration
struct TileConfig {
    /// Tile ID
    tid: TileId,
    /// Bounding box
    bbox: BBox,
    /// Geometry transform
    transform: Transform,
    /// Pixel size
    pixel_sz: f64,
}

/// Builder for layer groups
#[derive(Default)]
pub struct LayerGroupBuilder {
    /// Tile extent
    tile_extent: Option<u32>,
    /// Pixels in tile
    pixels: Option<u32>,
    /// Buffer pixels at edges
    buffer_pixels: Option<u32>,
    /// Query row limit
    query_limit: Option<u32>,
}

/// Group of layers for making tiles
#[derive(Clone)]
pub struct LayerGroup {
    /// Name of group
    name: String,
    /// Tile extent
    tile_extent: u32,
    /// Pixels in tile
    pixels: u32,
    /// Buffer pixels at edges
    buffer_pixels: u32,
    /// Query row limit
    query_limit: u32,
    /// Map grid configuration
    grid: MapGrid,
    /// Layer definitions
    layer_defs: Vec<LayerDef>,
    /// Table definitions
    table_defs: Vec<TableDef>,
}

impl TableDef {
    /// Create a new table definition
    fn new(table_cfg: &TableCfg, layer_defs: &[LayerDef]) -> Option<Self> {
        let name = &table_cfg.name();
        let id_column = table_cfg.id_column().to_string();
        let geom_type = lookup_geom_type(&table_cfg.geom_type())?;
        let tags = TableDef::table_tags(name, layer_defs);
        if tags.len() > 0 {
            let name = name.to_string();
            let sql = table_cfg.build_query_sql(&tags);
            Some(TableDef {
                name,
                id_column,
                geom_type,
                tags,
                sql,
            })
        } else {
            None
        }
    }

    /// Get the tags requested for the table from defined layers
    fn table_tags(name: &str, layer_defs: &[LayerDef]) -> Vec<String> {
        let mut tags = Vec::<String>::new();
        for ld in layer_defs {
            if ld.table() == name {
                for pattern in ld.patterns() {
                    let tag = pattern.tag();
                    if !tags.iter().any(|t| t == tag) {
                        tags.push(tag.to_string());
                    }
                }
            }
        }
        tags
    }
}

impl TileConfig {
    /// Get the zoom level
    fn zoom(&self) -> u32 {
        self.tid.z()
    }
}

impl LayerGroupBuilder {
    /// Set the tile extent (within MVT files)
    pub fn with_tile_extent(mut self, tile_extent: Option<u32>) -> Self {
        self.tile_extent = tile_extent;
        self
    }

    /// Set the tile pixels
    pub fn with_pixels(mut self, pixels: Option<u32>) -> Self {
        self.pixels = pixels;
        self
    }

    /// Set the buffer pixels (at tile edges)
    pub fn with_buffer_pixels(mut self, buffer_pixels: Option<u32>) -> Self {
        self.buffer_pixels = buffer_pixels;
        self
    }

    /// Set the query limit
    pub fn with_query_limit(mut self, query_limit: Option<u32>) -> Self {
        self.query_limit = query_limit;
        self
    }

    /// Build the layer group
    pub fn build(
        self,
        table_cfgs: &[TableCfg],
        layer_group: &LayerGroupCfg,
    ) -> Result<LayerGroup, Error> {
        let layer_defs = layer_group.to_layer_defs()?;
        let table_defs = self.build_table_defs(&layer_defs, table_cfgs);
        let name = layer_group.name().to_string();
        let tile_extent = self.tile_extent.unwrap_or(4096);
        let pixels = self.pixels.unwrap_or(256);
        let buffer_pixels = self.buffer_pixels.unwrap_or(0);
        let query_limit = self.query_limit.unwrap_or(u32::MAX);
        let grid = MapGrid::default();
        Ok(LayerGroup {
            name,
            tile_extent,
            pixels,
            buffer_pixels,
            query_limit,
            grid,
            layer_defs,
            table_defs,
        })
    }

    /// Build the table definitions
    fn build_table_defs(
        &self,
        layer_defs: &[LayerDef],
        table_cfgs: &[TableCfg],
    ) -> Vec<TableDef> {
        let mut table_defs = vec![];
        for table_cfg in table_cfgs {
            if let Some(table_def) = TableDef::new(table_cfg, layer_defs) {
                table_defs.push(table_def);
            }
        }
        table_defs
    }
}

impl LayerGroup {
    /// Create a builder for LayerGroup
    pub fn builder() -> LayerGroupBuilder {
        LayerGroupBuilder::default()
    }

    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Find a layer by name
    fn find_layer(&self, name: &str) -> Option<&LayerDef> {
        self.layer_defs.iter().find(|ld| ld.name() == name)
    }

    /// Create all layers for a tile
    fn create_layers(&self, tile: &Tile) -> Vec<Layer> {
        self.layer_defs
            .iter()
            .map(|ld| tile.create_layer(&ld.name()))
            .collect()
    }

    /// Check one table for matching layers
    fn check_layers(&self, table_def: &TableDef, zoom: u32) -> bool {
        let table = &table_def.name;
        self.layer_defs.iter().any(|l| l.check_table(table, zoom))
    }

    /// Create tile config for a tile ID
    fn tile_config(&self, tid: TileId) -> TileConfig {
        let bbox = self.grid.tile_bbox(tid);
        let tile_sz =
            (bbox.x_max() - bbox.x_min()).max(bbox.y_max() - bbox.y_min());
        let pixel_sz = tile_sz / self.pixels as f64;
        debug!("tile {}, pixel_sz {:?}", tid, pixel_sz);
        let ts = self.tile_extent as f64;
        let transform = self.grid.tile_transform(tid).scale(ts, ts);
        TileConfig {
            tid,
            bbox,
            transform,
            pixel_sz,
        }
    }

    /// Fetch a tile
    fn fetch_tile(
        &self,
        client: &mut Client,
        tid: TileId,
    ) -> Result<Tile, Error> {
        let config = self.tile_config(tid);
        let t = Instant::now();
        let tile = self.query_tile(client, &config)?;
        info!(
            "{} {}, fetched {} bytes in {:?}",
            self.name(),
            tid,
            tile.compute_size(),
            t.elapsed()
        );
        Ok(tile)
    }

    /// Query one tile from DB
    fn query_tile(
        &self,
        client: &mut Client,
        config: &TileConfig,
    ) -> Result<Tile, Error> {
        let mut tile = Tile::new(self.tile_extent);
        let mut layers = self.create_layers(&tile);
        for table_def in &self.table_defs {
            if self.check_layers(table_def, config.zoom()) {
                self.query_layers(client, table_def, &mut layers, config)?;
            }
        }
        for layer in layers.drain(..) {
            if layer.num_features() > 0 {
                tile.add_layer(layer)?;
            }
        }
        Ok(tile)
    }

    /// Query layers for one table
    fn query_layers(
        &self,
        client: &mut Client,
        table_def: &TableDef,
        layers: &mut Vec<Layer>,
        config: &TileConfig,
    ) -> Result<(), Error> {
        debug!("sql: {}", &table_def.sql);
        let mut trans = client.transaction()?;
        let stmt = trans.prepare(&table_def.sql)?;
        let x_min = config.bbox.x_min();
        let y_min = config.bbox.y_min();
        let x_max = config.bbox.x_max();
        let y_max = config.bbox.y_max();
        let tol = config.pixel_sz;
        let rad = tol * self.buffer_pixels as f64;
        let params: Vec<&(dyn ToSql + Sync)> =
            vec![&tol, &x_min, &y_min, &x_max, &y_max, &rad];
        debug!("params: {:?}", params);
        let portal = trans.bind(&stmt, &params[..])?;
        let mut remaining_limit = self.query_limit;
        while remaining_limit > 0 {
            let before_limit = remaining_limit;
            // Fetch next set of rows from portal
            let mut rows = trans.query_portal_raw(&portal, 50)?;
            while let Some(row) = rows.next()? {
                self.add_layer_features(table_def, &row, config, layers)?;
                remaining_limit -= 1;
            }
            if before_limit == remaining_limit {
                break;
            }
        }
        if remaining_limit == 0 {
            warn!(
                "table {}, query limit reached: {}",
                &table_def.name, self.query_limit
            );
        }
        Ok(())
    }

    /// Add features to a layer
    fn add_layer_features(
        &self,
        table_def: &TableDef,
        row: &Row,
        config: &TileConfig,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        let table = &table_def.name;
        let grow = GeomRow::new(row, table_def.geom_type, &table_def.id_column);
        for layer in layers {
            if let Some(layer_def) = self.find_layer(layer.name()) {
                if layer_def.check_table(table, config.zoom())
                    && grow.matches_layer(layer_def)
                {
                    if let Some(geom) = grow.get_geometry(&config.transform)? {
                        let lyr = std::mem::replace(layer, Layer::default());
                        *layer = grow.add_feature(lyr, layer_def, geom);
                    }
                }
            }
        }
        Ok(())
    }

    /// Write a tile
    pub fn write_tile<W: Write>(
        &self,
        out: &mut W,
        client: &mut Client,
        xtile: u32,
        ytile: u32,
        zoom: u32,
    ) -> Result<(), Error> {
        let tid = TileId::new(xtile, ytile, zoom)?;
        let tile = self.fetch_tile(client, tid)?;
        if tile.num_layers() > 0 {
            tile.write_to(out)?;
            Ok(())
        } else {
            debug!("tile {} empty (no layers)", tid);
            Err(Error::TileEmpty())
        }
    }
}

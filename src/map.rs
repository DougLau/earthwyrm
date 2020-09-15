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
use std::fs::File;
use std::io::Write;
use std::time::Instant;

/// Table definition (tags, sql query, etc)
#[derive(Clone, Debug)]
struct TableDef {
    name: String,
    id_column: String,
    geom_type: GeomType,
    tags: Vec<String>,
    sql: String,
}

/// Tile configuration
struct TileConfig {
    tid: TileId,
    bbox: BBox,
    transform: Transform,
    pixel_sz: f64,
}

/// Builder for tile maker
pub struct TileMakerBuilder {
    tile_extent: u32,
    pixels: u32,
    buffer_pixels: u32,
    query_limit: u32,
}

/// Map tile maker
#[derive(Clone)]
pub struct TileMaker {
    base_name: String,
    tile_extent: u32,
    pixels: u32,
    buffer_pixels: u32,
    query_limit: u32,
    grid: MapGrid,
    layer_defs: Vec<LayerDef>,
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

impl Default for TileMakerBuilder {
    /// Create a new TileMaker builder
    fn default() -> Self {
        let tile_extent = 4096;
        let pixels = 256;
        let buffer_pixels = 0;
        let query_limit = std::u32::MAX;
        TileMakerBuilder {
            tile_extent,
            pixels,
            buffer_pixels,
            query_limit,
        }
    }
}

impl TileMakerBuilder {
    /// Set the tile extent (within MVT files)
    pub fn set_tile_extent(&mut self, tile_extent: u32) {
        self.tile_extent = tile_extent;
    }

    /// Set the tile pixels
    pub fn set_pixels(&mut self, pixels: u32) {
        self.pixels = pixels;
    }

    /// Set the buffer pixels (at tile edges)
    pub fn set_buffer_pixels(&mut self, buffer_pixels: u32) {
        self.buffer_pixels = buffer_pixels;
    }

    /// Set the query limit
    pub fn set_query_limit(&mut self, query_limit: u32) {
        self.query_limit = query_limit;
    }

    /// Build the tile maker
    pub fn build(
        self,
        table_cfgs: &[TableCfg],
        layer_group: &LayerGroupCfg,
    ) -> Result<TileMaker, Error> {
        let layer_defs = LayerDef::load_all(layer_group.rules_path())?;
        let table_defs = self.build_table_defs(&layer_defs, table_cfgs);
        let base_name = layer_group.base_name().to_string();
        let tile_extent = self.tile_extent;
        let pixels = self.pixels;
        let buffer_pixels = self.buffer_pixels;
        let query_limit = self.query_limit;
        let grid = MapGrid::new_web_mercator();
        Ok(TileMaker {
            base_name,
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

impl TileMaker {
    /// Create a builder for TileMaker
    pub fn builder() -> TileMakerBuilder {
        TileMakerBuilder::default()
    }

    /// Get the base name
    pub fn base_name(&self) -> &str {
        &self.base_name
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
        conn: &mut Client,
        tid: TileId,
    ) -> Result<Tile, Error> {
        let config = self.tile_config(tid);
        let t = Instant::now();
        let tile = self.query_tile(conn, &config)?;
        info!(
            "{} {}, fetched {} bytes in {:?}",
            self.base_name(),
            tid,
            tile.compute_size(),
            t.elapsed()
        );
        Ok(tile)
    }

    /// Query one tile from DB
    fn query_tile(
        &self,
        conn: &mut Client,
        config: &TileConfig,
    ) -> Result<Tile, Error> {
        let mut tile = Tile::new(self.tile_extent);
        let mut layers = self.create_layers(&tile);
        for table_def in &self.table_defs {
            if self.check_layers(table_def, config.zoom()) {
                self.query_layers(conn, table_def, &mut layers, config)?;
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
        conn: &mut Client,
        table_def: &TableDef,
        layers: &mut Vec<Layer>,
        config: &TileConfig,
    ) -> Result<(), Error> {
        debug!("sql: {}", &table_def.sql);
        let mut trans = conn.transaction()?;
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
        let mut n_rows = 0;
        let row_limit = self.row_limit();
        loop {
            let start = n_rows;
            let mut rows = trans.query_portal_raw(&portal, row_limit)?;
            while let Some(row) = rows.next()? {
                self.add_layer_features(table_def, &row, config, layers)?;
                if n_rows == self.query_limit {
                    warn!(
                        "table {}, query limit reached: {}",
                        &table_def.name, n_rows
                    );
                    return Ok(());
                }
                n_rows += 1;
            }
            if start == n_rows {
                break;
            }
        }
        Ok(())
    }

    /// Get the row limit for a lazy query
    fn row_limit(&self) -> i32 {
        if self.query_limit < 50 {
            self.query_limit as i32
        } else {
            50
        }
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
        // FIXME: can this be done without a temp vec?
        let mut lyrs: Vec<Layer> = layers.drain(..).collect();
        for mut layer in lyrs.drain(..) {
            if let Some(layer_def) = self.find_layer(layer.name()) {
                if layer_def.check_table(table, config.zoom())
                    && grow.matches_layer(layer_def)
                {
                    if let Some(geom) = grow.get_geometry(&config.transform)? {
                        layer = grow.add_feature(layer, layer_def, geom);
                    }
                }
            }
            layers.push(layer);
        }
        Ok(())
    }

    /// Write a tile to a file
    pub fn write_tile(
        &self,
        conn: &mut Client,
        xtile: u32,
        ytile: u32,
        zoom: u32,
    ) -> Result<(), Error> {
        let tid = TileId::new(xtile, ytile, zoom)?;
        let fname = format!("{}/{}.mvt", &self.base_name, tid);
        let mut f = File::create(fname)?;
        self.write_to(conn, tid, &mut f)
    }

    /// Write a tile
    pub fn write_to(
        &self,
        conn: &mut Client,
        tid: TileId,
        out: &mut dyn Write,
    ) -> Result<(), Error> {
        let tile = self.fetch_tile(conn, tid)?;
        if tile.num_layers() > 0 {
            tile.write_to(out)?;
        } else {
            debug!("tile {} not written (no layers)", tid);
        }
        Ok(())
    }

    /// Write a tile to a buffer
    pub fn write_buf(
        &self,
        conn: &mut Client,
        xtile: u32,
        ytile: u32,
        zoom: u32,
    ) -> Result<Vec<u8>, Error> {
        let tid = TileId::new(xtile, ytile, zoom)?;
        let tile = self.fetch_tile(conn, tid)?;
        if tile.num_layers() > 0 {
            Ok(tile.to_bytes()?)
        } else {
            debug!("tile {} empty (no layers)", tid);
            Err(Error::TileEmpty())
        }
    }
}

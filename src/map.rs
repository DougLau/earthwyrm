// map.rs
//
// Copyright (c) 2019  Minnesota Department of Transportation
//
use crate::Error;
use crate::config::{LayerGroupCfg, TableCfg};
use crate::rules::{IncludeValue, LayerDef, MustMatch};
use fallible_iterator::FallibleIterator;
use log::{debug, info, trace, warn};
use mvt::{
    BBox, Feature, GeomData, GeomEncoder, GeomType, Layer, MapGrid, Tile,
    TileId, Transform,
};
use postgis::ewkb;
use postgres::rows::Row;
use postgres::types::{FromSql, ToSql};
use postgres::Connection;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

/// Lookup a geometry type from a string name
fn lookup_geom_type(geom_type: &str) -> Option<GeomType> {
    match geom_type {
        "polygon" => Some(GeomType::Polygon),
        "linestring" => Some(GeomType::Linestring),
        "point" => Some(GeomType::Point),
        _ => {
            warn!("unknown geom type: {}", geom_type);
            None
        },
    }
}

/// Table definition (tags, sql query, etc)
#[derive(Clone, Debug)]
struct TableDef {
    name: String,
    id_column: String,
    geom_type: GeomType,
    tags: Vec<String>,
    sql: String,
}

/// Builder for tile maker
pub struct Builder {
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

impl LayerDef {
    /// Check if a row matches the layer rule
    fn matches(&self, row: &Row) -> bool {
        for pattern in self.patterns() {
            if pattern.must_match == MustMatch::Yes {
                let key = pattern.tag();
                if pattern.matches_key(key) {
                    if !pattern.matches_value(self.get_tag_value(row, key)) {
                        return false;
                    }
                }
            }
        }
        true
    }
    /// Get tags from a row and add them to a feature
    fn get_tags(&self, id_column: &str, feature: &mut Feature, row: &Row) {
        // id_column is always #0 (see build_query_sql)
        let fid = row.get::<_, i64>(0);
        trace!("layer {}, fid {}", self.name(), fid);
        // NOTE: Leaflet apparently can't use mvt feature id; use tag/property
        feature.add_tag_sint(id_column, fid);
        for pattern in self.patterns() {
            if let IncludeValue::Yes = pattern.include {
                let key = pattern.tag();
                if let Some(v) = self.get_tag_value(row, key) {
                    feature.add_tag_string(key, &v);
                    trace!("layer {}, {}={}", self.name(), key, &v);
                }
            }
        }
    }
    /// Get one tag value (string)
    fn get_tag_value(&self, row: &Row, col: &str) -> Option<String> {
        if let Some(v) = row.get::<_, Option<String>>(col) {
            if v.len() > 0 {
                return Some(v);
            }
        }
        None
    }
    /// Add a feature to a layer (if it matches)
    fn add_feature(
        &self,
        layer: Layer,
        id_column: &str,
        geom_type: &GeomType,
        row: &Row,
        transform: &Transform,
    ) -> Result<Layer, Error> {
        if !self.matches(row) {
            return Ok(layer);
        }
        match get_geometry(geom_type, row, transform)? {
            Some(gd) => {
                let mut feature = layer.into_feature(gd);
                self.get_tags(id_column, &mut feature, row);
                Ok(feature.into_layer())
            }
            None => Ok(layer),
        }
    }
}

/// Get geometry from a row, encoded as MVT GeomData
fn get_geometry(geom_type: &GeomType, row: &Row, t: &Transform) -> GeomResult {
    match geom_type {
        GeomType::Point => get_geom_data(row, t, &encode_points),
        GeomType::Linestring => get_geom_data(row, t, &encode_linestrings),
        GeomType::Polygon => get_geom_data(row, t, &encode_polygons),
    }
}

type GeomResult = Result<Option<GeomData>, Error>;

/// Get geom data from a row
fn get_geom_data<T: FromSql>(
    row: &Row,
    t: &Transform,
    enc: &Fn(T, &Transform) -> GeomResult,
) -> GeomResult {
    // geom_column is always #1 (see build_query_sql)
    match row.get_opt(1) {
        Some(Ok(Some(g))) => enc(g, t),
        Some(Err(e)) => Err(Error::Pg(e)),
        _ => Ok(None),
    }
}

/// Encode points into GeomData
fn encode_points(g: ewkb::MultiPoint, t: &Transform) -> GeomResult {
    if g.points.len() == 0 {
        return Ok(None);
    }
    let mut ge = GeomEncoder::new(GeomType::Point, *t);
    for p in &g.points {
        ge.add_point(p.x, p.y);
    }
    Ok(Some(ge.encode()?))
}

/// Encode linestrings into GeomData
fn encode_linestrings(g: ewkb::MultiLineString, t: &Transform) -> GeomResult {
    if g.lines.len() == 0 {
        return Ok(None);
    }
    let mut ge = GeomEncoder::new(GeomType::Linestring, *t);
    for ls in &g.lines {
        ge.complete_geom()?;
        for p in &ls.points {
            ge.add_point(p.x, p.y);
        }
    }
    Ok(Some(ge.encode()?))
}

/// Encode polygons into GeomData
fn encode_polygons(g: ewkb::MultiPolygon, t: &Transform) -> GeomResult {
    if g.polygons.len() == 0 {
        return Ok(None);
    }
    let mut ge = GeomEncoder::new(GeomType::Polygon, *t);
    for polygon in &g.polygons {
        // NOTE: this assumes that rings are well-formed according to MVT spec
        for ring in &polygon.rings {
            ge.complete_geom()?;
            let len = ring.points.len();
            if len > 2 {
                for p in &ring.points[..(len - 1)] {
                    ge.add_point(p.x, p.y);
                }
            }
        }
    }
    Ok(Some(ge.encode()?))
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
                for p in ld.patterns() {
                    let tag = p.tag().to_string();
                    if !tags.contains(&tag) {
                        tags.push(tag.to_string());
                    }
                }
            }
        }
        tags
    }
}

impl Builder {
    /// Create a new builder
    pub fn new() -> Self {
        let tile_extent = 4096;
        let pixels = 256;
        let buffer_pixels = 0;
        let query_limit = std::u32::MAX;
        Builder { tile_extent, pixels, buffer_pixels, query_limit }
    }
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
    pub fn build(self, table_cfgs: &[TableCfg], layer_group: &LayerGroupCfg)
        -> Result<TileMaker, Error>
    {
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
    fn build_table_defs(&self, layer_defs: &[LayerDef],
        table_cfgs: &[TableCfg]) -> Vec<TableDef>
    {
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
    /// Get the base name
    pub fn base_name(&self) -> &str {
        &self.base_name
    }
    /// Write a tile to a file
    pub fn write_tile(
        &self,
        conn: &Connection,
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
        conn: &Connection,
        tid: TileId,
        out: &mut Write,
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
        conn: &Connection,
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
    /// Fetch a tile
    fn fetch_tile(
        &self,
        conn: &Connection,
        tid: TileId,
    ) -> Result<Tile, Error> {
        let bbox = self.grid.tile_bbox(tid);
        let tile_sz =
            (bbox.x_max() - bbox.x_min()).max(bbox.y_max() - bbox.y_min());
        let pixel_sz = tile_sz / self.pixels as f64;
        debug!("tile {}, pixel_sz {:?}", tid, pixel_sz);
        let ts = self.tile_extent as f64;
        let transform = self.grid.tile_transform(tid).scale(ts, ts);
        let t = Instant::now();
        let tile =
            self.query_tile(conn, &transform, &bbox, pixel_sz, tid.z())?;
        info!(
            "tile {}, fetched {} bytes in {:?}",
            tid,
            tile.compute_size(),
            t.elapsed()
        );
        Ok(tile)
    }
    /// Check one table for matching layers
    fn check_layers(&self, table_def: &TableDef, zoom: u32) -> bool {
        let table = &table_def.name;
        self.layer_defs.iter().any(|l| l.check_table(table, zoom))
    }
    /// Query one tile from DB
    fn query_tile(
        &self,
        conn: &Connection,
        transform: &Transform,
        bbox: &BBox,
        tol: f64,
        zoom: u32,
    ) -> Result<Tile, Error> {
        let mut tile = Tile::new(self.tile_extent);
        let mut layers = self
            .layer_defs
            .iter()
            .map(|ld| tile.create_layer(&ld.name()))
            .collect();
        for table_def in &self.table_defs {
            if self.check_layers(table_def, zoom) {
                self.query_layers(
                    conn,
                    table_def,
                    &bbox,
                    &transform,
                    tol,
                    zoom,
                    &mut layers,
                )?;
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
        conn: &Connection,
        table_def: &TableDef,
        bbox: &BBox,
        transform: &Transform,
        tol: f64,
        zoom: u32,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        debug!("sql: {}", &table_def.sql);
        let stmt = conn.prepare_cached(&table_def.sql)?;
        let trans = conn.transaction()?;
        let x_min = bbox.x_min();
        let y_min = bbox.y_min();
        let x_max = bbox.x_max();
        let y_max = bbox.y_max();
        let rad = tol * self.buffer_pixels as f64;
        let params: Vec<&ToSql> =
            vec![&tol, &x_min, &y_min, &x_max, &y_max, &rad];
        debug!("params: {:?}", params);
        let row_limit = if self.query_limit < 50 {
            self.query_limit as i32
        } else {
            50
        };
        let rows = stmt.lazy_query(&trans, &params[..], row_limit)?;
        let mut i = 0;
        for row in rows.iterator() {
            self.add_layer_features(table_def, &row?, transform, zoom, layers)?;
            if i == self.query_limit {
                info!("table {}, query limit reached: {}", &table_def.name, i);
                break;
            }
            i += 1;
        }
        Ok(())
    }
    /// Add features to a layer
    fn add_layer_features(
        &self,
        table_def: &TableDef,
        row: &Row,
        transform: &Transform,
        zoom: u32,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        let table = &table_def.name;
        let geom_type = &table_def.geom_type;
        // FIXME: can this be done without a temp vec?
        let mut lyrs: Vec<Layer> = layers.drain(..).collect();
        for mut layer in lyrs.drain(..) {
            let layer_def =
                self.layer_defs.iter().find(|ld| ld.name() == layer.name());
            if let Some(layer_def) = layer_def {
                if layer_def.check_table(table, zoom) {
                    layer = layer_def.add_feature(layer, &table_def.id_column,
                        geom_type, &row, transform)?;
                }
            }
            layers.push(layer);
        }
        Ok(())
    }
}

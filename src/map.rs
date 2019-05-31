// map.rs
//
// Copyright (c) 2019  Minnesota Department of Transportation
//
use crate::Error;
use crate::config::{LayerGroupCfg, TableCfg};
use fallible_iterator::FallibleIterator;
use log::{debug, error, info, trace, warn};
use mvt::{
    BBox, Feature, GeomData, GeomEncoder, GeomType, Layer, MapGrid, Tile,
    TileId, Transform,
};
use postgis::ewkb;
use postgres::rows::Row;
use postgres::types::{FromSql, ToSql};
use postgres::Connection;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::Instant;

const TILE_EXTENT: u32 = 4096;

const ZOOM_MAX: u32 = 30;

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

/// Tag pattern specification to require matching tag
#[derive(Clone, Debug, PartialEq)]
enum MustMatch {
    No,
    Yes,
}

/// Tag pattern specification to include tag value in layer
#[derive(Clone, Debug)]
enum IncludeValue {
    No,
    Yes,
}

/// Tag pattern specification to match value equal vs. not equal
#[derive(Clone, Debug)]
enum Equality {
    Equal,
    NotEqual,
}

/// Tag pattern specification for layer rule
#[derive(Clone, Debug)]
struct TagPattern {
    must_match: MustMatch,
    include: IncludeValue,
    key: String,
    equality: Equality,
    values: Vec<String>,
}

/// Layer rule definition
#[derive(Clone, Debug)]
struct LayerDef {
    name: String,
    table: String,
    zoom_min: u32,
    zoom_max: u32,
    patterns: Vec<TagPattern>,
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
    pixels: u32,
    buffer_pixels: u32,
    query_limit: u32,
}

/// Map tile maker
#[derive(Clone)]
pub struct TileMaker {
    base_name: String,
    pixels: u32,
    buffer_pixels: u32,
    query_limit: u32,
    grid: MapGrid,
    layer_defs: Vec<LayerDef>,
    tables: Vec<TableDef>,
}

impl TagPattern {
    /// Create a new "name" tag pattern
    fn new_name() -> Self {
        let must_match = MustMatch::No;
        let include = IncludeValue::Yes;
        let key = "name".to_string();
        let equality = Equality::NotEqual;
        let values = vec!["_".to_string()];
        TagPattern {
            must_match,
            include,
            key,
            equality,
            values,
        }
    }
    /// Get the tag (key)
    fn tag(&self) -> &str {
        &self.key
    }
    /// Check if the key matches
    fn matches_key(&self, key: &str) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        self.key == key
    }
    /// Check if the value matches
    fn matches_value(&self, value: Option<String>) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        match self.equality {
            Equality::Equal => self.matches_value_option(value),
            Equality::NotEqual => !self.matches_value_option(value),
        }
    }
    /// Check if an optional value matches
    fn matches_value_option(&self, value: Option<String>) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        match value {
            Some(val) => self.values.iter().any(|v| v == &val),
            None => self.values.iter().any(|v| v == &"_"),
        }
    }
    /// Parse a tag pattern rule
    fn parse_rule(pat: &str) -> (MustMatch, IncludeValue, &str) {
        if pat.starts_with('.') {
            (MustMatch::Yes, IncludeValue::Yes, &pat[1..])
        } else if pat.starts_with('?') {
            (MustMatch::No, IncludeValue::Yes, &pat[1..])
        } else {
            (MustMatch::Yes, IncludeValue::No, pat)
        }
    }
    /// Parse the equality portion
    fn parse_equality(pat: &str) -> Option<(&str, Equality, &str)> {
        if pat.contains('=') {
            let mut kv = pat.splitn(2, '=');
            let key = kv.next()?;
            let val = kv.next()?;
            if key.ends_with('!') {
                let key = &key[..key.len() - 1];
                Some((key, Equality::NotEqual, val))
            } else {
                Some((key, Equality::Equal, val))
            }
        } else {
            Some((pat, Equality::NotEqual, &"_"))
        }
    }
    /// Parse the value(s) portion
    fn parse_values(val: &str) -> Vec<String> {
        val.split('|').map(|v| v.to_string()).collect()
    }
    /// Parse a tag pattern rule
    fn parse(pat: &str) -> Option<TagPattern> {
        let (must_match, include, pat) = TagPattern::parse_rule(pat);
        let (key, equality, values) = TagPattern::parse_equality(pat)?;
        let key = key.to_string();
        let values = TagPattern::parse_values(values);
        Some(TagPattern {
            must_match,
            include,
            key,
            equality,
            values,
        })
    }
}

/// Parse the zoom portion of a layer rule
fn parse_zoom(z: &str) -> Option<(u32, u32)> {
    if z.ends_with('+') {
        let c = z.len() - 1;
        let zoom_min = parse_u32(&z[..c])?;
        Some((zoom_min, ZOOM_MAX))
    } else if z.contains('-') {
        let mut s = z.splitn(2, '-');
        let zoom_min = parse_u32(s.next()?)?;
        let zoom_max = parse_u32(s.next()?)?;
        Some((zoom_min, zoom_max))
    } else {
        let z = parse_u32(z)?;
        Some((z, z))
    }
}

/// Parse a u32 value
fn parse_u32(v: &str) -> Option<u32> {
    match v.parse::<u32>() {
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

/// Parse tag patterns of a layer rule
fn parse_patterns(c: &mut Iterator<Item = &str>) -> Option<Vec<TagPattern>> {
    let mut patterns = Vec::<TagPattern>::new();
    loop {
        if let Some(p) = c.next() {
            let p = TagPattern::parse(p)?;
            let key = p.tag();
            if let Some(d) = patterns.iter().find(|p| p.tag() == key) {
                error!("duplicate pattern {:?}", d);
                return None;
            }
            patterns.push(p);
        } else {
            break;
        }
    }
    if patterns.len() > 0 {
        if !patterns.iter().any(|p| &p.tag() == &"name") {
            patterns.push(TagPattern::new_name());
        }
        Some(patterns)
    } else {
        None
    }
}

impl LayerDef {
    /// Parse a layer definition rule
    fn parse(c: &mut Iterator<Item = &str>) -> Option<Self> {
        let name = c.next()?.to_string();
        let table = c.next()?.to_string();
        let (zoom_min, zoom_max) = parse_zoom(c.next()?)?;
        let patterns = parse_patterns(c)?;
        Some(LayerDef {
            name,
            table,
            zoom_min,
            zoom_max,
            patterns,
        })
    }
    /// Check if zoom level matches
    fn check_zoom(&self, zoom: u32) -> bool {
        zoom >= self.zoom_min && zoom <= self.zoom_max
    }
    /// Check a table definition and zoom level
    fn check_table(&self, table: &TableDef, zoom: u32) -> bool {
        self.check_zoom(zoom) && self.table == table.name
    }
    /// Check if a row matches the layer rule
    fn matches(&self, row: &Row) -> bool {
        for pattern in &self.patterns {
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
        trace!("layer {}, fid {}", &self.name, fid);
        // NOTE: Leaflet apparently can't use mvt feature id; use tag/property
        feature.add_tag_sint(id_column, fid);
        for pattern in &self.patterns {
            if let IncludeValue::Yes = pattern.include {
                let key = pattern.tag();
                if let Some(v) = self.get_tag_value(row, key) {
                    feature.add_tag_string(key, &v);
                    trace!("layer {}, {}={}", &self.name, key, &v);
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
            if ld.table == name {
                for p in &ld.patterns {
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
        let pixels = 256;
        let buffer_pixels = 0;
        let query_limit = std::u32::MAX;
        Builder { pixels, buffer_pixels, query_limit }
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
        let layer_defs = layer_group.load_layer_defs()?;
        let tables = self.build_table_defs(&layer_defs, table_cfgs);
        let base_name = layer_group.base_name().to_string();
        let pixels = self.pixels;
        let buffer_pixels = self.buffer_pixels;
        let query_limit = self.query_limit;
        let grid = MapGrid::new_web_mercator();
        Ok(TileMaker {
            base_name,
            pixels,
            buffer_pixels,
            query_limit,
            grid,
            layer_defs,
            tables,
        })
    }
    /// Build the table definitions
    fn build_table_defs(&self, layer_defs: &[LayerDef],
        table_cfgs: &[TableCfg]) -> Vec<TableDef>
    {
        let mut tables = vec![];
        for table_cfg in table_cfgs {
            if let Some(table) = TableDef::new(table_cfg, layer_defs) {
                tables.push(table);
            }
        }
        tables
    }
}

impl LayerGroupCfg {
    /// Load layer rule definition file
    fn load_layer_defs(&self) -> Result<Vec<LayerDef>, Error> {
        let mut defs = vec![];
        let f = BufReader::new(File::open(&self.rules_path())?);
        for line in f.lines() {
            if let Some(ld) = parse_layer_def(&line?) {
                debug!("LayerDef: {:?}", &ld);
                defs.push(ld);
            }
        }
        let mut names = String::new();
        for ld in &defs {
            names.push(' ');
            names.push_str(&ld.name);
        }
        info!("{} layers loaded:{}", defs.len(), names);
        Ok(defs)
    }
}

/// Parse one layer definition
fn parse_layer_def(line: &str) -> Option<LayerDef> {
    let line = if let Some(hash) = line.find('#') {
        &line[..hash]
    } else {
        &line
    };
    let c: Vec<&str> = line.split_whitespace().collect();
    match c.len() {
        0 => None,
        1...3 => {
            error!("Invalid rule (not enough columns): {}", line);
            None
        }
        _ => {
            let ld = LayerDef::parse(&mut c.into_iter());
            if ld.is_none() {
                error!("parsing \"{}\"", line);
            }
            ld
        }
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
        let ts = TILE_EXTENT as f64;
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
    fn check_layers(&self, table: &TableDef, zoom: u32) -> bool {
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
        let mut tile = Tile::new(TILE_EXTENT);
        let mut layers = self
            .layer_defs
            .iter()
            .map(|ld| tile.create_layer(&ld.name))
            .collect();
        for table in &self.tables {
            if self.check_layers(table, zoom) {
                self.query_layers(
                    conn,
                    table,
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
        table: &TableDef,
        bbox: &BBox,
        transform: &Transform,
        tol: f64,
        zoom: u32,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        debug!("sql: {}", &table.sql);
        let stmt = conn.prepare_cached(&table.sql)?;
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
            self.add_layer_features(table, &row?, transform, zoom, layers)?;
            if i == self.query_limit {
                info!("table {}, query limit reached: {}", &table.name, i);
                break;
            }
            i += 1;
        }
        Ok(())
    }
    /// Add features to a layer
    fn add_layer_features(
        &self,
        table: &TableDef,
        row: &Row,
        transform: &Transform,
        zoom: u32,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        let geom_type = &table.geom_type;
        // FIXME: can this be done without a temp vec?
        let mut lyrs: Vec<Layer> = layers.drain(..).collect();
        for mut layer in lyrs.drain(..) {
            let layer_def =
                self.layer_defs.iter().find(|ld| ld.name == layer.name());
            if let Some(layer_def) = layer_def {
                if layer_def.check_table(table, zoom) {
                    layer = layer_def.add_feature(layer, &table.id_column,
                        geom_type, &row, transform)?;
                }
            }
            layers.push(layer);
        }
        Ok(())
    }
}

// geom.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::layer::LayerDef;
use crate::Error;
use log::{trace, warn};
use mvt::{Feature, GeomData, GeomEncoder, GeomType, Layer};
use pointy::Transform64;
use postgis::ewkb;
use postgres::types::FromSql;
use postgres::Row;

/// Geometry result
type GeomResult = Result<Option<GeomData>, Error>;

/// Encode points into GeomData
fn encode_points(g: ewkb::MultiPoint, t: &Transform64) -> GeomResult {
    if g.points.is_empty() {
        return Ok(None);
    }
    let mut ge = GeomEncoder::new(GeomType::Point, *t);
    for p in &g.points {
        ge.add_point(p.x, p.y);
    }
    Ok(Some(ge.encode()?))
}

/// Encode linestrings into GeomData
fn encode_linestrings(g: ewkb::MultiLineString, t: &Transform64) -> GeomResult {
    if g.lines.is_empty() {
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
fn encode_polygons(g: ewkb::MultiPolygon, t: &Transform64) -> GeomResult {
    if g.polygons.is_empty() {
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

/// Geometry row from a DB query
pub struct GeomRow<'a> {
    /// DB row of query
    row: &'a Row,
    /// Geometry type
    geom_type: GeomType,
    /// ID Column
    id_column: &'a str,
}

impl<'a> GeomRow<'a> {
    /// Create a new geom row
    pub fn new(row: &'a Row, geom_type: GeomType, id_column: &'a str) -> Self {
        GeomRow {
            row,
            geom_type,
            id_column,
        }
    }

    /// Check if a row matches a layer
    pub fn matches_layer(&self, layer_def: &LayerDef) -> bool {
        for pattern in layer_def.patterns() {
            if let Some(key) = pattern.match_key() {
                if !pattern.matches_value(self.get_tag_value(key)) {
                    return false;
                }
            }
        }
        true
    }

    /// Get the row ID
    pub fn get_id(&self) -> i64 {
        // id_column is always #0 (see QueryDef::build_sql)
        self.row.get::<_, i64>(0)
    }

    /// Get one tag value (string)
    fn get_tag_value(&self, col: &str) -> Option<String> {
        if let Some(v) = self.row.get::<_, Option<String>>(col) {
            if !v.is_empty() {
                return Some(v);
            }
        }
        None
    }

    /// Get geometry from a row, encoded as MVT GeomData
    pub fn get_geometry(&self, t: &Transform64) -> GeomResult {
        match self.geom_type {
            GeomType::Point => self.get_geom_data(t, &encode_points),
            GeomType::Linestring => self.get_geom_data(t, &encode_linestrings),
            GeomType::Polygon => self.get_geom_data(t, &encode_polygons),
        }
    }

    /// Get geom data from a row
    fn get_geom_data<T: FromSql<'a>>(
        &self,
        t: &Transform64,
        enc: &dyn Fn(T, &Transform64) -> GeomResult,
    ) -> GeomResult {
        // geom_column is always #1 (see QueryDef::build_sql)
        match self.row.try_get(1) {
            Ok(Some(g)) => enc(g, t),
            Ok(None) => Ok(None),
            Err(e) => Err(Error::Pg(e)),
        }
    }

    /// Add a feature to a layer
    pub fn add_feature(
        &self,
        layer: Layer,
        layer_def: &LayerDef,
        geom_data: GeomData,
    ) -> Layer {
        let mut feature = layer.into_feature(geom_data);
        self.get_tags(layer_def, &mut feature);
        feature.into_layer()
    }

    /// Get tags from a row and add them to a feature
    fn get_tags(&self, layer_def: &LayerDef, feature: &mut Feature) {
        let fid = self.get_id();
        trace!("layer {}, fid {}", layer_def.name(), fid);
        // NOTE: Leaflet apparently can't use mvt feature id; use tag/property
        feature.add_tag_sint(self.id_column, fid);
        for pattern in layer_def.patterns() {
            if let Some(key) = pattern.include_key() {
                if let Some(v) = self.get_tag_value(key) {
                    feature.add_tag_string(key, &v);
                    trace!("layer {}, {}={}", layer_def.name(), key, &v);
                }
            }
        }
    }
}

/// Lookup a geometry type from a string name
pub fn lookup_geom_type(geom_type: &str) -> Option<GeomType> {
    match geom_type {
        "polygon" => Some(GeomType::Polygon),
        "linestring" => Some(GeomType::Linestring),
        "point" => Some(GeomType::Point),
        _ => {
            warn!("unknown geom type: {}", geom_type);
            None
        }
    }
}

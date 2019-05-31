// geom.rs
//
// Copyright (c) 2019  Minnesota Department of Transportation
//
use crate::Error;
use log::warn;
use mvt::{GeomData, GeomEncoder, GeomType, Transform };
use postgis::ewkb;
use postgres::rows::Row;
use postgres::types::FromSql;

type GeomResult = Result<Option<GeomData>, Error>;

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

/// Geometry row from a DB query
pub struct GeomRow<'a> {
    row: &'a Row<'a>,
    geom_type: GeomType,
}

impl<'a> GeomRow<'a> {
    /// Create a new geom row
    pub fn new(row: &'a Row, geom_type: GeomType) -> Self {
        GeomRow { row, geom_type }
    }
    /// Get the row ID
    pub fn get_id(&self) -> i64 {
        // id_column is always #0 (see build_query_sql)
        self.row.get::<_, i64>(0)
    }
    /// Get one tag value (string)
    pub fn get_tag_value(&self, col: &str) -> Option<String> {
        if let Some(v) = self.row.get::<_, Option<String>>(col) {
            if v.len() > 0 {
                return Some(v);
            }
        }
        None
    }
    /// Get geometry from a row, encoded as MVT GeomData
    pub fn get_geometry(&self, t: &Transform) -> GeomResult {
        match self.geom_type {
            GeomType::Point => self.get_geom_data(t, &encode_points),
            GeomType::Linestring => self.get_geom_data(t, &encode_linestrings),
            GeomType::Polygon => self.get_geom_data(t, &encode_polygons),
        }
    }
    /// Get geom data from a row
    fn get_geom_data<T: FromSql>(&self, t: &Transform, enc: &Fn(T, &Transform)
        -> GeomResult) -> GeomResult
    {
        // geom_column is always #1 (see build_query_sql)
        match self.row.get_opt(1) {
            Some(Ok(Some(g))) => enc(g, t),
            Some(Err(e)) => Err(Error::Pg(e)),
            _ => Ok(None),
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
        },
    }
}

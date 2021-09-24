// geom.rs
//
// Copyright (c) 2019-2021  Minnesota Department of Transportation
//
use crate::error::{Error, Result};
use crate::layer::LayerDef;
use crate::tile::TileCfg;
use mvt::{GeomData, GeomEncoder, GeomType, Layer};
use pointy::Transform;
use rosewood::{Geometry, Linestring, Point, Polygon, RTree};
use std::path::Path;

/// Geometry which can be encoded to GeomData
pub trait GeomEncode {
    /// Encode into GeomData
    fn encode(&self, t: Transform<f32>) -> Result<GeomData>;
}

/// Tag values, in order specified by tag pattern rule
type Values = Vec<Option<String>>;

/// Tree of geometry
pub trait GeomTree {
    /// Query a tile layer
    fn query_features(
        &self,
        layer_def: &LayerDef,
        layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer>;
}

/// Tree of point geometry
struct PointTree {
    tree: RTree<f32, Point<f32, Values>>,
}

/// Tree of linestring geometry
struct LinestringTree {
    tree: RTree<f32, Linestring<f32, Values>>,
}

/// Tree of polygon geometry
struct PolygonTree {
    tree: RTree<f32, Polygon<f32, Values>>,
}

impl<D> GeomEncode for Point<f32, D> {
    fn encode(&self, t: Transform<f32>) -> Result<GeomData> {
        let mut enc = GeomEncoder::new(GeomType::Point, t);
        for pt in self.as_points() {
            enc.add_point(pt.x(), pt.y())?;
        }
        Ok(enc.encode()?)
    }
}

impl PointTree {
    /// Create a new point tree
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let tree = RTree::new(path)?;
        Ok(Self { tree })
    }
}

impl GeomTree for PointTree {
    fn query_features(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let transform = tile_cfg.transform();
        for point in self.tree.query(tile_cfg.bbox()) {
            let point = point?;
            let values = point.data();
            if layer_def.values_match(values) {
                let geom = point.encode(transform)?;
                if !geom.is_empty() {
                    let mut feature = layer.into_feature(geom);
                    layer_def.add_tags(&mut feature, values);
                    layer = feature.into_layer();
                }
            }
        }
        Ok(layer)
    }
}

impl<D> GeomEncode for Linestring<f32, D> {
    fn encode(&self, t: Transform<f32>) -> Result<GeomData> {
        let mut enc = GeomEncoder::new(GeomType::Linestring, t);
        for line in self.as_lines() {
            enc.complete_geom()?;
            for pt in line {
                enc.add_point(pt.x(), pt.y())?;
            }
        }
        Ok(enc.encode()?)
    }
}

impl LinestringTree {
    /// Create a new linestring tree
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let tree = RTree::new(path)?;
        Ok(Self { tree })
    }
}

impl GeomTree for LinestringTree {
    fn query_features(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let transform = tile_cfg.transform();
        for lines in self.tree.query(tile_cfg.bbox()) {
            let lines = lines?;
            let values = lines.data();
            if layer_def.values_match(values) {
                let geom = lines.encode(transform)?;
                if !geom.is_empty() {
                    let mut feature = layer.into_feature(geom);
                    layer_def.add_tags(&mut feature, values);
                    layer = feature.into_layer();
                }
            }
        }
        Ok(layer)
    }
}

impl<D> GeomEncode for Polygon<f32, D> {
    fn encode(&self, t: Transform<f32>) -> Result<GeomData> {
        let mut enc = GeomEncoder::new(GeomType::Polygon, t);
        for ring in self.as_rings() {
            // NOTE: this assumes that rings are well-formed
            //       according to MVT spec
            enc.complete_geom()?;
            let len = ring.len();
            if len > 2 {
                for p in &ring[..(len - 1)] {
                    enc.add_point(p.x(), p.y())?;
                }
            }
        }
        Ok(enc.encode()?)
    }
}

impl PolygonTree {
    /// Create a new polygon tree
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let tree = RTree::new(path)?;
        Ok(Self { tree })
    }
}

impl GeomTree for PolygonTree {
    fn query_features(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let transform = tile_cfg.transform();
        for polygon in self.tree.query(tile_cfg.bbox()) {
            let polygon = polygon?;
            let values = polygon.data();
            if layer_def.values_match(values) {
                let geom = polygon.encode(transform)?;
                if !geom.is_empty() {
                    let mut feature = layer.into_feature(geom);
                    layer_def.add_tags(&mut feature, values);
                    layer = feature.into_layer();
                }
            }
        }
        Ok(layer)
    }
}

/// Make an RTree
pub fn make_tree(geom_tp: &str, path: &str) -> Result<Box<dyn GeomTree>> {
    match geom_tp {
        "point" => Ok(Box::new(PointTree::new(path)?)),
        "linestring" => Ok(Box::new(LinestringTree::new(path)?)),
        "polygon" => Ok(Box::new(PolygonTree::new(path)?)),
        _ => Err(Error::UnknownGeometryType()),
    }
}

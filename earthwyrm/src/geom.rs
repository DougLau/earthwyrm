// geom.rs
//
// Copyright (c) 2019-2024  Minnesota Department of Transportation
//
use crate::error::Result;
use crate::layer::LayerDef;
use crate::tile::TileCfg;
use mvt::{Feature, GeomData, GeomEncoder, GeomType, Layer};
use pointy::{BBox, Bounded, Transform};
use rosewood::{gis, gis::Gis, RTree};
use std::path::Path;

/// Geometry which can be encoded to GeomData
trait GisEncode {
    /// Encode into GeomData
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData>;
}

/// Tag values, in order specified by tag pattern rule
pub type Values = Vec<Option<String>>;

/// Tree of point geometry
pub struct PointTree {
    tree: RTree<f64, gis::Points<f64, Values>>,
}

/// Tree of linestring geometry
pub struct LinestringTree {
    tree: RTree<f64, gis::Linestrings<f64, Values>>,
}

/// Tree of polygon geometry
pub struct PolygonTree {
    tree: RTree<f64, gis::Polygons<f64, Values>>,
}

/// Tree of geometry
pub enum GeomTree {
    Point(PointTree),
    Linestring(LinestringTree),
    Polygon(PolygonTree),
}

impl LayerDef {
    /// Add tag values to a feature
    pub fn add_tags(&self, feature: &mut Feature, values: &Values) {
        for (tag, value, sint) in self.tag_values(values) {
            log::trace!("layer {}, {}={}", self.name(), tag, value);
            if sint {
                match value.parse() {
                    Ok(val) => feature.add_tag_sint(tag, val),
                    Err(_) => log::warn!(
                        "layer {}, {} invalid sint: {}",
                        self.name(),
                        tag,
                        value,
                    ),
                }
            } else {
                feature.add_tag_string(tag, value);
            }
        }
    }
}

impl<D> GisEncode for gis::Points<f64, D> {
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData> {
        let mut enc = GeomEncoder::new(GeomType::Point).bbox(bbox).transform(t);
        for pt in self.iter() {
            if pt.bounded_by(bbox) {
                enc.add_point(pt.x, pt.y)?;
            }
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
        log::debug!("PointTree: {:?}", path.as_ref());
        let tree = RTree::new(path)?;
        Ok(Self { tree })
    }

    /// Query point features
    fn query_features(
        &self,
        layer_def: &LayerDef,
        bbox: BBox<f64>,
    ) -> Result<()> {
        for points in self.tree.query(bbox) {
            let points = points?;
            let values = points.data();
            for (tag, value, _sint) in layer_def.tag_values(values) {
                println!("{}: {tag}={value}", layer_def.name());
            }
        }
        Ok(())
    }

    /// Query points in a tile
    fn query_tile(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_tile points: {bbox:?}");
        let transform = tile_cfg.transform();
        for points in self.tree.query(bbox) {
            let points = points?;
            let geom = points.encode(bbox, transform)?;
            if !geom.is_empty() {
                let mut feature = layer.into_feature(geom);
                layer_def.add_tags(&mut feature, points.data());
                layer = feature.into_layer();
            }
        }
        Ok(layer)
    }
}

impl<D> GisEncode for gis::Linestrings<f64, D> {
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData> {
        let mut enc = GeomEncoder::new(GeomType::Linestring)
            .bbox(bbox)
            .transform(t);
        for line in self.iter() {
            let mut connected = false;
            for seg in line.segments() {
                if seg.bounded_by(bbox) {
                    if !connected {
                        enc.complete_geom()?;
                        enc.add_point(seg.p0.x, seg.p0.y)?;
                    }
                    enc.add_point(seg.p1.x, seg.p1.y)?;
                    connected = true;
                } else {
                    connected = false;
                }
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
        log::debug!("LinestringTree: {:?}", path.as_ref());
        let tree = RTree::new(path)?;
        Ok(Self { tree })
    }

    /// Query linestring features
    fn query_features(
        &self,
        layer_def: &LayerDef,
        bbox: BBox<f64>,
    ) -> Result<()> {
        for lines in self.tree.query(bbox) {
            let lines = lines?;
            if lines.bounded_by(bbox) {
                let values = lines.data();
                for (tag, value, _sint) in layer_def.tag_values(values) {
                    println!("{}: {tag}={value}", layer_def.name());
                }
            }
        }
        Ok(())
    }

    /// Query linestrings in a tile
    fn query_tile(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_tile linestrings: {bbox:?}");
        let transform = tile_cfg.transform();
        for lines in self.tree.query(bbox) {
            let lines = lines?;
            let geom = lines.encode(bbox, transform)?;
            if !geom.is_empty() {
                let mut feature = layer.into_feature(geom);
                layer_def.add_tags(&mut feature, lines.data());
                layer = feature.into_layer();
            }
        }
        Ok(layer)
    }
}

impl<D> GisEncode for gis::Polygons<f64, D> {
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData> {
        let mut enc =
            GeomEncoder::new(GeomType::Polygon).bbox(bbox).transform(t);
        for ring in self.iter() {
            // NOTE: this assumes that rings are well-formed
            //       according to MVT spec
            let mut first = true;
            for seg in ring.segments() {
                if first {
                    enc.complete_geom()?;
                    enc.add_point(seg.p0.x, seg.p0.y)?;
                    first = false;
                }
                enc.add_point(seg.p1.x, seg.p1.y)?;
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
        log::debug!("PolygonTree: {:?}", path.as_ref());
        let tree = RTree::new(path)?;
        Ok(Self { tree })
    }

    /// Query polygon features
    fn query_features(
        &self,
        layer_def: &LayerDef,
        bbox: BBox<f64>,
    ) -> Result<()> {
        for poly in self.tree.query(bbox) {
            let poly = poly?;
            if poly.bounded_by(bbox) {
                let values = poly.data();
                for (tag, value, _sint) in layer_def.tag_values(values) {
                    println!("{}: {tag}={value}", layer_def.name());
                }
            }
        }
        Ok(())
    }

    /// Query polygons in a tile
    fn query_tile(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_tile polygons: {bbox:?}");
        let transform = tile_cfg.transform();
        for polygon in self.tree.query(bbox) {
            let polygon = polygon?;
            let geom = polygon.encode(bbox, transform)?;
            if !geom.is_empty() {
                let mut feature = layer.into_feature(geom);
                layer_def.add_tags(&mut feature, polygon.data());
                layer = feature.into_layer();
            }
        }
        Ok(layer)
    }
}

impl GeomTree {
    /// Make a tree to read geometry
    pub fn new<P>(geom_tp: GeomType, path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        match geom_tp {
            GeomType::Point => Ok(GeomTree::Point(PointTree::new(path)?)),
            GeomType::Linestring => {
                Ok(GeomTree::Linestring(LinestringTree::new(path)?))
            }
            GeomType::Polygon => Ok(GeomTree::Polygon(PolygonTree::new(path)?)),
        }
    }

    /// Query geometry features
    pub fn query_features(
        &self,
        layer_def: &LayerDef,
        bbox: BBox<f64>,
    ) -> Result<()> {
        match self {
            GeomTree::Point(tree) => tree.query_features(layer_def, bbox),
            GeomTree::Linestring(tree) => tree.query_features(layer_def, bbox),
            GeomTree::Polygon(tree) => tree.query_features(layer_def, bbox),
        }
    }

    /// Query geometry in a tile
    pub fn query_tile(
        &self,
        layer_def: &LayerDef,
        layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        match self {
            GeomTree::Point(tree) => {
                tree.query_tile(layer_def, layer, tile_cfg)
            }
            GeomTree::Linestring(tree) => {
                tree.query_tile(layer_def, layer, tile_cfg)
            }
            GeomTree::Polygon(tree) => {
                tree.query_tile(layer_def, layer, tile_cfg)
            }
        }
    }
}

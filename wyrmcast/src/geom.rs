// geom.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use crate::layer::LayerDef;
use anyhow::Result;
use pointy::{BBox, Bounded};
use rosewood::{RTree, gis, gis::Gis};
use std::path::Path;

/// Geometry types
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GeomTp {
    /// Point geometry
    #[default]
    Point,
    /// Linestring geometry
    Linestring,
    /// Polygon geometry
    Polygon,
}

/// Tag values, in order specified by tag pattern rule
pub type Values = Vec<Option<String>>;

/// Tree of point geometry
pub struct PointTree {
    pub tree: RTree<f64, gis::Points<f64, Values>>,
}

/// Tree of linestring geometry
pub struct LinestringTree {
    pub tree: RTree<f64, gis::Linestrings<f64, Values>>,
}

/// Tree of polygon geometry
pub struct PolygonTree {
    pub tree: RTree<f64, gis::Polygons<f64, Values>>,
}

/// Tree of geometry
pub enum GeomTree {
    Point(PointTree),
    Linestring(LinestringTree),
    Polygon(PolygonTree),
}

impl PointTree {
    /// Create a new point tree
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        log::debug!("PointTree: {:?}", path.as_ref());
        let tree = RTree::new(path);
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
}

impl LinestringTree {
    /// Create a new linestring tree
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        log::debug!("LinestringTree: {:?}", path.as_ref());
        let tree = RTree::new(path);
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
}

impl PolygonTree {
    /// Create a new polygon tree
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        log::debug!("PolygonTree: {:?}", path.as_ref());
        let tree = RTree::new(path);
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
}

impl GeomTree {
    /// Make a tree to read geometry
    pub fn new<P>(geom_tp: GeomTp, path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        match geom_tp {
            GeomTp::Point => Ok(GeomTree::Point(PointTree::new(path)?)),
            GeomTp::Linestring => {
                Ok(GeomTree::Linestring(LinestringTree::new(path)?))
            }
            GeomTp::Polygon => Ok(GeomTree::Polygon(PolygonTree::new(path)?)),
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
}

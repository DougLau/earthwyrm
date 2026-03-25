// polygon.rs
//
// Copyright (c) 2026  Minnesota Department of Transportation
//
use crate::geom::{PolygonTree, Values};
use crate::layer::LayerDef;
use crate::tile::TileCfg;
use anyhow::Result;
use hatmil::{PathDefBuilder, svg};
use pointy::{Bounded, Pt};
use rosewood::{gis, gis::Gis};

/// Wyrm polygon layer encoder
struct PolygonEncoder {
    /// Tile config
    tile_cfg: TileCfg,
    /// Path definition builder
    builder: PathDefBuilder,
    /// Start flag
    start: bool,
}

impl PolygonTree {
    /// Query polygons in a tile
    pub fn query_wyrm<'p>(
        &self,
        layer_def: &LayerDef,
        osm: bool,
        tile_cfg: &TileCfg,
        g: &'p mut svg::G<'p>,
    ) -> Result<bool> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_wyrm polygons: {bbox:?}");
        let mut found = false;
        for polygons in self.tree.query(bbox) {
            let Ok(polygons) = polygons else {
                log::warn!("IO error {}.loam polygons", layer_def.name());
                break;
            };
            let mut enc = PolygonEncoder::new(tile_cfg);
            if enc.contains(&polygons) {
                found = true;
                enc.encode_polygons(&polygons);
                let mut path = g.path();
                for (tag, value, sint) in layer_def.tag_values(polygons.data())
                {
                    if tag == "osm_id" && sint {
                        path.class(format!("osm-{value}"));
                    } else if osm {
                        path.data_(tag, value);
                    } else {
                        path.class(layer_def.class_name(value));
                    }
                }
                path.d(String::from(enc)).close();
            }
        }
        g.close();
        Ok(found)
    }
}

impl From<PolygonEncoder> for String {
    fn from(enc: PolygonEncoder) -> Self {
        String::from(enc.builder)
    }
}

impl PolygonEncoder {
    /// Create a new polygon layer encoder
    fn new(tile_cfg: &TileCfg) -> Self {
        let mut builder = svg::Path::def_builder();
        builder.precision(0);
        PolygonEncoder {
            tile_cfg: tile_cfg.clone(),
            builder,
            start: true,
        }
    }

    /// Check if bounding box contains polygons
    fn contains(&self, polygons: &gis::Polygons<f64, Values>) -> bool {
        let bbox = self.tile_cfg.bbox();
        polygons.iter().any(|pg| pg.bounded_by(bbox))
    }

    /// Encode polygons
    fn encode_polygons(&mut self, polygons: &gis::Polygons<f64, Values>) {
        let bbox = self.tile_cfg.bbox();
        for ring in polygons.iter() {
            if ring.bounded_by(bbox) {
                self.encode_ring(ring);
            }
        }
    }

    /// Encode one ring (polygon)
    fn encode_ring(&mut self, ring: &gis::Polygon<f64>) {
        self.start = true;
        let mut chain = self.tile_cfg.point_chain();
        for pt in ring.iter() {
            chain.push_back(pt);
            while chain.len() > 2 {
                if let Some(pt) = chain.pop_front() {
                    self.add_point(pt);
                }
            }
        }
        chain.connect();
        while let Some(pt) = chain.pop_front() {
            self.add_point(pt);
        }
        if !self.start {
            self.builder.close();
        }
    }

    /// Add a point
    fn add_point(&mut self, pt: Pt<f64>) {
        let (x, y) = self.tile_cfg.xform(pt);
        if self.start {
            self.builder.move_to((x, y));
            self.start = false;
        } else {
            self.builder.line((x, y));
        }
    }
}

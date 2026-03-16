// linestring.rs
//
// Copyright (c) 2026  Minnesota Department of Transportation
//
use crate::geom::{LinestringTree, Values};
use crate::layer::LayerDef;
use crate::tile::TileCfg;
use anyhow::Result;
use hatmil::{PathDefBuilder, svg};
use pointy::{Bounded, Pt};
use rosewood::{gis, gis::Gis};

/// Wyrm linestring layer encoder
struct LinestringEncoder {
    /// Tile config
    tile_cfg: TileCfg,
    /// Path definition builder
    builder: PathDefBuilder,
    /// Start flag
    start: bool,
}

impl LinestringTree {
    /// Query linestrings in a tile
    pub fn query_wyrm<'p>(
        &self,
        layer_def: &LayerDef,
        tile_cfg: &TileCfg,
        g: &'p mut svg::G<'p>,
    ) -> Result<bool> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_wyrm linestrings: {bbox:?}");
        let mut found = false;
        for lines in self.tree.query(bbox) {
            let lines = lines?;
            let mut enc = LinestringEncoder::new(tile_cfg);
            if enc.contains(&lines) {
                found = true;
                enc.encode_linestrings(&lines);
                let mut path = g.path();
                for (tag, value, sint) in layer_def.tag_values(lines.data()) {
                    if tag == "osm_id" && sint {
                        path.class(format!("osm-{value}"));
                    } else {
                        path.data_(tag, value);
                    }
                }
                path.d(String::from(enc)).close();
            }
        }
        g.close();
        Ok(found)
    }
}

impl From<LinestringEncoder> for String {
    fn from(enc: LinestringEncoder) -> Self {
        String::from(enc.builder)
    }
}

impl LinestringEncoder {
    /// Create a new linesting layer encoder
    fn new(tile_cfg: &TileCfg) -> Self {
        let mut builder = svg::Path::def_builder();
        builder.precision(0);
        LinestringEncoder {
            tile_cfg: tile_cfg.clone(),
            builder,
            start: true,
        }
    }

    /// Check if bounding box contains lines
    fn contains(&self, lines: &gis::Linestrings<f64, Values>) -> bool {
        let bbox = self.tile_cfg.bbox();
        lines.iter().any(|ln| ln.bounded_by(bbox))
    }

    /// Encode linesstrings
    fn encode_linestrings(
        &mut self,
        linestrings: &gis::Linestrings<f64, Values>,
    ) {
        let bbox = self.tile_cfg.bbox();
        for line in linestrings.iter() {
            if line.bounded_by(bbox) {
                self.encode_linestring(line);
            }
        }
    }

    /// Encode one linestring
    fn encode_linestring(&mut self, line: &gis::Linestring<f64>) {
        self.start = true;
        let mut chain = self.tile_cfg.point_chain();
        for pt in line.iter() {
            chain.push_back(pt);
            while chain.len() > 2 {
                if let Some(pt) = chain.pop_front() {
                    self.add_point(pt);
                }
            }
        }
        while let Some(pt) = chain.pop_front() {
            self.add_point(pt);
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

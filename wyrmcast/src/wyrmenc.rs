// wyrmenc.rs
//
// Copyright (c) 2026  Minnesota Department of Transportation
//
use crate::caster::CasterDef;
use crate::geom::{GeomTree, LinestringTree, PointTree, PolygonTree, Values};
use crate::group::LayerGroupDef;
use crate::layer::{LayerDef, LayerTree};
use crate::tile::TileCfg;
use anyhow::{Result, anyhow};
use hatmil::{Page, PathDefBuilder, svg};
use pointy::{Bounded, Pt};
use rosewood::{gis, gis::Gis};
use squarepeg::Peg;
use std::time::Instant;

/// Wyrm point layer encoder
struct PointEncoder {
    /// Tile config
    tile_cfg: TileCfg,
}

/// Wyrm linestring layer encoder
struct LinestringEncoder {
    /// Tile config
    tile_cfg: TileCfg,
    /// Path definition builder
    builder: PathDefBuilder,
    /// Start flag
    start: bool,
}

/// Wyrm polygon layer encoder
struct PolygonEncoder {
    /// Tile config
    tile_cfg: TileCfg,
    /// Path definition builder
    builder: PathDefBuilder,
    /// Start flag
    start: bool,
}

impl CasterDef {
    /// Fetch one Wyrm tile.
    ///
    /// * `group_name` Name of layer group.
    /// * `peg` Peg (tile ID).
    pub fn fetch_wyrm(
        &self,
        group_name: &str,
        peg: Peg,
    ) -> Result<Option<String>> {
        for group in self.groups() {
            if group_name == group.name() {
                // FIXME: don't extend bbox for point layers
                return group.write_wyrm(self.tile_cfg(peg));
            }
        }
        Err(anyhow!("Unknown group name: {group_name}"))
    }
}

impl LayerGroupDef {
    /// Write group layers to a wyrm tile
    fn write_wyrm(&self, tile_cfg: TileCfg) -> Result<Option<String>> {
        let wyrm = self.fetch_wyrm(&tile_cfg)?;
        if !wyrm.is_empty() {
            Ok(Some(wyrm))
        } else {
            log::debug!("tile {} empty (no layers)", tile_cfg.peg());
            Ok(None)
        }
    }

    /// Fetch a tile
    fn fetch_wyrm(&self, tile_cfg: &TileCfg) -> Result<String> {
        let t = Instant::now();
        let wyrm = self.query_wyrm(tile_cfg)?;
        log::info!(
            "{}/{}, fetched {} bytes in {:.2?}",
            self.name(),
            tile_cfg.peg(),
            wyrm.len(),
            t.elapsed()
        );
        Ok(wyrm)
    }

    /// Query one wyrm from trees
    fn query_wyrm(&self, tile_cfg: &TileCfg) -> Result<String> {
        let mut found = false;
        let mut page = Page::new();
        for layer_tree in self.layers() {
            if layer_tree.query_wyrm(tile_cfg, &mut page.frag::<svg::G>())? {
                found = true;
            }
        }
        if found {
            Ok(String::from(page))
        } else {
            Ok(String::new())
        }
    }
}

impl LayerTree {
    /// Query wyrm features
    fn query_wyrm<'p>(
        &self,
        tile_cfg: &TileCfg,
        g: &'p mut svg::G<'p>,
    ) -> Result<bool> {
        if self.layer_def().check_zoom(tile_cfg.peg().z()) {
            // FIXME: add layer tags as classes
            g.data_("name", self.layer_def().name());
            self.tree().query_wyrm(self.layer_def(), tile_cfg, g)
        } else {
            Ok(false)
        }
    }
}

impl GeomTree {
    /// Query wyrm geometry in a tile
    fn query_wyrm<'p>(
        &self,
        layer_def: &LayerDef,
        tile_cfg: &TileCfg,
        g: &'p mut svg::G<'p>,
    ) -> Result<bool> {
        match self {
            Self::Point(tree) => tree.query_wyrm(layer_def, tile_cfg, g),
            Self::Linestring(tree) => tree.query_wyrm(layer_def, tile_cfg, g),
            Self::Polygon(tree) => tree.query_wyrm(layer_def, tile_cfg, g),
        }
    }
}

impl PointTree {
    /// Query points in a tile
    fn query_wyrm<'p>(
        &self,
        layer_def: &LayerDef,
        tile_cfg: &TileCfg,
        g: &'p mut svg::G<'p>,
    ) -> Result<bool> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_wyrm points: {bbox:?}");
        let enc = PointEncoder::new(tile_cfg);
        let mut found = false;
        for points in self.tree.query(bbox) {
            let points = points?;
            if enc.contains(&points) {
                found = true;
                for (_tag, _value, _sint) in layer_def.tag_values(points.data())
                {
                    // FIXME: add classes
                }
                enc.encode_points(&points, &mut g.g());
            }
        }
        Ok(found)
    }
}

impl PointEncoder {
    /// Create a new point layer encoder
    fn new(tile_cfg: &TileCfg) -> Self {
        PointEncoder {
            tile_cfg: tile_cfg.clone(),
        }
    }

    /// Check if bounding box contains points
    fn contains(&self, points: &gis::Points<f64, Values>) -> bool {
        let bbox = self.tile_cfg.bbox();
        points.iter().any(|pt| pt.bounded_by(bbox))
    }

    /// Encode points
    fn encode_points<'p>(
        &self,
        points: &gis::Points<f64, Values>,
        g: &'p mut svg::G<'p>,
    ) {
        let bbox = self.tile_cfg.bbox();
        for pt in points.iter() {
            if pt.bounded_by(bbox) {
                let (x, y) = self.tile_cfg.xform(*pt);
                // FIXME: add href attribute and rotate transform
                g.r#use().x(x).y(y).close();
            }
        }
        g.close();
    }
}

impl LinestringTree {
    /// Query linestrings in a tile
    fn query_wyrm<'p>(
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
                for (_tag, _value, _sint) in layer_def.tag_values(lines.data())
                {
                    // FIXME: add classes
                }
                enc.encode_linestrings(&lines);
                g.path().d(String::from(enc)).close();
            }
        }
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
    pub fn new(tile_cfg: &TileCfg) -> Self {
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

impl PolygonTree {
    /// Query polygons in a tile
    fn query_wyrm<'p>(
        &self,
        layer_def: &LayerDef,
        tile_cfg: &TileCfg,
        g: &'p mut svg::G<'p>,
    ) -> Result<bool> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_wyrm polygons: {bbox:?}");
        let mut found = false;
        for polygons in self.tree.query(bbox) {
            let polygons = polygons?;
            let mut enc = PolygonEncoder::new(tile_cfg);
            if enc.contains(&polygons) {
                found = true;
                for (_tag, _value, _sint) in
                    layer_def.tag_values(polygons.data())
                {
                    // FIXME: add classes
                }
                enc.encode_polygons(&polygons);
                g.path().d(String::from(enc)).close();
            }
        }
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
    pub fn new(tile_cfg: &TileCfg) -> Self {
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

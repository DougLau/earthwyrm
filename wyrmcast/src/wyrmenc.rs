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
use pointy::{BBox, Bounded, Pt, Seg, Transform};
use rosewood::{gis, gis::Gis};
use squarepeg::Peg;
use std::time::Instant;

/// Wyrm point layer encoder
struct PointEncoder {
    /// Bounding box (projected coordinates)
    bbox: BBox<f64>,
    /// Transform to Peg coordinates
    transform: Transform<f64>,
}

/// Wyrm linestring layer encoder
struct LinestringEncoder {
    /// Bounding box (projected coordinates)
    bbox: BBox<f64>,
    /// Transform to Peg coordinates
    transform: Transform<f64>,
    /// Path definition builder
    builder: PathDefBuilder,
    /// Start flag
    start: bool,
}

/// Wyrm polygon layer encoder
struct PolygonEncoder {
    /// Bounding box (projected coordinates)
    bbox: BBox<f64>,
    /// Transform to Peg coordinates
    transform: Transform<f64>,
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
        let transform = tile_cfg.transform();
        let enc = PointEncoder::new(bbox, transform);
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
    fn new(bbox: BBox<f64>, transform: Transform<f64>) -> Self {
        PointEncoder { bbox, transform }
    }

    /// Check if bounding box contains points
    fn contains(&self, points: &gis::Points<f64, Values>) -> bool {
        points.iter().any(|pt| pt.bounded_by(self.bbox))
    }

    /// Encode points
    fn encode_points<'p>(
        &self,
        points: &gis::Points<f64, Values>,
        g: &'p mut svg::G<'p>,
    ) {
        for pt in points.iter() {
            if pt.bounded_by(self.bbox) {
                let (x, y) = self.make_point(pt.x, pt.y);
                // FIXME: add href attribute and rotate transform
                g.r#use().x(x).y(y).close();
            }
        }
        g.close();
    }

    /// Make point with tile coörindates
    fn make_point(&self, x: f64, y: f64) -> (i32, i32) {
        let p = self.transform * self.bbox.clamp((x, y));
        let x = p.x.round() as i32;
        let y = p.y.round() as i32;
        (x, y)
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
        let transform = tile_cfg.transform();
        let mut found = false;
        for lines in self.tree.query(bbox) {
            let lines = lines?;
            let mut enc = LinestringEncoder::new(bbox, transform);
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
    pub fn new(bbox: BBox<f64>, transform: Transform<f64>) -> Self {
        let mut builder = svg::Path::def_builder();
        builder.precision(0);
        LinestringEncoder {
            bbox,
            transform,
            builder,
            start: true,
        }
    }

    /// Check if bounding box contains lines
    fn contains(&self, lines: &gis::Linestrings<f64, Values>) -> bool {
        lines.iter().any(|ln| ln.bounded_by(self.bbox))
    }

    /// Encode linesstrings
    fn encode_linestrings(
        &mut self,
        linestrings: &gis::Linestrings<f64, Values>,
    ) {
        for line in linestrings.iter() {
            if line.bounded_by(self.bbox) {
                self.encode_linestring(line);
            }
        }
    }

    /// Encode one linestring
    fn encode_linestring(&mut self, line: &gis::Linestring<f64>) {
        let mut prev = Vec::with_capacity(2);
        for pt in line.iter() {
            if let Some(ppt) = prev.last()
                && let Some(seg) = Seg::new(ppt, pt).clip(self.bbox)
            {
                if prev.len() == 2
                    && self.should_simplify(&prev[0], &ppt, &seg.p1)
                {
                    prev.pop();
                }
                while prev.len() > 1 {
                    self.add_point(prev.remove(0));
                }
                prev.push(seg.p1);
            } else {
                prev.clear();
                self.start = true;
                prev.push(*pt);
            }
        }
        while !prev.is_empty() {
            self.add_point(prev.remove(0));
        }
        self.start = true;
    }

    /// Check if point `p1` should be simplified
    fn should_simplify(
        &self,
        p0: &Pt<f64>,
        p1: &Pt<f64>,
        p2: &Pt<f64>,
    ) -> bool {
        let (p0x, p0y) = self.make_point(p0);
        let (p1x, p1y) = self.make_point(p1);
        let (p2x, p2y) = self.make_point(p2);
        if p0x == p1x && p1x == p2x {
            return (p0y < p1y && p1y < p2y) || (p0y > p1y && p1y > p2y);
        }
        if p0y == p1y && p1y == p2y {
            return (p0x < p1x && p1x < p2x) || (p0x > p1x && p1x > p2x);
        }
        false
    }

    /// Make point with tile coörindates
    fn make_point(&self, pt: &Pt<f64>) -> (i32, i32) {
        let p = self.transform * self.bbox.clamp(pt);
        let x = p.x.round() as i32;
        let y = p.y.round() as i32;
        (x, y)
    }

    /// Add a point
    fn add_point(&mut self, pt: Pt<f64>) {
        let (x, y) = self.make_point(&pt);
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
        let transform = tile_cfg.transform();
        let mut found = false;
        for polygons in self.tree.query(bbox) {
            let polygons = polygons?;
            let mut enc = PolygonEncoder::new(bbox, transform);
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
    pub fn new(bbox: BBox<f64>, transform: Transform<f64>) -> Self {
        let mut builder = svg::Path::def_builder();
        builder.precision(0);
        PolygonEncoder {
            bbox,
            transform,
            builder,
            start: true,
        }
    }

    /// Check if bounding box contains polygons
    fn contains(&self, polygons: &gis::Polygons<f64, Values>) -> bool {
        polygons.iter().any(|pg| pg.bounded_by(self.bbox))
    }

    /// Encode polygons
    fn encode_polygons(&mut self, polygons: &gis::Polygons<f64, Values>) {
        for ring in polygons.iter() {
            /*
            let mut prev = Vec::with_capacity(2);
            if pgon.bounded_by(self.bbox) {
                for pt in pgon.iter() {
                    if let Some(ppt) = prev.last()
                        && let Some(seg) = Seg::new(ppt, pt).clip(self.bbox)
                    {
                        if prev.len() == 2
                            && self.should_simplify(&prev[0], &prev[1], &seg.p1)
                        {
                            prev.pop();
                        }
                        while prev.len() > 1 {
                            self.add_point(prev.remove(0));
                        }
                        prev.push(seg.p1);
                    } else {
                        prev.clear();
                        self.start = true;
                        prev.push(*pt);
                    }
                }
            }
            while !prev.is_empty() {
                self.add_point(prev.remove(0));
            }*/
        }
    }
}

/*
    /// Push one point with relative coörindates
    fn push_point(&mut self, x: i32, y: i32) {
        log::trace!("push_point: {x},{y}");
        self.pt0 = self.pt1;
        let (px, py) = self.pt0.unwrap_or((0, 0));
        self.data.push(ParamInt::new(x.saturating_sub(px)).encode());
        self.data.push(ParamInt::new(y.saturating_sub(py)).encode());
        self.pt1 = Some((x, y));
        self.count += 1;
    }

    /// Pop most recent point
    fn pop_point(&mut self) {
        log::trace!("pop_point");
        self.data.pop();
        self.data.pop();
        self.pt1 = self.pt0;
        self.count -= 1;
    }

    /// Add a point
    fn add_point(&mut self, x: f64, y: f64) {
        self.add_boundary_points(x, y);
        self.add_tile_point(x, y);
    }

    /// Add one or two boundary points (if needed)
    fn add_boundary_points(&mut self, x: f64, y: f64) {
        if let Some(pxy) = self.xy_end {
            let xy = Pt::from((x, y));
            let seg = Seg::new(pxy, xy);
            if let Some(seg) = seg.clip(self.bbox) {
                if seg.p0 != pxy {
                    self.add_tile_point(seg.p0.x, seg.p0.y);
                }
                if seg.p1 != xy {
                    self.add_tile_point(seg.p1.x, seg.p1.y);
                }
            }
        }
        match self.geom_tp {
            GeomTp::Linestring | GeomTp::Polygon => {
                self.xy_end = Some(Pt::from((x, y)));
            }
            _ => (),
        }
    }

    /// Add a tile point
    fn add_tile_point(&mut self, x: f64, y: f64) {
        let pt = self.make_point(x, y);
        if let Some((px, py)) = self.pt1
            && pt.0 == px
            && pt.1 == py
        {
            log::trace!("skipping redundant point: {px},{py}");
            return;
        }
        match self.geom_tp {
            GeomTp::Point => {
                if self.count == 0 {
                    self.push_command(Command::MoveTo);
                }
            }
            GeomTp::Linestring => match self.count {
                0 => self.push_command(Command::MoveTo),
                1 => self.push_command(Command::LineTo),
                _ => (),
            },
            GeomTp::Polygon => {
                match self.count {
                    0 => self.push_command(Command::MoveTo),
                    1 => self.push_command(Command::LineTo),
                    _ => (),
                }
                if self.count >= 2 && self.should_simplify_point(pt.0, pt.1) {
                    self.pop_point();
                }
            }
        }
        self.push_point(pt.0, pt.1);
    }

    /// Make point with tile coörindates
    fn make_point(&self, x: f64, y: f64) -> (i32, i32) {
        let p = self.transform * self.bbox.clamp((x, y));
        let x = p.x.round() as i32;
        let y = p.y.round() as i32;
        (x, y)
    }

    /// Check if point should be simplified
    fn should_simplify_point(&self, x: i32, y: i32) -> bool {
        if let (Some((p0x, p0y)), Some((p1x, p1y))) = (self.pt0, self.pt1) {
            if p0x == p1x && p1x == x {
                return (p0y < p1y && p1y < y) || (p0y > p1y && p1y > y);
            }
            if p0y == p1y && p1y == y {
                return (p0x < p1x && p1x < x) || (p0x > p1x && p1x > x);
            }
        }
        false
    }

    /// Complete the current geometry (for multilinestring / multipolygon)
    fn complete_geom(&mut self) {
        match self.geom_tp {
            GeomTp::Point => {
                // early return skips geometry reset
                return;
            }
            GeomTp::Linestring => (),
            GeomTp::Polygon => {
                if self.count > 1 {
                    self.push_command(Command::ClosePath);
                }
            }
        }
        // reset linestring / polygon geometry state
        self.count = 0;
        self.xy_end = None;
        self.pt0 = None;
    }

    /// Complete the current geometry (for multilinestring / multipolygon)
    pub fn complete(mut self) -> Self {
        self.complete_geom();
        self
    }
}*/

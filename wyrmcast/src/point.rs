// point.rs
//
// Copyright (c) 2026  Minnesota Department of Transportation
//
use crate::geom::{PointTree, Values};
use crate::layer::LayerDef;
use crate::tile::TileCfg;
use anyhow::Result;
use hatmil::svg;
use pointy::Bounded;
use rosewood::{gis, gis::Gis};

/// Wyrm point layer encoder
struct PointEncoder {
    /// Tile config
    tile_cfg: TileCfg,
}

impl PointTree {
    /// Query points in a tile
    pub fn query_wyrm<'p>(
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
                let mut name = None;
                let mut rotate = 0;
                for (tag, value, _sint) in layer_def.tag_values(points.data()) {
                    if tag == "name" {
                        name = Some(String::from(value));
                    }
                    if tag == "rotate"
                        && let Ok(r) = value.parse::<i16>()
                    {
                        rotate = r;
                    }
                }
                let mut g2 = g.g();
                let marker = format!("marker-{}", layer_def.name());
                if let Some(name) = name {
                    g2.class(format!("{}-{name}", layer_def.name()));
                }
                enc.encode_points(&points, &marker, rotate, &mut g2);
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
        marker: &str,
        rotate: i16,
        g: &'p mut svg::G<'p>,
    ) {
        let bbox = self.tile_cfg.bbox();
        for pt in points.iter() {
            if pt.bounded_by(bbox) {
                let (x, y) = self.tile_cfg.xform(*pt);
                let mut u = g.r#use();
                u.href(marker);
                if rotate != 0 {
                    u.transform(format!("rotate({rotate})"));
                }
                u.x(x).y(y).close();
            }
        }
        g.close();
    }
}

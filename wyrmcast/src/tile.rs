// tile.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use pointy::{BBox, Bounded, Pt, Seg, Transform};
use squarepeg::{MapGrid, Peg};

/// Tile configuration
#[derive(Clone)]
pub struct TileCfg {
    /// Tile extent; width and height in tile units
    tile_extent: u32,
    /// Peg (tile ID)
    peg: Peg,
    /// Bounding box of tile (including edge extent)
    bbox: BBox<f64>,
    /// Transform from spatial to tile coordinates
    transform: Transform<f64>,
}

/// Point chain for checking bounds and simplification
pub struct PointChain {
    /// Tile configuration
    tile_cfg: TileCfg,
    /// Chain of points
    pts: Vec<Pt<f64>>,
}

impl TileCfg {
    /// Create a new tile config
    pub fn new(grid: &MapGrid, peg: Peg, tile_extent: u32) -> Self {
        let mut bbox = grid.bbox_peg(peg);
        // increase bounding box by edge extent
        let edge = zoom_edge(peg);
        let edge_x = edge * (bbox.x_max() - bbox.x_min());
        let edge_y = edge * (bbox.y_max() - bbox.y_min());
        bbox.extend([
            (bbox.x_min() - edge_x, bbox.y_min() - edge_y),
            (bbox.x_max() + edge_x, bbox.y_max() + edge_y),
        ]);
        let ts = f64::from(tile_extent);
        let transform = grid.transform_peg(peg).scale(ts, ts);
        TileCfg {
            tile_extent,
            peg,
            bbox,
            transform,
        }
    }

    /// Get the tile extent
    pub fn tile_extent(&self) -> u32 {
        self.tile_extent
    }

    /// Get the tile `Peg`
    pub fn peg(&self) -> Peg {
        self.peg
    }

    /// Get the bounding box (including edge extent)
    pub fn bbox(&self) -> BBox<f64> {
        self.bbox
    }

    /// Get the tile transform
    pub fn transform(&self) -> Transform<f64> {
        self.transform
    }

    /// Transform point to tile coörindates
    pub fn xform(&self, pt: Pt<f64>) -> (i32, i32) {
        let p = self.bbox.clamp(pt) * self.transform;
        let x = p.x.round() as i32;
        let y = p.y.round() as i32;
        (x, y)
    }

    /// Create a point chain for the tile
    pub fn point_chain(&self) -> PointChain {
        PointChain::new(self)
    }
}

/// Calculate edge ratio based on tile zoom
///
/// Edge must be larger for higher zoom levels to prevent corrupt polygons.
fn zoom_edge(peg: Peg) -> f64 {
    match peg.z() {
        0..=12 => 1.0 / 32.0,
        13 => 1.0 / 16.0,
        14 => 1.0 / 8.0,
        15 => 1.0 / 4.0,
        16 => 1.0 / 2.0,
        _ => 1.0,
    }
}

impl PointChain {
    /// Create a new point chain
    fn new(tile_cfg: &TileCfg) -> Self {
        PointChain {
            tile_cfg: tile_cfg.clone(),
            pts: Vec::with_capacity(4),
        }
    }

    /// Get chain length
    pub fn len(&self) -> usize {
        self.pts.len()
    }

    /// Push a point to the end of the chain
    pub fn push_back(&mut self, pt: &Pt<f64>) {
        if let Some(ppt) = self.pts.last()
            && let Some(seg) = Seg::new(ppt, pt).clip(self.tile_cfg.bbox())
        {
            // Add point on edge of bounding box
            self.pts.push(if pt.bounded_by(self.tile_cfg.bbox()) {
                seg.p0
            } else {
                seg.p1
            });
        }
        self.pts.push(*pt);
    }

    /// Pop the front point in the chain
    pub fn pop_front(&mut self) -> Option<Pt<f64>> {
        while self.pts.len() >= 2 {
            self.simplify_coincident();
        }
        while self.pts.len() >= 3 {
            self.simplify_linear();
        }
        if !self.pts.is_empty() {
            Some(self.pts.remove(0))
        } else {
            None
        }
    }

    /// Simplify coincident points (in tile coordinates)
    fn simplify_coincident(&mut self) {
        let (p0x, p0y) = self.tile_cfg.xform(self.pts[0]);
        let (p1x, p1y) = self.tile_cfg.xform(self.pts[1]);
        if (p0x == p1x) && (p0y == p1y) {
            self.pts.remove(0);
        }
    }

    /// Simplify linear points
    fn simplify_linear(&mut self) {
        if self.should_simplify_linear() {
            // remove second point
            self.pts.remove(1);
        }
    }

    /// Check if second point should be simplified (linear)
    fn should_simplify_linear(&self) -> bool {
        let (p0x, p0y) = self.tile_cfg.xform(self.pts[0]);
        let (p1x, p1y) = self.tile_cfg.xform(self.pts[1]);
        let (p2x, p2y) = self.tile_cfg.xform(self.pts[2]);
        if p0x == p1x && p1x == p2x {
            return (p0y <= p1y && p1y <= p2y) || (p0y >= p1y && p1y >= p2y);
        }
        if p0y == p1y && p1y == p2y {
            return (p0x <= p1x && p1x <= p2x) || (p0x >= p1x && p1x >= p2x);
        }
        false
    }
}

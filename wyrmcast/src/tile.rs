// tile.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use pointy::{BBox, Pt, Transform};
use squarepeg::{MapGrid, Peg};

/// Tile configuration
#[derive(Clone)]
pub struct TileCfg {
    /// Tile extent; width and height in pixels
    tile_extent: u32,

    /// Peg (tile ID)
    peg: Peg,

    /// Bounding box of tile (including edge extent)
    bbox: BBox<f64>,

    /// Transform from spatial to tile coordinates
    transform: Transform<f64>,
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

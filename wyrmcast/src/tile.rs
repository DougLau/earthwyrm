// tile.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use pointy::{BBox, Transform};
use squarepeg::Peg;

/// Tile configuration
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
    pub fn new(
        tile_extent: u32,
        peg: Peg,
        bbox: BBox<f64>,
        transform: Transform<f64>,
    ) -> Self {
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

    /// Get the tile Peg
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
}

// Copyright (C) 2026  Douglas Lau
//
use crate::fetch::Uri;
use squarepeg::Peg;
use std::collections::{HashMap, HashSet};

/// Tile Key
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TileKey {
    /// Layer group name
    group: &'static str,
    /// Tile position
    peg: Peg,
}

/// Cache of wyrm tiles
pub struct WyrmCache {
    /// Layer group names
    groups: &'static [&'static str],
    /// Tile cache (wyrm files)
    tiles: HashMap<TileKey, String>,
    /// Keys of shown tiles (on map)
    shown_tiles: HashSet<TileKey>,
}

impl TileKey {
    /// Create a new tile key
    pub fn new(group: &'static str, peg: Peg) -> Self {
        TileKey { group, peg }
    }

    /// Fetch the tile
    pub async fn fetch(self) -> String {
        let mut uri = Uri::from("/");
        uri.push(self.group);
        uri.push(&self.peg.z().to_string());
        uri.push(&self.peg.x().to_string());
        uri.push(&self.peg.y().to_string());
        uri.add_extension(".wyrm");
        match uri.get().await {
            Ok(svg) => svg,
            Err(e) => {
                log::warn!("TileKey.fetch {uri:?} {e:?}");
                String::new()
            }
        }
    }

    /// Render svg element
    fn render_svg(&self, svg: &mut String, tx: i32, ty: i32, wyrm: &str) {
        svg.push_str("<svg class=\"wyrm-tile ");
        svg.push_str(self.group);
        svg.push('-');
        svg.push_str(&self.peg.to_string());
        svg.push('"');
        if tx != 0 || ty != 0 {
            svg.push_str(" transform=\"translate(");
            svg.push_str(&(tx * 256).to_string());
            svg.push(' ');
            svg.push_str(&(ty * 256).to_string());
            svg.push_str(")\"");
        }
        svg.push('>');
        svg.push_str(wyrm);
        svg.push_str("</svg>");
    }
}

impl WyrmCache {
    /// Create a new wyrm tile cache
    pub fn new(groups: &'static [&'static str]) -> Self {
        WyrmCache {
            groups,
            tiles: HashMap::default(),
            shown_tiles: HashSet::default(),
        }
    }

    /// Get a `Vec` of missing tile keys
    pub fn missing_keys(&self, peg_nw: Peg, peg_se: Peg) -> Vec<TileKey> {
        let mut keys = Vec::with_capacity(8);
        let zoom = peg_nw.z();
        for py in peg_nw.y()..=peg_se.y() {
            for px in peg_nw.x()..=peg_se.x() {
                if let Some(peg) = Peg::new(zoom, px, py) {
                    for gr in self.groups {
                        let key = TileKey::new(gr, peg);
                        if !self.shown_tiles.contains(&key) {
                            keys.push(key);
                        }
                    }
                }
            }
        }
        keys
    }

    /// Insert a wyrm tile into the cache
    pub fn insert_tile(&mut self, key: TileKey, wyrm: String) {
        if wyrm.is_empty() {
            log::debug!("insert: empty wyrm! {key:?}");
        }
        self.tiles.insert(key, wyrm);
        self.shown_tiles.insert(key);
    }

    /// Clear shown tiles
    pub fn clear_shown_tiles(&mut self) {
        self.shown_tiles.clear();
    }

    /// Show a tile (return false if not cached)
    pub fn show_tile(&mut self, key: TileKey) -> bool {
        if self.tiles.contains_key(&key) {
            self.shown_tiles.insert(key);
            true
        } else {
            false
        }
    }

    /// Check if a range of tiles is shown
    pub fn is_range_shown(&self, peg_nw: Peg, peg_se: Peg) -> bool {
        if self.groups.is_empty() {
            return false;
        }
        let z = peg_nw.z();
        if z != peg_se.z() {
            return false;
        }
        let Some(peg_ne) = Peg::new(z, peg_se.x(), peg_nw.y()) else {
            return false;
        };
        let Some(peg_sw) = Peg::new(z, peg_nw.x(), peg_se.y()) else {
            return false;
        };
        self.is_shown(peg_nw)
            && self.is_shown(peg_se)
            && self.is_shown(peg_ne)
            && self.is_shown(peg_sw)
    }

    /// Check if a tile `Peg` is shown
    fn is_shown(&self, peg: Peg) -> bool {
        let gr = self.groups[0];
        let key = TileKey::new(gr, peg);
        self.shown_tiles.contains(&key)
    }

    /// Render full map element, with each tile as an `<svg>`
    pub fn render_map(&self, peg_nw: Peg, peg_se: Peg) -> String {
        let mut svg = String::with_capacity(256);
        let zoom = peg_nw.z();
        for gr in self.groups {
            for py in peg_nw.y()..=peg_se.y() {
                for px in peg_nw.x()..=peg_se.x() {
                    if let Some(peg) = Peg::new(zoom, px, py) {
                        let key = TileKey::new(gr, peg);
                        if let Some(wyrm) = self.tiles.get(&key) {
                            let tx = (px - peg_nw.x()) as i32;
                            let ty = (py - peg_nw.y()) as i32;
                            key.render_svg(&mut svg, tx, ty, wyrm);
                        }
                    }
                }
            }
        }
        svg
    }

    /// Render one tile as an `<svg>`
    pub fn render_svg(&self, key: TileKey, origin: Peg) -> String {
        let mut svg = String::with_capacity(256);
        if let Some(wyrm) = self.tiles.get(&key) {
            let tx = key.peg.x() as i32 - origin.x() as i32;
            let ty = key.peg.y() as i32 - origin.y() as i32;
            key.render_svg(&mut svg, tx, ty, wyrm);
        } else {
            log::debug!("render_svg: no tile!  {key:?}");
        }
        svg
    }
}

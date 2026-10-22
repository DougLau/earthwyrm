// Copyright (C) 2026  Douglas Lau
//
use crate::fetch::Uri;
use futures_util::Future;
use futures_util::stream::FuturesUnordered;
use squarepeg::Peg;
use std::collections::HashMap;

/// Tile Key
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TileKey {
    /// Layer group name
    group: &'static str,
    /// Tile position
    peg: Peg,
}

/// Cache of wyrm tiles
#[derive(Default)]
pub struct WyrmCache {
    /// Layer group names
    groups: &'static [&'static str],
    /// Tile cache (wyrm files)
    tiles: HashMap<TileKey, String>,
}

impl TileKey {
    /// Create a new tile key
    pub fn new(group: &'static str, peg: Peg) -> Self {
        TileKey {
            group,
            peg,
        }
    }

    /// Fetch the tile
    async fn fetch(self) -> (TileKey, String) {
        let mut uri = Uri::from("/");
        uri.push(self.group);
        uri.push(&self.peg.z().to_string());
        uri.push(&self.peg.x().to_string());
        uri.push(&self.peg.y().to_string());
        uri.add_extension(".wyrm");
        let wyrm = match uri.get().await {
            Ok(svg) => svg,
            Err(e) => {
                log::warn!("TileKey.fetch {uri:?} {e:?}");
                String::new()
            }
        };
        (self, wyrm)
    }

    /// Build svg element
    fn build_svg(&self, svg: &mut String, tx: i32, ty: i32) {
        svg.push_str("<svg class=\"wyrm-tile ");
        svg.push_str(&self.group);
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
    }
}

impl WyrmCache {
    /// Create a new wyrm tile cache
    pub fn new(groups: &'static [&'static str]) -> Self {
        WyrmCache {
            groups,
            tiles: HashMap::default(),
        }
    }

    /// Make a tile fetcher
    pub fn fetcher(&self, peg_nw: Peg, peg_se: Peg) ->
        FuturesUnordered<impl Future<Output = (TileKey, String)> + use<>>
    {
        let futures = FuturesUnordered::new();
        let zoom = peg_nw.z();
        for py in peg_nw.y()..=peg_se.y() {
            for px in peg_nw.x()..=peg_se.x() {
                if let Some(peg) = Peg::new(zoom, px, py) {
                    for gr in self.groups {
                        let key = TileKey::new(gr, peg);
                        futures.push(key.fetch());
                    }
                }
            }
        }
        futures
    }

    /// Insert a wyrm tile into the cache
    pub fn insert(&mut self, key: TileKey, wyrm: String) {
        self.tiles.insert(key, wyrm);
    }

    /// Build full map element, with each tile as an `<svg>`
    pub fn build(&self, peg_nw: Peg, peg_se: Peg) -> String {
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
                            key.build_svg(&mut svg, tx, ty);
                            svg.push_str(&wyrm);
                            svg.push_str("</svg>");
                        }
                    }
                }
            }
        }
        svg
    }
}

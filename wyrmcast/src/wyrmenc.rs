// wyrmenc.rs
//
// Copyright (c) 2026  Minnesota Department of Transportation
//
use crate::caster::CasterDef;
use crate::geom::GeomTree;
use crate::group::LayerGroupDef;
use crate::layer::LayerDef;
use crate::tile::TileCfg;
use anyhow::{Result, anyhow};
use hatmil::{Tree, svg};
use squarepeg::Peg;
use std::time::Instant;

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
                return group.write_wyrm(self.tile_cfg(peg, 8));
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
        let mut tree = Tree::new();
        let zoom = tile_cfg.peg().z();
        for layer_tree in self.layers() {
            let layer = layer_tree.layer_def();
            if layer.check_zoom(zoom) {
                let mut g = tree.root::<svg::G>();
                g.class(format!("wyrm-{}", layer.name()));
                if layer_tree.tree().query_wyrm(layer, tile_cfg, &mut g)? {
                    found = true;
                }
            }
        }
        if found {
            Ok(String::from(tree))
        } else {
            Ok(String::new())
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

// mvtenc.rs
//
// Copyright (c) 2019-2026  Minnesota Department of Transportation
//
use crate::caster::WyrmCastDef;
use crate::geom::{GeomTree, LinestringTree, PointTree, PolygonTree};
use crate::group::LayerGroupDef;
use crate::layer::{LayerDef, LayerTree};
use crate::tile::TileCfg;
use anyhow::{Result, anyhow};
use mvt::{Feature, GeomData, GeomEncoder, GeomType, Layer, Tile};
use pointy::{BBox, Bounded, Transform};
use rosewood::{gis, gis::Gis};
use squarepeg::Peg;
use std::io::Write;
use std::time::Instant;

/// Geometry which can be encoded to MVT GeomData
trait MvtEncode {
    /// Encode into MVT GeomData
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData>;
}

/// Tag values, in order specified by tag pattern rule
pub type Values = Vec<Option<String>>;

impl LayerDef {
    /// Add tag values to a feature
    pub fn add_tags(&self, feature: &mut Feature, values: &Values) {
        for (tag, value, sint) in self.tag_values(values) {
            log::trace!("layer {}, {}={}", self.name(), tag, value);
            if sint {
                match value.parse() {
                    Ok(val) => feature.add_tag_sint(tag, val),
                    Err(_) => log::warn!(
                        "layer {}, {} invalid sint: {}",
                        self.name(),
                        tag,
                        value,
                    ),
                }
            } else {
                feature.add_tag_string(tag, value);
            }
        }
    }
}

impl<D> MvtEncode for gis::Points<f64, D> {
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData> {
        let mut enc = GeomEncoder::new(GeomType::Point).bbox(bbox).transform(t);
        for pt in self.iter() {
            if pt.bounded_by(bbox) {
                enc.add_point(pt.x, pt.y)?;
            }
        }
        Ok(enc.encode()?)
    }
}

impl PointTree {
    /// Query points in a tile
    fn query_mvt(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_mvt points: {bbox:?}");
        let transform = tile_cfg.transform();
        for points in self.tree.query(bbox) {
            let points = points?;
            let geom = points.encode(bbox, transform)?;
            if !geom.is_empty() {
                let mut feature = layer.into_feature(geom);
                layer_def.add_tags(&mut feature, points.data());
                layer = feature.into_layer();
            }
        }
        Ok(layer)
    }
}

impl<D> MvtEncode for gis::Linestrings<f64, D> {
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData> {
        let mut enc = GeomEncoder::new(GeomType::Linestring)
            .bbox(bbox)
            .transform(t);
        for line in self.iter() {
            let mut connected = false;
            for seg in line.segments() {
                if seg.bounded_by(bbox) {
                    if !connected {
                        enc.complete_geom()?;
                        enc.add_point(seg.p0.x, seg.p0.y)?;
                    }
                    enc.add_point(seg.p1.x, seg.p1.y)?;
                    connected = true;
                } else {
                    connected = false;
                }
            }
        }
        Ok(enc.encode()?)
    }
}

impl LinestringTree {
    /// Query linestrings in a tile
    fn query_mvt(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_mvt linestrings: {bbox:?}");
        let transform = tile_cfg.transform();
        for lines in self.tree.query(bbox) {
            let lines = lines?;
            let geom = lines.encode(bbox, transform)?;
            if !geom.is_empty() {
                let mut feature = layer.into_feature(geom);
                layer_def.add_tags(&mut feature, lines.data());
                layer = feature.into_layer();
            }
        }
        Ok(layer)
    }
}

impl<D> MvtEncode for gis::Polygons<f64, D> {
    fn encode(&self, bbox: BBox<f64>, t: Transform<f64>) -> Result<GeomData> {
        let mut enc =
            GeomEncoder::new(GeomType::Polygon).bbox(bbox).transform(t);
        for ring in self.iter() {
            // NOTE: this assumes that rings are well-formed
            //       according to MVT spec
            let mut first = true;
            for seg in ring.segments() {
                if first {
                    enc.complete_geom()?;
                    enc.add_point(seg.p0.x, seg.p0.y)?;
                    first = false;
                }
                enc.add_point(seg.p1.x, seg.p1.y)?;
            }
            enc.complete_geom()?;
        }
        Ok(enc.encode()?)
    }
}

impl PolygonTree {
    /// Query polygons in a tile
    fn query_mvt(
        &self,
        layer_def: &LayerDef,
        mut layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let bbox = tile_cfg.bbox();
        log::trace!("query_mvt polygons: {bbox:?}");
        let transform = tile_cfg.transform();
        for polygon in self.tree.query(bbox) {
            let polygon = polygon?;
            let geom = polygon.encode(bbox, transform)?;
            if !geom.is_empty() {
                let mut feature = layer.into_feature(geom);
                layer_def.add_tags(&mut feature, polygon.data());
                layer = feature.into_layer();
            }
        }
        Ok(layer)
    }
}

impl GeomTree {
    /// Query geometry in a tile
    pub fn query_mvt(
        &self,
        layer_def: &LayerDef,
        layer: Layer,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        match self {
            GeomTree::Point(tree) => tree.query_mvt(layer_def, layer, tile_cfg),
            GeomTree::Linestring(tree) => {
                tree.query_mvt(layer_def, layer, tile_cfg)
            }
            GeomTree::Polygon(tree) => {
                tree.query_mvt(layer_def, layer, tile_cfg)
            }
        }
    }
}

impl WyrmCastDef {
    /// Fetch one MVT tile.
    ///
    /// * `out` Writer to write MVT data.
    /// * `group_name` Name of layer group.
    /// * `peg` Peg (tile ID).
    pub fn fetch_mvt<W: Write>(
        &self,
        out: &mut W,
        group_name: &str,
        peg: Peg,
    ) -> Result<bool> {
        for group in self.groups() {
            if group_name == group.name() {
                let tile_cfg = self.tile_config(peg);
                return group.write_mvt(out, tile_cfg);
            }
        }
        Err(anyhow!("Unknown group name: {group_name}"))
    }
}

impl LayerGroupDef {
    /// Fetch a tile
    fn fetch_mvt(&self, tile_cfg: &TileCfg) -> Result<Tile> {
        let t = Instant::now();
        let tile = self.query_mvt(tile_cfg)?;
        log::info!(
            "{}/{}, fetched {} bytes in {:.2?}",
            self.name(),
            tile_cfg.peg(),
            tile.compute_size(),
            t.elapsed()
        );
        Ok(tile)
    }

    /// Query one MVT from trees
    fn query_mvt(&self, tile_cfg: &TileCfg) -> Result<Tile> {
        let mut tile = Tile::new(tile_cfg.tile_extent());
        for layer_tree in self.layers() {
            let layer = layer_tree.query_mvt(&tile, tile_cfg)?;
            if layer.num_features() > 0 {
                tile.add_layer(layer)?;
            }
        }
        Ok(tile)
    }

    /// Write group layers to a tile
    fn write_mvt<W: Write>(
        &self,
        out: &mut W,
        tile_cfg: TileCfg,
    ) -> Result<bool> {
        let tile = self.fetch_mvt(&tile_cfg)?;
        if tile.num_layers() > 0 {
            tile.write_to(out)?;
            Ok(true)
        } else {
            log::debug!("tile {} empty (no layers)", tile_cfg.peg());
            Ok(false)
        }
    }
}

impl LayerTree {
    /// Query MVT features
    fn query_mvt(&self, tile: &Tile, tile_cfg: &TileCfg) -> Result<Layer> {
        let layer = tile.create_layer(self.layer_def().name());
        if self.layer_def().check_zoom(tile_cfg.peg().z()) {
            self.tree().query_mvt(self.layer_def(), layer, tile_cfg)
        } else {
            Ok(layer)
        }
    }
}

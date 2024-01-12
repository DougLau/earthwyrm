// osm.rs
//
// Copyright (c) 2021-2024  Minnesota Department of Transportation
//
use crate::config::WyrmCfg;
use crate::error::Result;
use crate::geom::Values;
use crate::layer::{DataSource, LayerDef};
use mvt::{GeomType, WebMercatorPos, Wgs84Pos};
use osmpbfreader::{
    Node, NodeId, OsmId, OsmObj, OsmPbfReader, Relation, Tags, Way,
};
use rosewood::{gis, gis::Gis, BulkWriter};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

/// OSM object map
type ObjMap = BTreeMap<OsmId, OsmObj>;

/// Tool to extract data from an OSM file
struct OsmExtractor {
    pbf: OsmPbfReader<File>,
}

/// Geometry layer maker
struct GeometryMaker {
    layer: LayerDef,
    objs: ObjMap,
}

impl OsmExtractor {
    /// Create a new OSM extractor
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let osm = File::open(path)?;
        let pbf = OsmPbfReader::new(osm);
        Ok(OsmExtractor { pbf })
    }

    /// Extract a objects for a map layer
    fn extract_layer(&mut self, layer: &LayerDef) -> Result<ObjMap> {
        log::debug!("extracting layer: {}", layer.name());
        Ok(self.pbf.get_objs_and_deps(|obj| layer.check_obj(obj))?)
    }
}

impl LayerDef {
    /// Check if an OSM object matches a layer's tag patterns
    fn check_obj(&self, obj: &OsmObj) -> bool {
        let tags = obj.tags();
        match self.geom_tp() {
            GeomType::Point | GeomType::Linestring => self.check_tags(tags),
            GeomType::Polygon => {
                // polygons are relations or closed ways
                (obj.is_relation() || obj.is_way()) && self.check_tags(tags)
            }
        }
    }
}

impl GeometryMaker {
    /// Create a new geometry layer maker
    fn new(layer: LayerDef, objs: ObjMap) -> Self {
        Self { layer, objs }
    }

    /// Make point geometry from a `Node`
    fn node_point(&self, node: &Node) -> Option<gis::Points<f64, Values>> {
        let values = self.tag_values(node.id.0, &node.tags);
        let mut point = gis::Points::new(values);
        for pt in self.lookup_nodes(&[node.id]) {
            point.push(pt);
        }
        log::debug!("added point ({:?})", point.data());
        Some(point)
    }

    /// Make linestring geometry from a `Way`
    fn way_linestring(
        &self,
        way: &Way,
    ) -> Option<gis::Linestrings<f64, Values>> {
        let values = self.tag_values(way.id.0, &way.tags);
        let mut linestring = gis::Linestrings::new(values);
        if way.nodes.is_empty() {
            log::warn!("no nodes ({:?})", linestring.data());
            return None;
        }
        let (w0, w1) = end_points(&way.nodes);
        log::trace!("way {:?} .. {:?}", w0.0, w1.0);
        let len = way.nodes.len();
        linestring.push(self.lookup_nodes(&way.nodes));
        log::debug!("added way with {len} nodes ({:?})", linestring.data());
        Some(linestring)
    }

    /// Make polygon geometry from a `Relation`
    fn rel_polygon(
        &self,
        rel: &Relation,
    ) -> Option<gis::Polygons<f64, Values>> {
        let values = self.tag_values(rel.id.0, &rel.tags);
        let mut ways = Vec::new();
        let mut polygon = gis::Polygons::new(values);
        for rf in &rel.refs {
            let outer = if rf.role == "outer" {
                true
            } else if rf.role == "inner" {
                false
            } else {
                continue;
            };
            let nodes = self.way_nodes(rf.member);
            if nodes.is_empty() {
                // relations on edges of dump area
                // can have empty member ways
                continue;
            }
            let (w0, w1) = end_points(&nodes);
            log::trace!(
                "{:?} way {:?} .. {:?} ({})",
                rf.role,
                w0.0,
                w1.0,
                ways.len()
            );
            ways.push(nodes);
            while ways.len() > 1 {
                if !connect_ways(&mut ways) {
                    break;
                }
            }
            while let Some(ring) = find_ring(&mut ways) {
                let len = ring.len();
                let pts = self.lookup_nodes(&ring);
                if outer {
                    polygon.push_outer(pts);
                } else {
                    polygon.push_inner(pts);
                }
                log::debug!(
                    "added {:?} way with {} nodes ({:?})",
                    rf.role,
                    len,
                    polygon.data(),
                );
            }
        }
        if ways.is_empty() {
            Some(polygon)
        } else {
            log::debug!("broken polygon ({:?})", polygon.data());
            None
        }
    }

    /// Make polygon geometry from a `Way`
    fn way_polygon(&self, way: &Way) -> Option<gis::Polygons<f64, Values>> {
        if way.is_open() || way.nodes.is_empty() {
            return None;
        }
        let (w0, w1) = end_points(&way.nodes);
        if w0 != w1 {
            log::trace!("way {} not closed {} .. {}", way.id.0, w0.0, w1.0);
            return None;
        }
        let values = self.tag_values(way.id.0, &way.tags);
        let len = way.nodes.len();
        let pts = self.lookup_nodes(&way.nodes);
        let mut polygon = gis::Polygons::new(values);
        polygon.push_outer(pts);
        log::debug!("added way with {len} nodes ({:?})", polygon.data());
        Some(polygon)
    }

    /// Get the member way nodes for a relation
    fn way_nodes(&self, id: OsmId) -> Vec<NodeId> {
        if let Some(member) = self.objs.get(&id) {
            if let Some(way) = member.way() {
                if way.nodes.len() > 1 {
                    return way.nodes.clone();
                }
            }
        }
        Vec::new()
    }

    /// Lookup points for a slice of nodes
    fn lookup_nodes(&self, nodes: &[NodeId]) -> Vec<(f64, f64)> {
        let mut pts = Vec::with_capacity(nodes.len());
        for node in nodes {
            let nid = OsmId::Node(*node);
            if let Some(OsmObj::Node(node)) = self.objs.get(&nid) {
                let pos = Wgs84Pos::new(node.lat(), node.lon());
                let pos = WebMercatorPos::from(pos);
                pts.push((pos.x, pos.y));
            } else {
                log::error!("node not found: {:?}", node);
                return Vec::new();
            }
        }
        pts
    }

    /// Get values for included tags
    fn tag_values(&self, id: i64, tags: &Tags) -> Values {
        self.layer
            .tags()
            .map(|tag| {
                (tag == "osm_id")
                    .then(|| id.to_string())
                    .or_else(|| tags.get(tag).map(|v| v.to_string()))
            })
            .collect()
    }

    /// Make all points for a layer
    fn make_points<P>(&self, loam: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let mut writer = BulkWriter::new(loam)?;
        let mut n_point = 0;
        for node in self.objs.iter().filter_map(|(_, obj)| obj.node()) {
            if let Some(geom) = self.node_point(node) {
                writer.push(&geom)?;
                n_point += 1;
            }
        }
        println!("  layer: {} ({n_point} points)", self.layer.name());
        if n_point > 0 {
            writer.finish()?;
        } else {
            writer.cancel()?;
        }
        Ok(())
    }

    /// Make all linestrings for a layer
    fn make_linestrings<P>(&self, loam: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let mut writer = BulkWriter::new(loam)?;
        let mut n_line = 0;
        for way in self.objs.iter().filter_map(|(_, obj)| obj.way()) {
            if let Some(geom) = self.way_linestring(way) {
                writer.push(&geom)?;
                n_line += 1;
            }
        }
        println!("  layer: {} ({n_line} linestrings)", self.layer.name());
        if n_line > 0 {
            writer.finish()?;
        } else {
            writer.cancel()?;
        }
        Ok(())
    }

    /// Make all polygons for a layer
    fn make_polygons<P>(&self, loam: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let mut writer = BulkWriter::new(loam)?;
        let mut n_poly = 0;
        for (_id, obj) in self.objs.iter() {
            if let Some(rel) = obj.relation() {
                // NOTE: check tags again because relations are nebulous
                if self.layer.check_tags(&rel.tags) {
                    if let Some(geom) = self.rel_polygon(rel) {
                        writer.push(&geom)?;
                        n_poly += 1;
                    }
                }
            }
            if let Some(way) = obj.way() {
                if let Some(geom) = self.way_polygon(way) {
                    writer.push(&geom)?;
                    n_poly += 1;
                }
            }
        }
        println!("  layer: {} ({n_poly} polygons)", self.layer.name());
        if n_poly > 0 {
            writer.finish()?;
        } else {
            writer.cancel()?;
        }
        Ok(())
    }

    /// Make all geometry for a layer
    fn make_geometry<P>(&self, loam: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        match self.layer.geom_tp() {
            GeomType::Point => self.make_points(loam),
            GeomType::Linestring => self.make_linestrings(loam),
            GeomType::Polygon => self.make_polygons(loam),
        }
    }
}

/// Connect ways on matching node Ids
fn connect_ways(ways: &mut Vec<Vec<NodeId>>) -> bool {
    let len = ways.len();
    for i in 0..len - 1 {
        let (a0, a1) = end_points(&ways[i]);
        for j in i + 1..len {
            let (b0, b1) = end_points(&ways[j]);
            if a0 == b0 || a0 == b1 || a1 == b0 || a1 == b1 {
                let mut way = ways.swap_remove(j);
                // Do not reverse way `a` if both ends connect
                if a1 != b0 && a1 != b1 {
                    log::trace!("reversed {:?} <-> {:?}", a1.0, a0.0);
                    ways[i].reverse();
                }
                let (_a0, a1) = end_points(&ways[i]);
                if b1 == a1 {
                    log::trace!("reversed {:?} <-> {:?}", b1.0, b0.0);
                    way.reverse();
                }
                let (b0, _b1) = end_points(&way);
                assert_eq!(a1, b0);
                ways[i].pop();
                ways[i].extend(way);
                log::debug!("connected @ {:?}", a1.0);
                return true;
            }
        }
    }
    false
}

/// Find a ring in a `Vec` of ways
fn find_ring(ways: &mut Vec<Vec<NodeId>>) -> Option<Vec<NodeId>> {
    let len = ways.len();
    for i in 0..len {
        let (w0, w1) = end_points(&ways[i]);
        if w0 == w1 {
            return Some(ways.swap_remove(i));
        }
    }
    None
}

/// Get the end point nodes of a way
fn end_points(way: &[NodeId]) -> (NodeId, NodeId) {
    assert!(way.len() > 1);
    let len = way.len() - 1;
    (way[0], way[len])
}

impl WyrmCfg {
    /// Extract the `osm` layer group, creating a loam file for each layer
    pub fn extract_osm<P>(&self, osm: P) -> Result<()>
    where
        P: AsRef<Path> + std::fmt::Debug,
    {
        let mut extractor = OsmExtractor::new(&osm)?;
        println!("Extracting layers from {:?}", osm);
        for group in &self.layer_group {
            for layer in &group.layer {
                let layer = LayerDef::try_from(layer)?;
                if layer.source() == DataSource::Osm {
                    let objs = extractor.extract_layer(&layer)?;
                    let loam = self.loam_path(layer.name());
                    let maker = GeometryMaker::new(layer, objs);
                    maker.make_geometry(loam)?;
                }
            }
        }
        Ok(())
    }
}

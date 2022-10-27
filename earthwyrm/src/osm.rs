// osm.rs
//
// Copyright (c) 2021-2022  Minnesota Department of Transportation
//
use crate::config::WyrmCfg;
use crate::error::Result;
use crate::geom::Values;
use crate::layer::LayerDef;
use mvt::GeomType;
use osmpbfreader::{NodeId, OsmId, OsmObj, OsmPbfReader, Relation, Tags, Way};
use rosewood::{BulkWriter, Geometry, Linestring, Polygon};
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
            GeomType::Polygon => obj.is_relation() && self.check_tags(tags),
        }
    }
}

impl GeometryMaker {
    /// Create a new geometry layer maker
    fn new(layer: LayerDef, objs: ObjMap) -> Self {
        Self { layer, objs }
    }

    /// Make a linestring from a `Way`
    fn make_linestring(&self, way: &Way) -> Option<Linestring<f32, Values>> {
        let values = self.tag_values(way.id.0, &way.tags);
        let mut linestring = Linestring::new(values);
        if way.nodes.is_empty() {
            log::warn!("no nodes ({:?})", linestring.data());
            return None;
        }
        let nodes = way.nodes.clone();
        let (w0, w1) = end_points(&nodes);
        log::trace!("way {:?} .. {:?}", w0.0, w1.0);
        let len = nodes.len();
        linestring.push(self.lookup_nodes(&nodes));
        log::debug!("added way with {len} nodes ({:?})", linestring.data());
        Some(linestring)
    }

    /// Make a polygon from a `Relation`
    fn make_polygon(&self, rel: &Relation) -> Option<Polygon<f32, Values>> {
        let values = self.tag_values(rel.id.0, &rel.tags);
        let mut ways = vec![];
        let mut polygon = Polygon::new(values);
        for rf in &rel.refs {
            let outer = if rf.role == "outer" {
                true
            } else if rf.role == "inner" {
                false
            } else {
                continue;
            };
            let nodes = self.way_nodes(rf.member);
            if !nodes.is_empty() {
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
            } else {
                log::warn!("no nodes ({:?})", polygon.data());
                return None;
            }
        }
        if ways.is_empty() {
            Some(polygon)
        } else {
            log::warn!("not all ways connected ({:?})", polygon.data());
            None
        }
    }

    /// Get the nodes for a way
    fn way_nodes(&self, id: OsmId) -> Vec<NodeId> {
        if let Some(member) = self.objs.get(&id) {
            if let Some(way) = member.way() {
                if way.nodes.len() > 1 {
                    return way.nodes.clone();
                }
            }
        }
        log::debug!("invalid way: {:?}", id);
        vec![]
    }

    /// Lookup points for a slice of nodes
    fn lookup_nodes(&self, nodes: &[NodeId]) -> Vec<(f32, f32)> {
        let mut pts = vec![];
        for node in nodes {
            let nid = OsmId::Node(*node);
            if let Some(OsmObj::Node(node)) = self.objs.get(&nid) {
                pts.push((node.lon() as f32, node.lat() as f32));
            } else {
                log::error!("node not found: {:?}", node);
                return vec![];
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

    /// Make all linestrings for a layer
    fn make_linestrings<P>(&self, loam: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let mut writer = BulkWriter::new(loam)?;
        let mut n_line = 0;
        for rel in self.objs.iter().filter_map(|(_, obj)| obj.way()) {
            if let Some(line) = self.make_linestring(rel) {
                writer.push(&line)?;
                n_line += 1;
            }
        }
        log::info!("{} layer ({n_line} linestrings)", self.layer.name());
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
        for rel in self.objs.iter().filter_map(|(_, obj)| obj.relation()) {
            if let Some(poly) = self.make_polygon(rel) {
                writer.push(&poly)?;
                n_poly += 1;
            }
        }
        log::info!("{} layer ({n_poly} polygons)", self.layer.name());
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
            GeomType::Point => todo!(),
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
        P: AsRef<Path>,
    {
        let mut extractor = OsmExtractor::new(osm)?;
        for group in &self.layer_group {
            if group.name == "osm" {
                for layer in &group.layer {
                    let layer = LayerDef::try_from(layer)?;
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

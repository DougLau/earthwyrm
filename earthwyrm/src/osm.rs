// osm.rs
//
// Copyright (c) 2021-2022  Minnesota Department of Transportation
//
use crate::config::{LayerCfg, WyrmCfg};
use crate::error::Result;
use crate::geom::Values;
use crate::layer::LayerDef;
use osmpbfreader::{NodeId, OsmId, OsmObj, OsmPbfReader, Relation, Tags};
use rosewood::{BulkWriter, Geometry, Polygon};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

/// Check if an OSM object matches a layer's tag patterns
fn check_tags(layer: &LayerDef, obj: &OsmObj) -> bool {
    obj.is_relation() && layer.check_tags(obj.tags())
}

/// Tool to extract data from an OSM file
struct OsmExtractor {
    pbf: OsmPbfReader<File>,
}

/// Polygon layer maker
struct PolygonMaker {
    layer: LayerDef,
    objs: BTreeMap<OsmId, OsmObj>,
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

    /// Extract a map layer
    fn extract_layer<P>(&mut self, loam_dir: P, layer: &LayerCfg) -> Result<()>
    where
        P: AsRef<Path>,
    {
        log::info!("extracting layer: {}", &layer.name);
        let loam = format!("{}.loam", layer.name);
        let loam = Path::new(loam_dir.as_ref().as_os_str()).join(loam);
        let layer = LayerDef::try_from(layer)?;
        let objs = self.pbf.get_objs_and_deps(|o| check_tags(&layer, o))?;
        let maker = PolygonMaker::new(layer, objs);
        maker.make_polygons(loam)?;
        Ok(())
    }
}

impl PolygonMaker {
    /// Create a new polygon layer maker
    fn new(layer: LayerDef, objs: BTreeMap<OsmId, OsmObj>) -> Self {
        Self { layer, objs }
    }

    /// Make a polygon from a `Relation`
    fn make_polygon(&self, rel: &Relation) -> Option<Polygon<f32, Values>> {
        let values = self.tag_values(&rel.tags)?;
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
    fn tag_values(&self, tags: &Tags) -> Option<Values> {
        let name = tags.get("name")?;
        // FIXME: add all tags
        let values = vec![Some(name.to_string())];
        Some(values)
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
        log::info!("polygons: {}", n_poly);
        if n_poly > 0 {
            writer.finish()?;
        }
        Ok(())
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
        let loam_dir = &self.base_dir.join("loam");
        for group in &self.layer_group {
            if group.name == "osm" {
                for layer in &group.layer {
                    extractor.extract_layer(loam_dir, layer)?;
                }
            }
        }
        Ok(())
    }
}

// osm.rs
//
// Copyright (c) 2021  Minnesota Department of Transportation
//
use crate::error::Error;
use osmpbfreader::{NodeId, OsmId, OsmObj, OsmPbfReader, Ref};
use rosewood::{BulkWriter, Polygon};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;

const LOAM: &str = &"cities.loam";

fn is_county(obj: &OsmObj) -> bool {
    obj.is_relation()
        && obj.tags().contains("type", "boundary")
        && obj.tags().contains("boundary", "administrative")
        && obj.tags().contains("admin_level", "8")
}

/// Polygon maker
struct PolyMaker {
    objs: BTreeMap<OsmId, OsmObj>,
}

impl PolyMaker {
    /// Create a new polygon maker
    fn new(objs: BTreeMap<OsmId, OsmObj>) -> Self {
        Self { objs }
    }

    /// Make a polygon from a slice of `Ref`s
    fn make_polygon(
        &mut self,
        name: &str,
        refs: &[Ref],
    ) -> Option<Polygon<f32, String>> {
        let mut ways = vec![];
        let mut polygon = Polygon::new(name.to_string());
        for rf in refs {
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
                        "added {:?} way with {} nodes ({})",
                        rf.role,
                        len,
                        name
                    );
                }
            } else {
                return None;
            }
        }
        if ways.is_empty() {
            Some(polygon)
        } else {
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

    /// Make polygons for a layer
    fn make_polygons(&mut self) -> Result<(), Error> {
        let mut writer = BulkWriter::new(LOAM)?;
        let mut n_poly = 0;
        let relations: Vec<_> = self
            .objs
            .iter()
            .filter_map(|(_, obj)| obj.relation())
            .map(|rel| rel.clone())
            .collect();
        for rel in relations {
            if let Some(name) = rel.tags.get("name") {
                match self.make_polygon(name, &rel.refs) {
                    Some(poly) => {
                        writer.push(&poly)?;
                        n_poly += 1;
                    }
                    None => log::warn!("invalid polygon ({})", name),
                }
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

/// Make one map layer
pub fn make_layer(name: OsString) -> Result<(), Error> {
    let osm = File::open(name)?;
    let mut pbf = OsmPbfReader::new(osm);
    let objs = pbf.get_objs_and_deps(is_county)?;
    let mut maker = PolyMaker::new(objs);
    maker.make_polygons()?;
    Ok(())
}

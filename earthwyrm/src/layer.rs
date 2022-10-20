// layer.rs
//
// Copyright (c) 2019-2022  Minnesota Department of Transportation
//
use crate::config::LayerGroupCfg;
use crate::error::{Error, Result};
use crate::geom::{make_tree, GeomTree};
use crate::tile::TileCfg;
use mvt::{Feature, Layer, Tile};

/// Max zoom level
const ZOOM_MAX: u32 = 30;

/// Tag pattern specification to require matching tag
#[derive(Copy, Clone, Debug, PartialEq)]
enum MustMatch {
    /// Pattern does not require match
    No,

    /// Pattern must match
    Yes,
}

/// Tag pattern specification to include tag value in layer
#[derive(Copy, Clone, Debug)]
enum IncludeValue {
    /// Do not include tag value in layer
    No,

    /// Include tag value in layer
    Yes,
}

/// Tag pattern specification for MVT feature type
#[derive(Copy, Clone, Debug)]
enum FeatureType {
    /// MVT string type
    MvtString,

    /// MVT sint type
    MvtSint,
}

/// Tag pattern specification to match value equal vs. not equal
#[derive(Copy, Clone, Debug)]
enum Equality {
    /// Pattern equals value
    Equal,

    /// Pattern not equal value
    NotEqual,
}

/// Tag pattern specification for layer rule
#[derive(Clone, Debug)]
struct TagPattern {
    /// Pattern must match (yes / no)
    must_match: MustMatch,

    /// Should tag be included in layer
    include: IncludeValue,

    /// MVT feature type
    feature_type: FeatureType,

    /// Tag name
    tag: String,

    /// Pattern equality
    equality: Equality,

    /// Pattern values
    values: Vec<String>,
}

/// Layer rule definition
pub struct LayerDef {
    /// Layer name
    name: String,

    /// R-Tree of geometry
    tree: Box<dyn GeomTree>,

    /// Minimum zoom level
    zoom_min: u32,

    /// Maximum zoom level
    zoom_max: u32,

    /// Tag patterns
    patterns: Vec<TagPattern>,
}

impl TagPattern {
    /// Get the tag
    fn tag(&self) -> &str {
        &self.tag
    }

    /// Get tag for match patterns only
    fn match_tag(&self) -> Option<&str> {
        match self.must_match {
            MustMatch::Yes => Some(self.tag()),
            MustMatch::No => None,
        }
    }

    /// Get tag for include patterns only
    fn include_tag(&self) -> Option<&str> {
        match self.include {
            IncludeValue::Yes => Some(self.tag()),
            IncludeValue::No => None,
        }
    }

    /// Check if the value matches
    fn matches_value(&self, value: &Option<String>) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        match self.equality {
            Equality::Equal => self.matches_value_option(value),
            Equality::NotEqual => !self.matches_value_option(value),
        }
    }

    /// Check if an optional value matches
    fn matches_value_option(&self, value: &Option<String>) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        match value {
            Some(val) => self.values.iter().any(|v| v == val),
            None => self.values.iter().any(|v| v == "_"),
        }
    }

    /// Parse a tag pattern rule
    fn parse_rule(pat: &str) -> (MustMatch, IncludeValue, FeatureType, &str) {
        if let Some(pat) = pat.strip_prefix('.') {
            (MustMatch::Yes, IncludeValue::Yes, FeatureType::MvtString, pat)
        } else if let Some(pat) = pat.strip_prefix('?') {
            (MustMatch::No, IncludeValue::Yes, FeatureType::MvtString, pat)
        } else if let Some(pat) = pat.strip_prefix('$') {
            (MustMatch::No, IncludeValue::Yes, FeatureType::MvtSint, pat)
        } else {
            (MustMatch::Yes, IncludeValue::No, FeatureType::MvtString, pat)
        }
    }

    /// Parse the equality portion
    fn parse_equality(pat: &str) -> (&str, Equality, &str) {
        match pat.split_once('=') {
            Some((tag, values)) => match tag.strip_suffix('!') {
                Some(tag) => (tag, Equality::NotEqual, values),
                None => (tag, Equality::Equal, values),
            },
            None => (pat, Equality::NotEqual, "_"),
        }
    }

    /// Parse the value(s) portion
    fn parse_values(values: &str) -> Vec<String> {
        values.split('|').map(|v| v.to_string()).collect()
    }

    /// Parse a tag pattern rule
    fn parse(pat: &str) -> Self {
        let (must_match, include, feature_type, pat) =
            TagPattern::parse_rule(pat);
        let (tag, equality, values) = TagPattern::parse_equality(pat);
        let tag = tag.to_string();
        let values = TagPattern::parse_values(values);
        TagPattern {
            must_match,
            include,
            feature_type,
            tag,
            equality,
            values,
        }
    }
}

/// Parse the zoom portion of a layer rule
fn parse_zoom_range(z: &str) -> Result<(u32, u32)> {
    if let Some((a, b)) = z.split_once('-') {
        let zoom_min = parse_zoom(a)?;
        let zoom_max = parse_zoom(b)?;
        Ok((zoom_min, zoom_max))
    } else if let Some(z) = z.strip_suffix('+') {
        let zoom_min = parse_zoom(z)?;
        Ok((zoom_min, ZOOM_MAX))
    } else {
        let zoom = parse_zoom(z)?;
        Ok((zoom, zoom))
    }
}

/// Parse a zoom level
fn parse_zoom(zoom: &str) -> Result<u32> {
    let zoom = zoom.parse()?;
    if zoom <= ZOOM_MAX {
        Ok(zoom)
    } else {
        Err(Error::InvalidZoomLevel(zoom))
    }
}

/// Parse tag patterns of a layer rule
fn parse_patterns(tags: &[String]) -> Result<Vec<TagPattern>> {
    let mut patterns = Vec::<TagPattern>::new();
    for pat in tags {
        let p = TagPattern::parse(pat);
        let tag = p.tag();
        if patterns.iter().any(|p| p.tag() == tag) {
            return Err(Error::DuplicatePattern(pat.to_string()));
        }
        log::debug!("tag pattern: {:?}", &p);
        patterns.push(p);
    }
    Ok(patterns)
}

impl LayerDef {
    /// Convert layer group config to layer defs
    pub fn from_group_cfg(group_cfg: &LayerGroupCfg) -> Result<Vec<Self>> {
        let mut layers = vec![];
        for layer in &group_cfg.layer {
            let layer_def = LayerDef::new(
                &layer.name,
                &layer.geom_type,
                &layer.zoom,
                &layer.tags[..],
            )?;
            layers.push(layer_def);
        }
        Ok(layers)
    }

    /// Create a new layer definition
    fn new(
        name: &str,
        geom_tp: &str,
        zoom: &str,
        patterns: &[String],
    ) -> Result<Self> {
        let name = name.to_string();
        let tree = make_tree(geom_tp, "file.loam")?;
        let (zoom_min, zoom_max) = parse_zoom_range(zoom)?;
        log::debug!("zoom: {}-{}", zoom_min, zoom_max);
        let patterns = parse_patterns(patterns)?;
        Ok(LayerDef {
            name,
            tree,
            zoom_min,
            zoom_max,
            patterns,
        })
    }

    /// Get a slice of tag patterns
    fn patterns(&self) -> &[TagPattern] {
        &self.patterns
    }

    /// Check if zoom level matches
    fn check_zoom(&self, zoom: u32) -> bool {
        zoom >= self.zoom_min && zoom <= self.zoom_max
    }

    /// Query layer features
    pub fn query_features(
        &self,
        tile: &Tile,
        tile_cfg: &TileCfg,
    ) -> Result<Layer> {
        let layer = tile.create_layer(&self.name);
        if self.check_zoom(tile_cfg.zoom()) {
            self.tree.query_features(self, layer, tile_cfg)
        } else {
            Ok(layer)
        }
    }

    /// Check if tag values match
    pub fn values_match(&self, values: &[Option<String>]) -> bool {
        for (idx, pattern) in self.patterns().iter().enumerate() {
            if let Some(_tag) = pattern.match_tag() {
                match values.get(idx) {
                    Some(val) => {
                        if !pattern.matches_value(val) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
        true
    }

    /// Add tag values to a feature
    pub fn add_tags(&self, feature: &mut Feature, values: &[Option<String>]) {
        for (idx, pattern) in self.patterns().iter().enumerate() {
            if let (Some(tag), Some(Some(val))) =
                (pattern.include_tag(), values.get(idx))
            {
                log::trace!("layer {}, {}={}", self.name, tag, val);
                match pattern.feature_type {
                    FeatureType::MvtString => feature.add_tag_string(tag, val),
                    FeatureType::MvtSint => match val.parse() {
                        Ok(sint) => feature.add_tag_sint(tag, sint),
                        Err(_) => log::warn!(
                            "layer {}, {} invalid sint: {}",
                            self.name,
                            tag,
                            val,
                        ),
                    },
                }
            }
        }
    }
}

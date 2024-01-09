// layer.rs
//
// Copyright (c) 2019-2023  Minnesota Department of Transportation
//
use crate::config::LayerCfg;
use crate::error::{Error, Result};
use mvt::GeomType;
use osmpbfreader::Tags;
use std::fmt;

/// Max zoom level
const ZOOM_MAX: u32 = 30;

/// Layer rule definition
#[derive(Debug)]
pub struct LayerDef {
    /// Layer name
    name: String,

    /// Data source
    source: DataSource,

    /// Geometry type
    geom_tp: GeomType,

    /// Minimum zoom level
    zoom_min: u32,

    /// Maximum zoom level
    zoom_max: u32,

    /// Tag patterns
    patterns: Vec<TagPattern>,
}

/// Data source for layers
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataSource {
    /// Open street map
    Osm,
    /// Json data source
    Json,
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

/// Tag pattern specification to require matching tag
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MustMatch {
    /// Pattern does not require match
    No,

    /// Pattern must match
    Yes,
}

/// Tag pattern specification to include tag value in layer
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IncludeValue {
    /// Do not include tag value in layer
    No,

    /// Include tag value in layer
    Yes,
}

/// Tag pattern specification for MVT feature type
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FeatureType {
    /// MVT string type
    MvtString,

    /// MVT sint type
    MvtSint,
}

/// Tag pattern specification to match value equal vs. not equal
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Equality {
    /// Pattern equals value
    Equal,

    /// Pattern not equal value
    NotEqual,
}

impl fmt::Display for TagPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = match (self.must_match, self.include, self.feature_type) {
            (MustMatch::No, _, FeatureType::MvtSint) => "$",
            (MustMatch::No, _, FeatureType::MvtString) => "?",
            (MustMatch::Yes, IncludeValue::Yes, _) => ".",
            _ => "",
        };
        write!(f, "{prefix}{}", &self.tag)?;
        if let (Equality::NotEqual, Some("_")) =
            (self.equality, self.values.first().map(String::as_str))
        {
            return Ok(());
        }
        let equality = match self.equality {
            Equality::Equal => "=",
            Equality::NotEqual => "!=",
        };
        write!(f, "{equality}")?;
        for (i, val) in self.values.iter().enumerate() {
            if i > 0 {
                write!(f, "|")?;
            }
            write!(f, "{val}")?;
        }
        Ok(())
    }
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
    fn matches_value(&self, value: Option<&str>) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        match self.equality {
            Equality::Equal => self.matches_value_option(value),
            Equality::NotEqual => !self.matches_value_option(value),
        }
    }

    /// Check if an optional value matches
    fn matches_value_option(&self, value: Option<&str>) -> bool {
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
        log::trace!("tag pattern: {p}");
        patterns.push(p);
    }
    Ok(patterns)
}

/// Parse data source
fn parse_source(source: &str) -> Result<DataSource> {
    match source {
        "osm" => Ok(DataSource::Osm),
        "json" => Ok(DataSource::Json),
        _ => Err(Error::UnknownDataSource()),
    }
}

/// Parse geometry type
fn parse_geom_type(geom_tp: &str) -> Result<GeomType> {
    match geom_tp {
        "point" => Ok(GeomType::Point),
        "linestring" => Ok(GeomType::Linestring),
        "polygon" => Ok(GeomType::Polygon),
        _ => Err(Error::UnknownGeometryType()),
    }
}

impl TryFrom<&LayerCfg> for LayerDef {
    type Error = Error;

    fn try_from(layer: &LayerCfg) -> Result<Self> {
        let name = layer.name.to_string();
        let source = parse_source(&layer.source)?;
        let geom_tp = parse_geom_type(&layer.geom_type)?;
        let (zoom_min, zoom_max) = parse_zoom_range(&layer.zoom)?;
        log::trace!("zoom: {}-{}", zoom_min, zoom_max);
        let patterns = parse_patterns(&layer.tags)?;
        Ok(LayerDef {
            name,
            source,
            geom_tp,
            zoom_min,
            zoom_max,
            patterns,
        })
    }
}

impl LayerDef {
    /// Get the layer name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get data source
    pub fn source(&self) -> DataSource {
        self.source
    }

    /// Get the geometry type
    pub fn geom_tp(&self) -> GeomType {
        self.geom_tp
    }

    /// Get a slice of tag patterns
    fn patterns(&self) -> &[TagPattern] {
        &self.patterns
    }

    /// Check if zoom level matches
    pub fn check_zoom(&self, zoom: u32) -> bool {
        zoom >= self.zoom_min && zoom <= self.zoom_max
    }

    /// Check if OSM tags match all patterns
    pub fn check_tags(&self, tags: &Tags) -> bool {
        for pattern in self.patterns() {
            if let Some(tag) = pattern.match_tag() {
                let value = tags.get(tag).map(|t| t.as_str());
                if !pattern.matches_value(value) {
                    return false;
                }
            }
        }
        true
    }

    /// Get an iterator of tags to include
    pub fn tags(&self) -> impl Iterator<Item = &str> {
        self.patterns().iter().filter_map(|pat| pat.include_tag())
    }

    /// Get an iterator of included tags, values and sint flags
    pub fn tag_values<'a>(
        &'a self,
        values: &'a [Option<String>],
    ) -> impl Iterator<Item = (&'a str, &'a str, bool)> {
        self.patterns()
            .iter()
            .filter_map(|pat| {
                pat.include_tag()
                    .map(|tag| (tag, pat.feature_type == FeatureType::MvtSint))
            })
            .zip(values)
            .filter_map(|((tag, sint), val)| {
                val.as_ref().map(|val| (tag, &val[..], sint))
            })
    }
}

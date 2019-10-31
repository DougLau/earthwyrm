// rules.rs
//
// Copyright (c) 2019  Minnesota Department of Transportation
//
use crate::Error;
use log::{debug, error, info};
use std::fs::File;
use std::io::{BufRead, BufReader};

const ZOOM_MAX: u32 = 30;

/// Tag pattern specification to require matching tag
#[derive(Clone, Debug, PartialEq)]
enum MustMatch {
    No,
    Yes,
}

/// Tag pattern specification to include tag value in layer
#[derive(Clone, Debug)]
enum IncludeValue {
    No,
    Yes,
}

/// Tag pattern specification to match value equal vs. not equal
#[derive(Clone, Debug)]
enum Equality {
    Equal,
    NotEqual,
}

/// Tag pattern specification for layer rule
#[derive(Clone, Debug)]
pub struct TagPattern {
    must_match: MustMatch,
    include: IncludeValue,
    key: String,
    equality: Equality,
    values: Vec<String>,
}

/// Layer rule definition
#[derive(Clone, Debug)]
pub struct LayerDef {
    name: String,
    table: String,
    zoom_min: u32,
    zoom_max: u32,
    patterns: Vec<TagPattern>,
}

impl TagPattern {
    /// Create a new "name" tag pattern
    fn new_name() -> Self {
        let must_match = MustMatch::No;
        let include = IncludeValue::Yes;
        let key = "name".to_string();
        let equality = Equality::NotEqual;
        let values = vec!["_".to_string()];
        TagPattern {
            must_match,
            include,
            key,
            equality,
            values,
        }
    }
    /// Get the tag (key)
    pub fn tag(&self) -> &str {
        &self.key
    }
    /// Get key for match patterns only
    pub fn match_key(&self) -> Option<&str> {
        match self.must_match {
            MustMatch::Yes => Some(self.tag()),
            MustMatch::No => None,
        }
    }
    /// Get key for include patterns only
    pub fn include_key(&self) -> Option<&str> {
        match self.include {
            IncludeValue::Yes => Some(self.tag()),
            IncludeValue::No => None,
        }
    }
    /// Check if the value matches
    pub fn matches_value(&self, value: Option<String>) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        match self.equality {
            Equality::Equal => self.matches_value_option(value),
            Equality::NotEqual => !self.matches_value_option(value),
        }
    }
    /// Check if an optional value matches
    fn matches_value_option(&self, value: Option<String>) -> bool {
        debug_assert!(self.must_match == MustMatch::Yes);
        match value {
            Some(val) => self.values.iter().any(|v| v == &val),
            None => self.values.iter().any(|v| v == &"_"),
        }
    }
    /// Parse a tag pattern rule
    fn parse_rule(pat: &str) -> (MustMatch, IncludeValue, &str) {
        if pat.starts_with('.') {
            (MustMatch::Yes, IncludeValue::Yes, &pat[1..])
        } else if pat.starts_with('?') {
            (MustMatch::No, IncludeValue::Yes, &pat[1..])
        } else {
            (MustMatch::Yes, IncludeValue::No, pat)
        }
    }
    /// Parse the equality portion
    fn parse_equality(pat: &str) -> Option<(&str, Equality, &str)> {
        if pat.contains('=') {
            let mut kv = pat.splitn(2, '=');
            let key = kv.next()?;
            let val = kv.next()?;
            if key.ends_with('!') {
                let key = &key[..key.len() - 1];
                Some((key, Equality::NotEqual, val))
            } else {
                Some((key, Equality::Equal, val))
            }
        } else {
            Some((pat, Equality::NotEqual, &"_"))
        }
    }
    /// Parse the value(s) portion
    fn parse_values(val: &str) -> Vec<String> {
        val.split('|').map(|v| v.to_string()).collect()
    }
    /// Parse a tag pattern rule
    fn parse(pat: &str) -> Option<TagPattern> {
        let (must_match, include, pat) = TagPattern::parse_rule(pat);
        let (key, equality, values) = TagPattern::parse_equality(pat)?;
        let key = key.to_string();
        let values = TagPattern::parse_values(values);
        Some(TagPattern {
            must_match,
            include,
            key,
            equality,
            values,
        })
    }
}

/// Parse the zoom portion of a layer rule
fn parse_zoom(z: &str) -> Option<(u32, u32)> {
    if z.ends_with('+') {
        let c = z.len() - 1;
        let zoom_min = parse_u32(&z[..c])?;
        Some((zoom_min, ZOOM_MAX))
    } else if z.contains('-') {
        let mut s = z.splitn(2, '-');
        let zoom_min = parse_u32(s.next()?)?;
        let zoom_max = parse_u32(s.next()?)?;
        Some((zoom_min, zoom_max))
    } else {
        let z = parse_u32(z)?;
        Some((z, z))
    }
}

/// Parse a u32 value
fn parse_u32(v: &str) -> Option<u32> {
    match v.parse::<u32>() {
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

/// Parse tag patterns of a layer rule
fn parse_patterns(c: &mut dyn Iterator<Item = &str>) -> Option<Vec<TagPattern>>
{
    let mut patterns = Vec::<TagPattern>::new();
    loop {
        if let Some(p) = c.next() {
            let p = TagPattern::parse(p)?;
            let key = p.tag();
            if let Some(d) = patterns.iter().find(|p| p.tag() == key) {
                error!("duplicate pattern {:?}", d);
                return None;
            }
            patterns.push(p);
        } else {
            break;
        }
    }
    if patterns.len() > 0 {
        if !patterns.iter().any(|p| &p.tag() == &"name") {
            patterns.push(TagPattern::new_name());
        }
        Some(patterns)
    } else {
        None
    }
}

/// Parse one layer definition
fn parse_layer_def(line: &str) -> Option<LayerDef> {
    let line = if let Some(hash) = line.find('#') {
        &line[..hash]
    } else {
        &line
    };
    let c: Vec<&str> = line.split_whitespace().collect();
    match c.len() {
        0 => None,
        1..=3 => {
            error!("Invalid rule (not enough columns): {}", line);
            None
        }
        _ => {
            let ld = LayerDef::parse(&mut c.into_iter());
            if ld.is_none() {
                error!("parsing \"{}\"", line);
            }
            ld
        }
    }
}

impl LayerDef {
    /// Load layer rule definition file
    pub fn load_all(rules_path: &str) -> Result<Vec<LayerDef>, Error> {
        let mut defs = vec![];
        let f = BufReader::new(File::open(rules_path)?);
        for line in f.lines() {
            if let Some(ld) = parse_layer_def(&line?) {
                debug!("LayerDef: {:?}", &ld);
                defs.push(ld);
            }
        }
        let mut names = String::new();
        for ld in &defs {
            names.push(' ');
            names.push_str(&ld.name);
        }
        info!("{} layers loaded:{}", defs.len(), names);
        Ok(defs)
    }
    /// Parse a layer definition rule
    fn parse(c: &mut dyn Iterator<Item = &str>) -> Option<Self> {
        let name = c.next()?.to_string();
        let table = c.next()?.to_string();
        let (zoom_min, zoom_max) = parse_zoom(c.next()?)?;
        let patterns = parse_patterns(c)?;
        Some(LayerDef {
            name,
            table,
            zoom_min,
            zoom_max,
            patterns,
        })
    }
    /// Get the layer name
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Get the table name
    pub fn table(&self) -> &str {
        &self.table
    }
    /// Get a slice of tag patterns
    pub fn patterns(&self) -> &[TagPattern] {
        &self.patterns
    }
    /// Check a table definition and zoom level
    pub fn check_table(&self, table: &str, zoom: u32) -> bool {
        self.check_zoom(zoom) && self.table == table
    }
    /// Check if zoom level matches
    fn check_zoom(&self, zoom: u32) -> bool {
        zoom >= self.zoom_min && zoom <= self.zoom_max
    }
}

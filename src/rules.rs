// rules.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::Error;

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
pub struct TagPattern {
    /// Pattern must / must not match
    must_match: MustMatch,
    /// Should key be included in layer
    include: IncludeValue,
    /// Key name
    key: String,
    /// Pattern equality
    equality: Equality,
    /// Pattern values
    values: Vec<String>,
}

/// Layer rule definition
#[derive(Clone, Debug)]
pub struct LayerDef {
    /// Layer name
    name: String,
    /// Table name
    table: String,
    /// Minimum zoom level
    zoom_min: u32,
    /// Maximum zoom level
    zoom_max: u32,
    /// Tag patterns
    patterns: Vec<TagPattern>,
}

impl Default for TagPattern {
    fn default() -> Self {
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
}

impl TagPattern {
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
    fn parse_equality(pat: &str) -> (&str, Equality, &str) {
        let kv: Vec<&str> = pat.splitn(2, '=').collect();
        if kv.len() > 1 {
            let key = kv[0];
            let val = kv[1];
            if key.ends_with('!') {
                let len = key.len() - 1;
                (&key[..len], Equality::NotEqual, val)
            } else {
                (key, Equality::Equal, val)
            }
        } else {
            (pat, Equality::NotEqual, &"_")
        }
    }

    /// Parse the value(s) portion
    fn parse_values(val: &str) -> Vec<String> {
        val.split('|').map(|v| v.to_string()).collect()
    }

    /// Parse a tag pattern rule
    fn parse(pat: &str) -> TagPattern {
        let (must_match, include, pat) = TagPattern::parse_rule(pat);
        let (key, equality, values) = TagPattern::parse_equality(pat);
        let key = key.to_string();
        let values = TagPattern::parse_values(values);
        TagPattern {
            must_match,
            include,
            key,
            equality,
            values,
        }
    }
}

/// Parse the zoom portion of a layer rule
fn parse_zoom(z: &str) -> Result<(u32, u32), Error> {
    let zz: Vec<&str> = z.splitn(2, '-').collect();
    if zz.len() > 1 {
        let zoom_min = zz[0].parse()?;
        let zoom_max = zz[1].parse()?;
        Ok((zoom_min, zoom_max))
    } else if z.ends_with('+') {
        let c = z.len() - 1;
        let zoom_min = z[..c].parse()?;
        Ok((zoom_min, ZOOM_MAX))
    } else {
        let z = z.parse()?;
        Ok((z, z))
    }
}

/// Parse tag patterns of a layer rule
fn parse_patterns(rule: &[String]) -> Result<Vec<TagPattern>, Error> {
    let mut patterns = Vec::<TagPattern>::new();
    for pat in rule {
        let p = TagPattern::parse(pat);
        let key = p.tag();
        if let Some(_) = patterns.iter().find(|p| p.tag() == key) {
            return Err(Error::DuplicatePattern(pat.to_string()));
        }
        log::debug!("tag pattern: {:?}", &p);
        patterns.push(p);
    }
    if patterns.len() > 0 {
        // Add default pattern if no "name" patterns exist
        if !patterns.iter().any(|p| &p.tag() == &"name") {
            patterns.push(TagPattern::default());
        }
    }
    Ok(patterns)
}

impl LayerDef {
    /// Create a new layer definition
    pub fn new(
        name: &str,
        table: &str,
        zoom: &str,
        patterns: &[String],
    ) -> Result<Self, Error> {
        let name = name.to_string();
        let table = table.to_string();
        let (zoom_min, zoom_max) = parse_zoom(zoom)?;
        log::debug!("zoom: {}-{}", zoom_min, zoom_max);
        let patterns = parse_patterns(patterns)?;
        Ok(LayerDef {
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

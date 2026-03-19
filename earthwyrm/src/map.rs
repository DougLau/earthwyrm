// Copyright (C) 2026  Minnesota Department of Transportation
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
use crate::error::{Error, Result};
use hatmil::{Page, html};
use squarepeg::Peg;
use web_sys::{Document, Element};

/// Map widget
pub struct Map {
    /// Map element ID
    id: String,
    /// Style element ID
    style_id: String,
    /// Origin peg tile
    origin: Option<Peg>,
}

impl Map {
    /// Create new map on `id` element
    pub fn new(id: &str) -> Result<Self> {
        let style_id = format!("{id}-style");
        let map = Map {
            id: id.to_string(),
            style_id,
            origin: None,
        };
        let _elem = lookup_id(&map.id)?;
        let mut page = Page::new();
        let mut style = page.frag::<html::Style>();
        style.id(&map.style_id);
        let style = String::from(page);
        let head = lookup_head()?;
        head.append_with_str_1(&style)
            .map_err(|_e| Error::WebSys("append_with_str_1"))?;
        Ok(map)
    }

    /// Set map CSS rules
    pub fn set_style(&self, css: &str) -> Result<()> {
        let style = lookup_id(&self.style_id)?;
        style.set_inner_html(css);
        Ok(())
    }

    /// Set map view
    pub fn set_view(&mut self, _zoom: u16, _lat: f64, _lon: f64) {
        // FIXME: set origin peg
        //        fetch tiles (asynchronously)
        //        start fade animation to new tiles
        //        remove unused tiles (garbage collect)
    }
}

/// Get document
fn doc() -> Result<Document> {
    let window = web_sys::window().ok_or(Error::WebSys("no window"))?;
    let doc = window.document().ok_or(Error::WebSys("no document"))?;
    Ok(doc)
}

/// Lookup the document head element
fn lookup_head() -> Result<Element> {
    let heads = doc()?.get_elements_by_tag_name("head");
    if heads.length() > 0 {
        heads.item(0).ok_or(Error::WebSys("no head 0"))
    } else {
        Err(Error::WebSys("no head"))
    }
}

/// Lookup an element by ID
fn lookup_id(id: &str) -> Result<Element> {
    let elem = doc()?
        .get_element_by_id(id)
        .ok_or(Error::WebSys("elem not found"))?;
    Ok(elem)
}

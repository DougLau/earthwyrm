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
use crate::fetch::Uri;
use squarepeg::{MapGrid, Peg, WebMercatorPos, Wgs84Pos};
use web_sys::{Document, Element};

/// Map widget
pub struct Map {
    /// Map element ID
    id: String,
    /// Map grid
    grid: MapGrid,
    /// Layer groups
    groups: &'static [&'static str],
}

impl Map {
    /// Create new map on `id` element
    pub fn new(id: &str, groups: &'static [&'static str]) -> Self {
        Map {
            id: id.to_string(),
            grid: MapGrid::default(),
            groups,
        }
    }

    /// Get ID of map style element
    fn style_id(&self) -> String {
        format!("{}-style", self.id)
    }

    /// Set map CSS rules
    pub fn set_style(&self, css: &str) -> Result<()> {
        let style = lookup_id(&self.style_id())?;
        style.set_inner_html(css);
        Ok(())
    }

    /// Set map view
    pub async fn set_view(&self, zoom: u32, lon: f64, lat: f64) -> Result<()> {
        let pos: WebMercatorPos = Wgs84Pos::new(lon, lat).into();
        match self.grid.zxy_peg(zoom, pos.x, pos.y) {
            Some(peg) => self.do_set_view(peg, pos).await,
            None => {
                log::warn!("invalid Peg: {zoom} {lon} {lat}");
                Ok(())
            }
        }
    }

    /// Set view
    async fn do_set_view(&self, peg: Peg, _pos: WebMercatorPos) -> Result<()> {
        let elem = lookup_id(&self.id)?;
        let rect = elem.get_bounding_client_rect();
        let width = ((rect.width() / 256.0).floor() as u32) + 1;
        let height = ((rect.height() / 256.0).floor() as u32) + 1;
        // FIXME: center lon/lat (pos)
        let zoom = peg.z();
        let mut inner = String::new();
        for y in 0..=height {
            let py = peg.y() + y;
            for x in 0..=width {
                let px = peg.x() + x;
                if let Some(peg) = Peg::new(zoom, px, py) {
                    for group in self.groups {
                        match fetch_tile(group, peg, x as i32, y as i32).await {
                            Ok(svg) => inner.push_str(&svg),
                            Err(Error::HttpNotFound()) => (),
                            Err(e) => log::warn!("fetch {peg:?} {e:?}"),
                        }
                    }
                }
            }
        }
        elem.set_inner_html(&inner);
        // FIXME: start fade animation to new tiles
        // FIXME: remove unused tiles (garbage collect)
        Ok(())
    }
}

/// Get document
fn doc() -> Result<Document> {
    let window = web_sys::window().ok_or(Error::WebSys("no window"))?;
    let doc = window.document().ok_or(Error::WebSys("no document"))?;
    Ok(doc)
}

/// Lookup an element by ID
fn lookup_id(id: &str) -> Result<Element> {
    let elem = doc()?
        .get_element_by_id(id)
        .ok_or(Error::WebSys("elem not found"))?;
    Ok(elem)
}

/// Fetch one wyrm tile
async fn fetch_tile(group: &str, peg: Peg, xt: i32, yt: i32) -> Result<String> {
    let mut uri = Uri::from("/");
    uri.push(group);
    uri.push(&peg.z().to_string());
    uri.push(&peg.x().to_string());
    uri.push(&peg.y().to_string());
    uri.add_extension(".wyrm");
    let wyrm = uri.get().await?;
    let transform = if xt != 0 || yt != 0 {
        format!(" transform=\"translate({} {})\"", xt * 256, yt * 256)
    } else {
        String::new()
    };
    Ok(format!(
        "<svg id=\"{group}-{peg}\" class=\"wyrm-tile\"{transform}>{wyrm}</svg>"
    ))
}

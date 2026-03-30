// Copyright (C) 2026  Douglas Lau
//
use crate::error::{Error, Result};
use crate::fetch::Uri;
use squarepeg::{MapGrid, Peg, WebMercatorPos, Wgs84Pos};
use web_sys::{Document, Element};

/// Map widget
#[derive(Clone)]
pub struct Map {
    /// Map element ID
    id: String,
    /// Map grid
    grid: MapGrid,
    /// Layer groups
    groups: &'static [&'static str],
    /// Cycle number
    cycle: u32,
}

impl Map {
    /// Create new map on `id` element
    pub fn new(id: &str, groups: &'static [&'static str]) -> Self {
        Map {
            id: id.to_string(),
            grid: MapGrid::default(),
            groups,
            cycle: 0,
        }
    }

    /// Advance to next cycle
    pub fn next_cycle(&mut self) {
        self.cycle += 1;
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
        elem.set_attribute("style", "")
            .map_err(|_e| Error::WebSys("set_style view"))?;
        // FIXME: center lon/lat (pos)
        let zoom = peg.z();
        let mut inner = String::new();
        for y in 0..=height {
            let py = peg.y() + y;
            for x in 0..=width {
                let px = peg.x() + x;
                if let Some(peg) = Peg::new(zoom, px, py) {
                    for group in self.groups {
                        match fetch_tile(
                            group, peg, self.cycle, x as i32, y as i32,
                        )
                        .await
                        {
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

    /// Set style
    pub fn set_style(&self, value: &str) -> Result<()> {
        let elem = lookup_id(&self.id)?;
        elem.set_attribute("style", value)
            .map_err(|_e| Error::WebSys("set_style"))?;
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
async fn fetch_tile(
    group: &str,
    peg: Peg,
    cycle: u32,
    xt: i32,
    yt: i32,
) -> Result<String> {
    let mut uri = Uri::from("/");
    uri.push(group);
    uri.push(&peg.z().to_string());
    uri.push(&peg.x().to_string());
    uri.push(&peg.y().to_string());
    uri.add_extension(".wyrm");
    let wyrm = uri.get().await?;
    let mut svg = String::with_capacity(wyrm.len() + 100);
    svg.push_str("<svg class=\"wyrm-tile ");
    svg.push_str(group);
    svg.push('-');
    svg.push_str(&peg.to_string());
    svg.push_str(" cycle-");
    svg.push_str(&cycle.to_string());
    svg.push('"');
    if xt != 0 || yt != 0 {
        svg.push_str(" transform=\"translate(");
        svg.push_str(&(xt * 256).to_string());
        svg.push(' ');
        svg.push_str(&(yt * 256).to_string());
        svg.push_str(")\"");
    }
    svg.push('>');
    svg.push_str(&wyrm);
    svg.push_str("</svg>");
    Ok(svg)
}

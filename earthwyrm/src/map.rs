// Copyright (C) 2026  Douglas Lau
//
use crate::error::{Error, Result};
use crate::fetch::Uri;
use crate::util::lookup_id;
use squarepeg::{MapGrid, Peg, WebMercatorPos, Wgs84Pos};
use wasm_bindgen_futures::spawn_local;
use web_sys::DomRect;

/// Map pane
#[derive(Clone)]
pub struct MapPane {
    /// Map element ID
    id: String,
    /// Map grid
    grid: MapGrid,
    /// Layer groups
    groups: &'static [&'static str],
    /// Cycle number
    cycle: u32,
}

impl MapPane {
    /// Initialize the map pane
    ///
    /// - `id`: HTML `id` attribute of map element
    /// - `groups`: Layer group tile names
    pub fn init(id: &str, groups: &'static [&'static str]) -> Option<Self> {
        match crate::state::init(id, groups) {
            Ok(_) => Some(MapPane::new(id, groups)),
            Err(e) => {
                log::warn!("MapPane::init: {e:?}");
                None
            }
        }
    }

    /// Create new map on `id` element
    pub(crate) fn new(id: &str, groups: &'static [&'static str]) -> Self {
        MapPane {
            id: id.to_string(),
            grid: MapGrid::default(),
            groups,
            cycle: 0,
        }
    }

    /// Get map pane
    pub fn get() -> Option<MapPane> {
        crate::state::map_pane()
    }

    /// Advance to next cycle
    pub(crate) fn next_cycle(&mut self) {
        self.cycle += 1;
    }

    /// Center map at a given position
    pub fn center(self, zoom: u32, lon: f64, lat: f64) {
        let pos: WebMercatorPos = Wgs84Pos::new(lon, lat).into();
        match self.grid.zxy_peg(zoom, pos.x, pos.y) {
            Some(peg) => spawn_local(self.do_center(peg, pos)),
            None => log::warn!("invalid Peg: {zoom} {lon} {lat}"),
        }
    }

    /// Center map at a given position
    async fn do_center(self, peg: Peg, pos: WebMercatorPos) {
        let Ok(elem) = lookup_id(&self.id) else {
            return;
        };
        // start fading out current tiles
        self.set_anim(
            ".wyrm-tile { animation: wyrm-fade-out 0.25s forwards; }",
        );
        let rect = elem.get_bounding_client_rect();
        // "Center" position at (0.32, 0.5) of client bounds
        let cx = (rect.width() * 0.32) as u32;
        let cy = (rect.height() * 0.5) as u32;
        // Offset from north-west corner of peg (0-255)
        let off = (pos.x, pos.y) * self.grid.peg_transform(peg);
        let ox = (off.x * 256.0) as u32;
        let oy = (off.y * 256.0) as u32;
        let (peg_nw, peg_se) = peg_bounds(&rect, peg, cx - ox, cy - oy);
        let zoom = peg.z();
        let ocx = ((peg.x() - peg_nw.x()) * 256 + ox).saturating_sub(cx);
        let ocy = ((peg.y() - peg_nw.y()) * 256 + oy).saturating_sub(cy);
        let origin = (-(ocx as i32), -(ocy as i32));
        let mut inner = String::new();
        for py in peg_nw.y()..=peg_se.y() {
            let ty = (py - peg_nw.y()) as i32;
            for px in peg_nw.x()..=peg_se.x() {
                if let Some(peg) = Peg::new(zoom, px, py) {
                    let tx = (px - peg_nw.x()) as i32;
                    for group in self.groups {
                        match fetch_tile(group, peg, self.cycle, tx, ty).await {
                            Ok(svg) => inner.push_str(&svg),
                            Err(Error::HttpNotFound()) => (),
                            Err(e) => log::warn!("fetch {peg:?} {e:?}"),
                        }
                    }
                }
            }
        }
        crate::state::reset(origin.0, origin.1);
        elem.set_inner_html(&inner);
        // start fading in new tiles
        self.set_anim(".wyrm-tile { animation: wyrm-fade-in 0.25s forwards; }");
    }

    /// Set style
    pub(crate) fn set_style(&self, value: &str) {
        match lookup_id(&self.id) {
            Ok(elem) => {
                if let Err(e) = elem.set_attribute("style", value) {
                    log::warn!("set_style: {e:?}");
                }
            }
            Err(e) => log::warn!("set_style: {e:?}"),
        }
    }

    /// Set wyrm animation style
    fn set_anim(&self, css: &str) {
        match lookup_id("wyrm-anim-style") {
            Ok(style) => style.set_inner_html(css),
            Err(e) => log::warn!("set_anim: {e:?}"),
        }
    }
}

/// Calculate bounds for a client rectangle
fn peg_bounds(rect: &DomRect, peg: Peg, x: u32, y: u32) -> (Peg, Peg) {
    let px = peg.x().saturating_sub((x + 256) / 256);
    let py = peg.y().saturating_sub((y + 256) / 256);
    let peg_nw = Peg::new(peg.z(), px, py).unwrap_or(peg);
    let width = (rect.width() as u32 + 256) / 256;
    let height = (rect.height() as u32 + 256) / 256;
    let px = peg_nw.x().saturating_add(width);
    let py = peg_nw.y().saturating_add(height);
    let peg_se = Peg::new(peg.z(), px, py).unwrap_or(peg);
    (peg_nw, peg_se)
}

/// Fetch one wyrm tile
async fn fetch_tile(
    group: &str,
    peg: Peg,
    cycle: u32,
    tx: i32,
    ty: i32,
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
    if tx != 0 || ty != 0 {
        svg.push_str(" transform=\"translate(");
        svg.push_str(&(tx * 256).to_string());
        svg.push(' ');
        svg.push_str(&(ty * 256).to_string());
        svg.push_str(")\"");
    }
    svg.push('>');
    svg.push_str(&wyrm);
    svg.push_str("</svg>");
    Ok(svg)
}

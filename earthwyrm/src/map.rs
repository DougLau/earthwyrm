// Copyright (C) 2026  Douglas Lau
//
use crate::error::{Error, Result};
use crate::fetch::Uri;
use crate::util::lookup_id;
use squarepeg::{MapGrid, Peg, WebMercatorPos, Wgs84Pos};
use wasm_bindgen_futures::spawn_local;

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
    async fn do_center(self, peg: Peg, _pos: WebMercatorPos) {
        let Ok(elem) = lookup_id(&self.id) else {
            return;
        };
        // start fading out current tiles
        if let Ok(style) = lookup_id("wyrm-anim-style") {
            style.set_inner_html(
                ".wyrm-tile { animation: wyrm-fade-out 0.5s forwards; }",
            );
        }
        let rect = elem.get_bounding_client_rect();
        let width = ((rect.width() / 256.0).floor() as u32) + 1;
        let height = ((rect.height() / 256.0).floor() as u32) + 1;
        if let Err(e) = elem.set_attribute("style", "") {
            log::warn!("do_center: {e:?}");
            return;
        }
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
        // start fading in new tiles
        if let Ok(style) = lookup_id("wyrm-anim-style") {
            style.set_inner_html(
                ".wyrm-tile { animation: wyrm-fade-in 0.25s forwards; }",
            );
        }
        // FIXME: remove unused tiles (garbage collect)
    }

    /// Set style
    pub(crate) fn set_style(&self, value: &str) -> Result<()> {
        let elem = lookup_id(&self.id)?;
        elem.set_attribute("style", value)
            .map_err(|_e| Error::Other("set_style"))?;
        Ok(())
    }
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

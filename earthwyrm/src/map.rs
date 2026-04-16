// Copyright (C) 2026  Douglas Lau
//
use crate::tile::make_fetcher;
use crate::util::lookup_id;
use futures_util::StreamExt;
use jiff::Zoned;
use squarepeg::{MapGrid, Peg, WebMercatorPos, Wgs84Pos};
use wasm_bindgen_futures::spawn_local;
use web_sys::{DomRect, Element, Event};

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
    /// - `click_cb`: Click callback
    /// - `zoom_cb`: Zoom callback
    pub fn init(
        id: &str,
        groups: &'static [&'static str],
        click_cb: impl Fn(Event) + 'static,
        zoom_cb: impl Fn(u32) + 'static,
    ) -> Option<Self> {
        crate::state::init(id, groups, click_cb, zoom_cb)
            .inspect_err(|e| log::warn!("MapPane::init: {e:?}"))
            .ok()
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

    /// Get map grid
    pub(crate) fn grid(&self) -> &MapGrid {
        &self.grid
    }

    /// Get map pane
    pub fn get() -> Option<MapPane> {
        crate::state::map_pane()
    }

    /// Lookup map pane element
    fn elem(&self) -> Option<Element> {
        lookup_id(&self.id)
            .inspect_err(|e| log::warn!("{e:?}"))
            .ok()
    }

    /// Advance to next cycle
    pub(crate) fn next_cycle(&mut self) {
        self.cycle += 1;
    }

    /// Get client position in rectangle
    pub(crate) fn client_pos(&self, rx: i32, ry: i32) -> (f64, f64) {
        if let Some(elem) = self.elem() {
            let rect = elem.get_bounding_client_rect();
            let cx = rx as f64 / rect.width();
            let cy = ry as f64 / rect.height();
            (cx, cy)
        } else {
            (0.0, 0.0)
        }
    }

    /// Position map with given zoom and lon/lat
    pub fn position(self, zoom: u32, lon: f64, lat: f64, rx: f64, ry: f64) {
        if let Some(elem) = self.elem() {
            let pos: WebMercatorPos = Wgs84Pos::new(lon, lat).into();
            match self.grid.zxy_peg(zoom, pos.x, pos.y) {
                Some(peg) => {
                    spawn_local(self.do_position(elem, peg, pos, rx, ry))
                }
                None => log::warn!("invalid Peg: {zoom} {lon} {lat}"),
            }
        }
    }

    /// Position map with given peg and lon/lat
    async fn do_position(
        self,
        elem: Element,
        peg: Peg,
        pos: WebMercatorPos,
        rx: f64,
        ry: f64,
    ) {
        // start fading out current tiles
        self.set_anim(
            ".wyrm-tile { animation: wyrm-fade-out 0.25s forwards; }",
        );
        let rect = elem.get_bounding_client_rect();
        // "Client" position within rectangle
        let cx = (rect.width() * rx) as u32;
        let cy = (rect.height() * ry) as u32;
        let peg_nw = peg_nw(peg, cx, cy);
        let peg_se = peg_se(peg, &rect);
        // Offset from north-west corner of peg (0-255)
        let off = (pos.x, pos.y) * self.grid.peg_transform(peg);
        let ox = (off.x * 256.0) as u32;
        let oy = (off.y * 256.0) as u32;
        let ocx = ((peg.x() - peg_nw.x()) * 256 + ox).saturating_sub(cx);
        let ocy = ((peg.y() - peg_nw.y()) * 256 + oy).saturating_sub(cy);
        let origin = (-(ocx as i32), -(ocy as i32));
        let mut layers = Vec::new();
        for _gr in self.groups {
            layers.push(String::with_capacity(1024));
        }
        let start = Zoned::now();
        let mut n_tiles = 0;
        let mut fetcher = make_fetcher(peg_nw, peg_se, self.groups, self.cycle);
        while let Some((group, svg)) = fetcher.next().await {
            for (gr, layer) in self.groups.iter().zip(&mut layers) {
                if *gr == group {
                    layer.push_str(&svg);
                    n_tiles += 1;
                }
            }
        }
        let finish = Zoned::now();
        log::info!("fetched {n_tiles} tiles in {:#}", finish - start);
        crate::state::reset(origin, peg_nw);
        elem.set_inner_html(&layers.join(""));
        // start fading in new tiles
        self.set_anim(".wyrm-tile { animation: wyrm-fade-in 0.25s forwards; }");
    }

    /// Set style
    pub(crate) fn set_style(&self, value: &str) {
        if let Some(elem) = self.elem()
            && let Err(e) = elem.set_attribute("style", value)
        {
            log::warn!("set_style: {e:?}");
        }
    }

    /// Set wyrm animation style
    fn set_anim(&self, css: &str) {
        match lookup_id("wyrm-anim-style") {
            Ok(style) => style.set_inner_html(css),
            Err(e) => log::warn!("set_anim: {e:?}"),
        }
    }

    /// Fetch tiles on an edge if necessary
    pub(crate) fn fetch_edge_tiles(&self) {
        if self.needs_more_tiles() {
            spawn_local(self.clone().do_fetch_edge_tiles());
        }
    }

    /// Check if more tiles need to be fetched
    fn needs_more_tiles(&self) -> bool {
        // FIXME
        false
    }

    /// Fetch tiles on an edge if necessary
    async fn do_fetch_edge_tiles(self) {
        // FIXME: do the needful
    }

    /// Get zoom level
    pub fn zoom(&self) -> u32 {
        crate::state::zoom()
    }
}

/// Calculate Northwest peg
fn peg_nw(peg: Peg, cx: u32, cy: u32) -> Peg {
    let px = peg.x().saturating_sub(cx / 256 + 1);
    let py = peg.y().saturating_sub(cy / 256 + 1);
    Peg::new(peg.z(), px, py).unwrap_or(peg)
}

/// Calculate Southeast peg
fn peg_se(peg: Peg, rect: &DomRect) -> Peg {
    let width = (rect.width() as u32) / 256;
    let height = (rect.height() as u32) / 256;
    let px = peg.x().saturating_add(width + 1);
    let py = peg.y().saturating_add(height + 1);
    Peg::new(peg.z(), px, py).unwrap_or(peg)
}

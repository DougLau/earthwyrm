// Copyright (C) 2026  Douglas Lau
//
use crate::state::MapEvent;
use crate::tile::make_fetcher;
use crate::util::lookup_id;
use futures_util::StreamExt;
use jiff::Zoned;
use squarepeg::{MapGrid, Peg, WebMercatorPos, Wgs84Pos};
use wasm_bindgen_futures::spawn_local;
use web_sys::{DomRect, Element};

/// Map pane
#[derive(Clone)]
pub struct MapPane {
    /// Map element ID
    id: String,
    /// Map grid
    grid: MapGrid,
    /// Anchor position within pane
    anchor: (f64, f64),
    /// Layer groups
    groups: &'static [&'static str],
    /// Zoom handler
    pub(crate) zoom_handler: fn(u32) -> (),
    /// Click handler
    pub(crate) click_handler: fn(MapEvent) -> (),
    /// Context menu handler
    pub(crate) contextmenu_handler: fn(MapEvent, i32, i32) -> (),
}

impl MapPane {
    /// Get registered map pane by element ID
    pub fn get(id: &str) -> Option<MapPane> {
        crate::state::map_pane(id)
    }

    /// Create a new map on element with ID
    pub fn new(id: &str) -> Self {
        let zoom_handler = |_z| {};
        let click_handler = |_tgt| {};
        let contextmenu_handler = |_tgt, _x, _y| {};
        MapPane {
            id: id.to_string(),
            anchor: (0.5, 0.5),
            grid: MapGrid::default(),
            groups: &[],
            zoom_handler,
            click_handler,
            contextmenu_handler,
        }
    }

    /// Set anchor point
    ///
    /// - `ax`: Anchor X value, between `0.0` and `1.0`
    /// - `ay`: Anchor Y value, between `0.0` and `1.0`
    pub fn with_anchor(mut self, ax: f64, ay: f64) -> Self {
        self.anchor = (ax, ay);
        self
    }

    /// Set layer group tile names
    pub fn with_groups(mut self, groups: &'static [&'static str]) -> Self {
        self.groups = groups;
        self
    }

    /// Set zoom event handler
    pub fn with_zoom_handler(mut self, zoom_handler: fn(u32) -> ()) -> Self {
        self.zoom_handler = zoom_handler;
        self
    }

    /// Set click event handler
    pub fn with_click_handler(
        mut self,
        click_handler: fn(MapEvent) -> (),
    ) -> Self {
        self.click_handler = click_handler;
        self
    }

    /// Set contextmenu event handler
    pub fn with_contextmenu_handler(
        mut self,
        contextmenu_handler: fn(MapEvent, i32, i32) -> (),
    ) -> Self {
        self.contextmenu_handler = contextmenu_handler;
        self
    }

    /// Register the map pane in the browser
    ///
    /// Returns `true` if successful
    pub fn register(self) -> bool {
        crate::state::register(self)
            .inspect_err(|e| log::warn!("MapPane::register: {e:?}"))
            .is_ok()
    }

    /// Get map element ID
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Get map grid
    pub(crate) fn grid(&self) -> &MapGrid {
        &self.grid
    }

    /// Lookup map pane element
    fn elem(&self) -> Option<Element> {
        lookup_id(&self.id)
            .inspect_err(|e| log::warn!("{e:?}"))
            .ok()
    }

    /// Get client position in rectangle
    pub(crate) fn client_pos(&self, ax: i32, ay: i32) -> (f64, f64) {
        if let Some(elem) = self.elem() {
            let rect = elem.get_bounding_client_rect();
            let cx = ax as f64 / rect.width();
            let cy = ay as f64 / rect.height();
            (cx, cy)
        } else {
            (0.0, 0.0)
        }
    }

    /// Set map position with given zoom and lon/lat
    pub fn set_position(self, zoom: u32, lon: f64, lat: f64) {
        let (ax, ay) = self.anchor;
        self.set_position_anchor(zoom, lon, lat, ax, ay);
    }

    /// Set map position with given zoom, lon/lat and anchor
    pub(crate) fn set_position_anchor(
        self,
        zoom: u32,
        lon: f64,
        lat: f64,
        ax: f64,
        ay: f64,
    ) {
        if let Some(elem) = self.elem() {
            let pos: WebMercatorPos = Wgs84Pos::new(lon, lat).into();
            match self.grid.zxy_peg(zoom, pos.x, pos.y) {
                Some(peg) => {
                    spawn_local(self.do_position(elem, peg, pos, ax, ay))
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
        ax: f64,
        ay: f64,
    ) {
        // start fading out current tiles
        self.set_anim(
            ".wyrm-tile { animation: wyrm-fade-out 0.25s forwards; }",
        );
        let rect = elem.get_bounding_client_rect();
        // "Client" position within rectangle
        let cx = (rect.width() * ax) as u32;
        let cy = (rect.height() * ay) as u32;
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
        let mut fetcher = make_fetcher(peg_nw, peg_se, self.groups);
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

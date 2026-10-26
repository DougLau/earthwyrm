// Copyright (C) 2026  Douglas Lau
//
use crate::state::{self, MapEvent};
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

    /// Set anchor point
    ///
    /// - `ax`: Anchor X value, in pixels
    /// - `ay`: Anchor Y value, in pixels
    pub(crate) fn with_anchor_px(mut self, ax: i32, ay: i32) -> Self {
        if let Some(elem) = self.elem() {
            let rect = elem.get_bounding_client_rect();
            let cx = f64::from(ax) / rect.width();
            let cy = f64::from(ay) / rect.height();
            self.anchor = (cx, cy);
        }
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

    /// Get layer groups names
    pub fn groups(&self) -> &'static [&'static str] {
        self.groups
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

    /// Set map position with given zoom and lon/lat
    pub fn set_position(self, zoom: u32, lon: f64, lat: f64) {
        if let Some(elem) = self.elem() {
            let pos: WebMercatorPos = Wgs84Pos::new(lon, lat).into();
            match self.grid.zxy_peg(zoom, pos.x, pos.y) {
                Some(peg) => {
                    let (peg_nw, peg_se, offset) =
                        self.peg_bounds(&elem, peg, pos);
                    spawn_local(self.do_position(elem, peg_nw, peg_se, offset))
                }
                None => log::warn!("invalid Peg: {zoom} {lon} {lat}"),
            }
        }
    }

    /// Get Peg bounds
    fn peg_bounds(
        &self,
        elem: &Element,
        peg: Peg,
        pos: WebMercatorPos,
    ) -> (Peg, Peg, (i32, i32)) {
        let rect = elem.get_bounding_client_rect();
        let (ax, ay) = self.anchor;
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
        let offset = (-(ocx as i32), -(ocy as i32));
        (peg_nw, peg_se, offset)
    }

    /// Position map with given peg and lon/lat
    async fn do_position(
        self,
        elem: Element,
        peg_nw: Peg,
        peg_se: Peg,
        offset: (i32, i32),
    ) {
        // start fading out current tiles
        self.set_anim(
            ".wyrm-tile { animation: wyrm-fade-out 0.25s forwards; }",
        );
        let start = Zoned::now();
        let mut n_tiles = 0;
        let mut fetcher = state::fetcher(peg_nw, peg_se);
        while let Some((key, wyrm)) = fetcher.next().await {
            state::cache_wyrm(key, wyrm);
            n_tiles += 1;
        }
        let finish = Zoned::now();
        log::info!("fetched {n_tiles} tiles in {:#}", finish - start);
        let svg = state::render_map(peg_nw, peg_se);
        crate::state::reset(peg_nw, offset);
        elem.set_inner_html(&svg);
        // start fading in new tiles
        self.set_anim(".wyrm-tile { animation: wyrm-fade-in 0.25s forwards; }");
    }

    /// Set the map pane offset
    pub(crate) fn set_offset(&self, offset: (i32, i32), grabbing: bool) {
        let (x, y) = offset;
        let mut css = String::with_capacity(80);
        if x != 0 || y != 0 {
            css.push_str("transform: translate(");
            css.push_str(&x.to_string());
            css.push_str("px, ");
            css.push_str(&y.to_string());
            css.push_str("px);");
        }
        if grabbing {
            css.push_str("cursor: grabbing;");
        }
        self.set_style(&css);
        if self.needs_more_tiles() {
            spawn_local(self.clone().do_fetch_edge_tiles());
        }
    }

    /// Set style
    fn set_style(&self, value: &str) {
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

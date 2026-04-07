// Copyright (C) 2026  Douglas Lau
//
use crate::error::{Error, Result};
use crate::fetch::Uri;
use crate::util::lookup_id;
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

/// Tile fetcher
struct TileFetcher {
    /// Peg at northwest corner
    peg_nw: Peg,
    /// Peg at southeast corner
    peg_se: Peg,
    /// Layer groups to fetch
    groups: &'static [&'static str],
    /// Cycle number
    cycle: u32,
    /// Current peg X
    px: u32,
    /// Current peg Y
    py: u32,
    /// Current group index
    gr: usize,
}

impl MapPane {
    /// Initialize the map pane
    ///
    /// - `id`: HTML `id` attribute of map element
    /// - `groups`: Layer group tile names
    /// - `click_cb`: Click callback
    pub fn init(
        id: &str,
        groups: &'static [&'static str],
        click_cb: impl Fn(Event) + 'static,
    ) -> Option<Self> {
        crate::state::init(id, groups, click_cb)
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

    /// Center map at a given position
    pub fn center(self, zoom: u32, lon: f64, lat: f64) {
        if let Some(elem) = self.elem() {
            let pos: WebMercatorPos = Wgs84Pos::new(lon, lat).into();
            match self.grid.zxy_peg(zoom, pos.x, pos.y) {
                Some(peg) => spawn_local(self.do_center(elem, peg, pos)),
                None => log::warn!("invalid Peg: {zoom} {lon} {lat}"),
            }
        }
    }

    /// Center map at a given position
    async fn do_center(self, elem: Element, peg: Peg, pos: WebMercatorPos) {
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
        let peg_nw = peg_nw(peg, cx - ox, cy - oy);
        let peg_se = peg_se(peg, &rect);
        let ocx = ((peg.x() - peg_nw.x()) * 256 + ox).saturating_sub(cx);
        let ocy = ((peg.y() - peg_nw.y()) * 256 + oy).saturating_sub(cy);
        let origin = (-(ocx as i32), -(ocy as i32));
        let mut fetcher =
            TileFetcher::new(peg_nw, peg_se, self.groups, self.cycle);
        let mut inner = String::with_capacity(1024);
        while let Some(tile) = fetcher.next_tile().await {
            inner.push_str(&tile);
        }
        crate::state::reset(origin, peg_nw);
        elem.set_inner_html(&inner);
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
fn peg_nw(peg: Peg, ox: u32, oy: u32) -> Peg {
    let px = peg.x().saturating_sub((ox + 256) / 256);
    let py = peg.y().saturating_sub((oy + 256) / 256);
    Peg::new(peg.z(), px, py).unwrap_or(peg)
}

/// Calculate Southeast peg
fn peg_se(peg: Peg, rect: &DomRect) -> Peg {
    let width = (rect.width() as u32 + 256) / 256;
    let height = (rect.height() as u32 + 256) / 256;
    let px = peg.x().saturating_add(width);
    let py = peg.y().saturating_add(height);
    Peg::new(peg.z(), px, py).unwrap_or(peg)
}

impl TileFetcher {
    /// Make tile fetcher
    fn new(
        peg_nw: Peg,
        peg_se: Peg,
        groups: &'static [&'static str],
        cycle: u32,
    ) -> Self {
        let px = peg_nw.x();
        let py = peg_nw.y();
        TileFetcher {
            peg_nw,
            peg_se,
            groups,
            cycle,
            px,
            py,
            gr: 0,
        }
    }

    /// Fetch the next tile
    async fn next_tile(&mut self) -> Option<String> {
        let zoom = self.peg_nw.z();
        while !self.done() {
            let peg = Peg::new(zoom, self.px, self.py)?;
            let tx = (self.px - self.peg_nw.x()) as i32;
            let ty = (self.py - self.peg_nw.y()) as i32;
            let group = self.groups[self.gr];
            self.advance();
            match fetch_tile(group, peg, self.cycle, tx, ty).await {
                Ok(svg) => return Some(svg),
                Err(Error::HttpNotFound()) => (),
                Err(e) => log::warn!("fetch {peg:?} {e:?}"),
            }
        }
        None
    }

    /// Advance to the next tile
    fn advance(&mut self) {
        self.gr += 1;
        if self.gr >= self.groups.len() {
            self.gr = 0;
            self.px += 1;
            if self.px > self.peg_se.x() {
                self.px = self.peg_nw.x();
                self.py += 1;
            }
        }
    }

    /// Check if done fetching
    fn done(&self) -> bool {
        self.py > self.peg_se.y()
    }
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

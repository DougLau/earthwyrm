// Copyright (C) 2026  Minnesota Department of Transportation
//
use crate::error::{Error, Result};
use crate::map::MapPane;
use crate::util::lookup_id;
use squarepeg::{Peg, WebMercatorPos, Wgs84Pos};
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event, PointerEvent, WheelEvent};

/// Event target
#[derive(Debug)]
pub struct Target {
    /// Target class
    pub cls: String,
    /// Target layer
    pub layer: String,
    /// OSM reference (`data-ref`)
    pub osm_ref: Option<String>,
    /// Target name (`data-name`)
    pub name: Option<String>,
}

/// Global map state
struct MapState {
    /// Map pane
    map_pane: MapPane,
    /// Pointerdown handler
    pointerdown: Closure<dyn Fn(PointerEvent)>,
    /// Pointerup (and pointercancel) handler
    pointerup: Closure<dyn Fn(PointerEvent)>,
    /// Pointermove handler
    pointermove: Closure<dyn Fn(PointerEvent)>,
    /// Contextmenu handler
    contextmenu: Closure<dyn Fn(PointerEvent)>,
    /// Wheel handler
    wheel: Closure<dyn Fn(WheelEvent)>,
    /// Click handler
    click: Closure<dyn Fn(Event)>,
    /// Flag to suppress click (while panning)
    suppress_click: bool,
    /// Origin point (relative to upper-left corner of map pane)
    origin: (i32, i32),
    /// Pan "grab" point
    pan_point: Option<(i32, i32)>,
    /// Current pointer position (client units)
    point: (i32, i32),
    /// North-west peg
    nw: Option<Peg>,
}

thread_local! {
    static MAP_STATE: RefCell<Option<MapState>> = const { RefCell::new(None) };
}

impl MapState {
    /// Make a new map state
    fn new(map_pane: MapPane) -> Self {
        MapState {
            map_pane,
            pointerdown: Closure::new(handle_pointerdown),
            pointerup: Closure::new(handle_pointerup),
            pointermove: Closure::new(handle_pointermove),
            wheel: Closure::new(handle_wheel),
            click: Closure::new(handle_click),
            contextmenu: Closure::new(handle_contextmenu),
            suppress_click: false,
            origin: (0, 0),
            pan_point: None,
            point: (0, 0),
            nw: None,
        }
    }

    /// Set pointer position
    fn set_point(&mut self, point: (i32, i32)) {
        self.point = point;
    }

    /// Start panning
    fn start_panning(&mut self) {
        if self.pan_point.is_none() {
            self.pan_point = Some(self.point);
        }
    }

    /// Stop panning
    fn stop_panning(&mut self) {
        if let Some(pan) = self.pan_point {
            self.origin = (
                self.origin.0 + self.point.0 - pan.0,
                self.origin.1 + self.point.1 - pan.1,
            );
            self.pan_point = None;
            self.set_style("");
        }
    }

    /// Get translated pointer position
    fn translated_point(&self) -> (i32, i32) {
        match self.pan_point {
            Some(pan) => (
                self.origin.0 + self.point.0 - pan.0,
                self.origin.1 + self.point.1 - pan.1,
            ),
            None => self.origin,
        }
    }

    /// Reset the map state
    fn reset(&mut self, origin: (i32, i32)) {
        self.map_pane.next_cycle();
        self.origin = origin;
        self.pan_point = None;
        self.point = (0, 0);
        self.set_style("");
    }

    /// Drag (pan) map to a position
    fn drag_map(&mut self, point: (i32, i32)) {
        if self.pan_point.is_some() {
            self.set_point(point);
            self.set_style("grabbing");
            self.map_pane.fetch_edge_tiles();
        }
    }

    /// Set map pane style
    fn set_style(&self, cursor: &str) {
        let (x, y) = self.translated_point();
        let mut css = String::with_capacity(80);
        css.push_str("transform: translate(");
        css.push_str(&x.to_string());
        css.push_str("px, ");
        css.push_str(&y.to_string());
        css.push_str("px);");
        if !cursor.is_empty() {
            css.push_str(" cursor: ");
            css.push_str(cursor);
            css.push(';');
        }
        self.map_pane.set_style(&css);
    }

    /// Change zoom level
    fn change_zoom(&self, zoom: i32, rx: i32, ry: i32) {
        if let Some(nw) = self.nw {
            let zoom = zoom as u32;
            let point = (rx - self.origin.0, ry - self.origin.1);
            let px = nw.x().saturating_add((point.0 / 255) as u32);
            let py = nw.y().saturating_add((point.1 / 256) as u32);
            if let Some(peg) = Peg::new(nw.z(), px, py) {
                let x = (point.0 % 256) as f64 / 256.0;
                let y = (point.1 % 256) as f64 / 256.0;
                let map_pane = self.map_pane.clone();
                let zoom_handler = map_pane.zoom_handler;
                let bbox = map_pane.grid().peg_bbox(peg);
                let width = bbox.x_span();
                let height = bbox.y_span();
                // dimensions in WebMercator
                let mx = bbox.x_min() + x * width;
                let my = bbox.y_max() - y * height;
                let pos = WebMercatorPos::new(mx, my);
                let loc = Wgs84Pos::from(pos);
                let (rx, ry) = map_pane.client_pos(rx, ry);
                map_pane.set_position_anchor(
                    zoom,
                    loc.lon_deg(),
                    loc.lat_deg(),
                    rx,
                    ry,
                );
                (zoom_handler)(zoom);
            }
        }
    }
}

/// Handle a `pointerdown` event
fn handle_pointerdown(pe: PointerEvent) {
    if pe.button() == 0 {
        let point = (pe.client_x(), pe.client_y());
        set_pan_point(true, point);
        if let Some(target) = pe.target()
            && let Ok(elem) = target.dyn_into::<Element>()
            && let Err(e) = elem.set_pointer_capture(0)
        {
            log::warn!("set_pointer_capture: {e:?}");
        }
    }
}

/// Handle a `pointerup` or `pointercancel` event
fn handle_pointerup(pe: PointerEvent) {
    if pe.button() == 0 {
        let point = (pe.client_x(), pe.client_y());
        set_pan_point(false, point);
    }
}

/// Handle a `pointermove` event
fn handle_pointermove(pe: PointerEvent) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            let point = (pe.client_x(), pe.client_y());
            state.drag_map(point);
            state.suppress_click = true;
        }
    });
}

/// Handle a `contextmenu` event
fn handle_contextmenu(pe: PointerEvent) {
    MAP_STATE.with(|rc| {
        if let Some(ref state) = *rc.borrow() {
            pe.prevent_default();
            let x = pe.client_x();
            let y = pe.client_y();
            if let Some(target) = Target::from_pointer_event(pe) {
                (state.map_pane.contextmenu_handler)(target, x, y);
            }
        }
    });
}

impl Target {
    /// Get target data from target element
    fn new(target: Element) -> Option<Self> {
        if let Ok(Some(el)) = target.closest("g,path")
            && let Some(cls) = el.get_attribute("class")
            && let Some(parent) = el.parent_element()
            && let Some(pcls) = parent.get_attribute("class")
            && let Some(("wyrm", layer)) = pcls.split_once('-')
        {
            let layer = layer.to_string();
            let osm_ref = el.get_attribute("data-ref");
            let name = el.get_attribute("data-name");
            return Some(Target {
                cls,
                layer,
                osm_ref,
                name,
            });
        }
        None
    }

    /// Get target data from event
    fn from_event(ev: Event) -> Option<Self> {
        match ev.target().map(|e| e.dyn_into::<Element>()) {
            Some(Ok(target)) => Target::new(target),
            _ => None,
        }
    }

    /// Get target data from pointer event
    fn from_pointer_event(pe: PointerEvent) -> Option<Self> {
        match pe.target().map(|e| e.dyn_into::<Element>()) {
            Some(Ok(target)) => Target::new(target),
            _ => None,
        }
    }
}

/// Handle a `wheel` event
fn handle_wheel(we: WheelEvent) {
    let delta_zoom: i32 = if we.delta_y() < 0.0 {
        1
    } else if we.delta_y() > 0.0 {
        -1
    } else {
        0
    };
    if delta_zoom != 0 {
        MAP_STATE.with(|rc| {
            if let Some(ref mut state) = *rc.borrow_mut() {
                let zoom = state
                    .nw
                    .map(|peg| peg.z() as i32 + delta_zoom)
                    .unwrap_or(0);
                if zoom > 0 {
                    state.change_zoom(zoom, we.client_x(), we.client_y());
                }
            }
        });
    }
}

/// Handle a `click` event
fn handle_click(ev: Event) {
    MAP_STATE.with(|rc| {
        if let Some(ref state) = *rc.borrow()
            && !state.suppress_click
            && let Some(target) = Target::from_event(ev)
        {
            (state.map_pane.click_handler)(target);
        }
    });
}

/// Register map pane state
pub fn register(map_pane: MapPane) -> Result<()> {
    let mp = lookup_id(map_pane.id())?;
    MAP_STATE.with(|rc| {
        let mut state = rc.borrow_mut();
        if state.is_some() {
            // FIXME: allow multiple map panes?
            return Err(Error::Other("init: state exists!"));
        }
        let ms = MapState::new(map_pane);
        mp.add_event_listener_with_callback(
            "pointerdown",
            ms.pointerdown.as_ref().unchecked_ref(),
        )?;
        mp.add_event_listener_with_callback(
            "pointerup",
            ms.pointerup.as_ref().unchecked_ref(),
        )?;
        mp.add_event_listener_with_callback(
            "pointercancel",
            ms.pointerup.as_ref().unchecked_ref(),
        )?;
        mp.add_event_listener_with_callback(
            "pointermove",
            ms.pointermove.as_ref().unchecked_ref(),
        )?;
        mp.add_event_listener_with_callback(
            "contextmenu",
            ms.contextmenu.as_ref().unchecked_ref(),
        )?;
        mp.add_event_listener_with_callback(
            "wheel",
            ms.wheel.as_ref().unchecked_ref(),
        )?;
        mp.add_event_listener_with_callback(
            "click",
            ms.click.as_ref().unchecked_ref(),
        )?;
        *state = Some(ms);
        Ok(())
    })
}

/// Set map pan point
fn set_pan_point(start: bool, point: (i32, i32)) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            if start {
                state.suppress_click = false;
                state.set_point(point);
                state.start_panning();
            } else {
                state.stop_panning();
                // FIXME: remove unused tiles (garbage collect)
            }
        }
    });
}

/// Reset map pane state
pub fn reset(origin: (i32, i32), nw: Peg) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            state.reset(origin);
            state.nw = Some(nw);
        }
    })
}

/// Get map pane
pub fn map_pane(_id: &str) -> Option<MapPane> {
    // FIXME: use id to select from multiple map panes
    MAP_STATE
        .with(|rc| (*rc.borrow()).as_ref().map(|state| state.map_pane.clone()))
}

/// Get current zoom level
pub fn zoom() -> u32 {
    MAP_STATE.with(|rc| {
        if let Some(ref state) = *rc.borrow() {
            state.nw.map(|peg| peg.z()).unwrap_or(0)
        } else {
            0
        }
    })
}

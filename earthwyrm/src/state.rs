// Copyright (C) 2026  Minnesota Department of Transportation
//
use crate::error::{Error, Result};
use crate::map::MapPane;
use crate::util::lookup_id;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::UnwrapThrowExt;
use web_sys::{Element, PointerEvent};

/// Global map state
struct MapState {
    /// Map pane
    map_pane: MapPane,
    /// Pointerdown callback
    pointerdown: Closure<dyn Fn(PointerEvent)>,
    /// Pointerup (and pointercancel) callback
    pointerup: Closure<dyn Fn(PointerEvent)>,
    /// Pointermove callback
    pointermove: Closure<dyn Fn(PointerEvent)>,
    /// Origin point
    origin: (i32, i32),
    /// Pan "grab" point
    pan_point: Option<(i32, i32)>,
    /// Current pointer position (client units)
    point: (i32, i32),
}

thread_local! {
    static MAP_STATE: RefCell<Option<MapState>> = const { RefCell::new(None) };
}

impl MapState {
    /// Make a new map state
    fn new(map_pane: MapPane) -> Self {
        MapState {
            map_pane,
            pointerdown: Closure::new(handle_map_pointerdown),
            pointerup: Closure::new(handle_map_pointerup),
            pointermove: Closure::new(handle_map_pointermove),
            origin: (0, 0),
            pan_point: None,
            point: (0, 0),
        }
    }

    /// Set pointer position
    fn set_point(&mut self, x: i32, y: i32) {
        self.point = (x, y);
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
    fn reset(&mut self) {
        self.map_pane.next_cycle();
        self.origin = (0, 0);
        self.pan_point = None;
        self.point = (0, 0);
    }

    /// Drag (pan) map to a position
    fn drag_map(&mut self, x: i32, y: i32) {
        if self.pan_point.is_some() {
            self.set_point(x, y);
            self.set_style("grabbing");
            // FIXME: load edge tiles if necessary
            // FIXME: remove unused tiles (garbage collect)
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
}

/// Handle a `pointerdown` event
fn handle_map_pointerdown(pe: PointerEvent) {
    if pe.button() == 0 {
        set_pan_point(true, pe.client_x(), pe.client_y());
        if let Some(target) = pe.target()
            && let Ok(elem) = target.dyn_into::<Element>()
            && let Err(e) = elem.set_pointer_capture(0)
        {
            log::warn!("set_pointer_capture: {e:?}");
        }
    }
}

/// Handle a `pointerup` or `pointercancel` event
fn handle_map_pointerup(pe: PointerEvent) {
    if pe.button() == 0 {
        set_pan_point(false, pe.client_x(), pe.client_y());
    }
}

/// Handle a `pointermove` event
fn handle_map_pointermove(pe: PointerEvent) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            let (x, y) = (pe.client_x(), pe.client_y());
            state.drag_map(x, y);
        }
    });
}

/// Initialize map state
///
/// - `id`: HTML `id` attribute of map element
/// - `groups`: Layer group tile names
pub fn init(id: &str, groups: &'static [&'static str]) -> Result<()> {
    let mp = lookup_id(id)?;
    let map_pane = MapPane::new(id, groups);
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
        )
        .unwrap_throw();
        mp.add_event_listener_with_callback(
            "pointerup",
            ms.pointerup.as_ref().unchecked_ref(),
        )
        .unwrap_throw();
        mp.add_event_listener_with_callback(
            "pointercancel",
            ms.pointerup.as_ref().unchecked_ref(),
        )
        .unwrap_throw();
        mp.add_event_listener_with_callback(
            "pointermove",
            ms.pointermove.as_ref().unchecked_ref(),
        )
        .unwrap_throw();
        *state = Some(ms);
        Ok(())
    })
}

/// Set map pan point
fn set_pan_point(start: bool, x: i32, y: i32) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            if start {
                state.set_point(x, y);
                state.start_panning();
            } else {
                state.stop_panning();
            }
        }
    });
}

/// Reset map pane state
pub fn reset() {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            state.reset();
        }
    })
}

/// Get map pane
pub fn map_pane() -> Option<MapPane> {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            Some(state.map_pane.clone())
        } else {
            None
        }
    })
}

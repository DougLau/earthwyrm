// Copyright (C) 2026  Minnesota Department of Transportation
//
use crate::error::{Error, Result};
use crate::map::MapPane;
use crate::util::lookup_id;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::UnwrapThrowExt;
use web_sys::{Element, Event, PointerEvent};

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
    /// Click handler
    click: Closure<dyn Fn(Event)>,
    /// Click callback
    click_cb: Box<dyn Fn(Event)>,
    /// Flag to suppress click (while panning)
    suppress_click: bool,
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
    fn new(map_pane: MapPane, click_cb: impl Fn(Event) + 'static) -> Self {
        MapState {
            map_pane,
            pointerdown: Closure::new(handle_pointerdown),
            pointerup: Closure::new(handle_pointerup),
            pointermove: Closure::new(handle_pointermove),
            click: Closure::new(handle_click),
            click_cb: Box::new(click_cb),
            suppress_click: false,
            origin: (0, 0),
            pan_point: None,
            point: (0, 0),
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

/// Handle a `click` event
fn handle_click(e: Event) {
    MAP_STATE.with(|rc| {
        if let Some(ref state) = *rc.borrow()
            && !state.suppress_click
        {
            (state.click_cb)(e);
        }
    });
}

/// Initialize map state
///
/// - `id`: HTML `id` attribute of map element
/// - `groups`: Layer group tile names
/// - `click_cb`: Click callback
pub fn init(
    id: &str,
    groups: &'static [&'static str],
    click_cb: impl Fn(Event) + 'static,
) -> Result<()> {
    let mp = lookup_id(id)?;
    let map_pane = MapPane::new(id, groups);
    MAP_STATE.with(|rc| {
        let mut state = rc.borrow_mut();
        if state.is_some() {
            // FIXME: allow multiple map panes?
            return Err(Error::Other("init: state exists!"));
        }
        let ms = MapState::new(map_pane, click_cb);
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
        mp.add_event_listener_with_callback(
            "click",
            ms.click.as_ref().unchecked_ref(),
        )
        .unwrap_throw();
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
pub fn reset(origin: (i32, i32)) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            state.reset(origin);
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

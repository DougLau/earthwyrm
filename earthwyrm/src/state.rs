// Copyright (C) 2026  Minnesota Department of Transportation
//
use crate::error::{Error, Result};
use crate::map::MapPane;
use crate::tile::{TileKey, WyrmCache};
use crate::util::lookup_id;
use futures_util::stream::FuturesUnordered;
use squarepeg::Peg;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, MouseEvent, PointerEvent, WheelEvent};

/// Map event for [click] and [contextmenu] handling
///
/// [click]: struct.MapPane.html#method.with_click_handler
/// [contextmenu]: struct.MapPane.html#method.with_contextmenu_handler
#[derive(Clone, Debug)]
pub struct MapEvent {
    /// Target class
    pub target: String,
    /// Map layer name
    pub layer: String,
    /// Target `data-ref` attribute
    pub data_ref: Option<String>,
    /// Target `data-name` attribute
    pub data_name: Option<String>,
    /// Alt key state
    pub alt_key: bool,
    /// Shift key state
    pub shift_key: bool,
    /// Meta key state
    pub meta_key: bool,
    /// Ctrl key state
    pub ctrl_key: bool,
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
    click: Closure<dyn Fn(MouseEvent)>,
    /// Flag to suppress click (while panning)
    suppress_click: bool,
    /// Tile cache
    cache: WyrmCache,
}

thread_local! {
    static MAP_STATE: RefCell<Option<MapState>> = const { RefCell::new(None) };
}

impl MapState {
    /// Make a new map state
    fn new(map_pane: MapPane) -> Self {
        let groups = map_pane.groups();
        MapState {
            map_pane,
            pointerdown: Closure::new(handle_pointerdown),
            pointerup: Closure::new(handle_pointerup),
            pointermove: Closure::new(handle_pointermove),
            wheel: Closure::new(handle_wheel),
            click: Closure::new(handle_click),
            contextmenu: Closure::new(handle_contextmenu),
            suppress_click: false,
            cache: WyrmCache::new(groups),
        }
    }
}

/// Handle a `pointerdown` event
fn handle_pointerdown(pe: PointerEvent) {
    if pe.button() == 0 {
        let point = (pe.client_x(), pe.client_y());
        set_grab_point(true, point);
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
        set_grab_point(false, point);
    }
}

/// Handle a `pointermove` event
fn handle_pointermove(pe: PointerEvent) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            let point = (pe.client_x(), pe.client_y());
            state.map_pane.drag_map(point);
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
            if let Ok(me) = pe.dyn_into::<MouseEvent>()
                && let Some(target) = MapEvent::from_mouse_event(me)
            {
                (state.map_pane.contextmenu_handler)(target, x, y);
            }
        }
    });
}

impl MapEvent {
    /// Get event data from mouse event
    fn from_mouse_event(me: MouseEvent) -> Option<Self> {
        if let Some(Ok(target)) = me.target().map(|e| e.dyn_into::<Element>())
            && let Ok(Some(el)) = target.closest("g,path")
            && let Some(target) = el.get_attribute("class")
            && let Some(parent) = el.parent_element()
            && let Some(pcls) = parent.get_attribute("class")
            && let Some(("wyrm", layer)) = pcls.split_once('-')
        {
            return Some(MapEvent {
                target,
                layer: layer.to_string(),
                data_ref: el.get_attribute("data-ref"),
                data_name: el.get_attribute("data-name"),
                alt_key: me.alt_key(),
                ctrl_key: me.ctrl_key(),
                meta_key: me.meta_key(),
                shift_key: me.shift_key(),
            });
        }
        None
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
            if let Some(ref state) = *rc.borrow() {
                state.map_pane.clone().change_zoom(
                    delta_zoom,
                    we.client_x(),
                    we.client_y(),
                );
            }
        });
    }
}

/// Handle a `click` event
fn handle_click(me: MouseEvent) {
    MAP_STATE.with(|rc| {
        if let Some(ref state) = *rc.borrow()
            && !state.suppress_click
            && let Some(target) = MapEvent::from_mouse_event(me)
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

/// Set map grab (pan) point
fn set_grab_point(start: bool, point: (i32, i32)) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            if start {
                state.suppress_click = false;
                state.map_pane.start_panning(point);
            } else {
                state.map_pane.stop_panning();
            }
        }
    });
}

/// Get map pane
pub fn map_pane(_id: &str) -> Option<MapPane> {
    // FIXME: use id to select from multiple map panes
    MAP_STATE
        .with(|rc| (*rc.borrow()).as_ref().map(|state| state.map_pane.clone()))
}

/// Update map pane state
pub fn update_map(map_pane: MapPane) {
    MAP_STATE.with(|rc| {
        if let Some(ref mut state) = *rc.borrow_mut() {
            state.map_pane = map_pane;
        }
    });
}

/// Make a tile fetcher
pub fn fetcher(
    peg_nw: Peg,
    peg_se: Peg,
) -> FuturesUnordered<impl Future<Output = (TileKey, String)> + use<>> {
    MAP_STATE.with(|rc| {
        let state = rc.borrow();
        let cache = &state.as_ref().unwrap().cache;
        cache.fetcher(peg_nw, peg_se)
    })
}

/// Cache a wyrm tile
pub fn cache_wyrm(key: TileKey, wyrm: String) {
    MAP_STATE.with(|rc| {
        let mut state = rc.borrow_mut();
        let cache = &mut state.as_mut().unwrap().cache;
        cache.insert(key, wyrm);
    });
}

/// Render map from cached tiles
pub fn render_map(peg_nw: Peg, peg_se: Peg) -> String {
    MAP_STATE.with(|rc| {
        let state = rc.borrow();
        let cache = &state.as_ref().unwrap().cache;
        cache.render_map(peg_nw, peg_se)
    })
}

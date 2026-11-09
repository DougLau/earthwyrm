// Copyright (C) 2026  Minnesota Department of Transportation
//
use wasm_bindgen::JsCast;
use web_sys::{Element, MouseEvent};

/// Map event for [click] and [contextmenu] handling
///
/// [click]: struct.MapPane.html#method.with_click_handler
/// [contextmenu]: struct.MapPane.html#method.with_contextmenu_handler
#[derive(Clone, Debug)]
pub struct MapEvent {
    /// Name of map layer
    pub layer: String,
    /// Target class name
    pub class: String,
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

impl MapEvent {
    /// Get event data from mouse event
    pub(crate) fn from_mouse_event(me: MouseEvent) -> Option<Self> {
        if let Some(Ok(target)) = me.target().map(|e| e.dyn_into::<Element>())
            && let Ok(Some(el)) = target.closest("g,path")
            && let Some(class) = el.get_attribute("class")
            && let Some(parent) = el.parent_element()
            && let Some(pcls) = parent.get_attribute("class")
            && let Some(("wyrm", layer)) = pcls.split_once('-')
        {
            return Some(MapEvent {
                layer: layer.to_string(),
                class,
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

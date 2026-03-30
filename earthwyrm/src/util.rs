// Copyright (C) 2026  Douglas Lau
//
use crate::error::{Error, Result};
use web_sys::{Document, Element};

/// Get document
pub fn doc() -> Result<Document> {
    let window = web_sys::window().ok_or(Error::WebSys("no window"))?;
    let doc = window.document().ok_or(Error::WebSys("no document"))?;
    Ok(doc)
}

/// Lookup an element by ID
pub fn lookup_id(id: &str) -> Result<Element> {
    let elem = doc()?
        .get_element_by_id(id)
        .ok_or(Error::WebSys("elem not found"))?;
    Ok(elem)
}

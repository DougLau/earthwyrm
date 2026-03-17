// Copyright (C) 2026  Minnesota Department of Transportation
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
use crate::error::{Error, Result};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use std::borrow::{Borrow, Cow};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

/// Uniform resource identifier
#[derive(Clone, Debug)]
pub struct Uri {
    path: Cow<'static, str>,
}

impl From<&'static str> for Uri {
    fn from(s: &'static str) -> Self {
        Uri {
            path: Cow::Borrowed(s),
        }
    }
}

impl Uri {
    /// Extend Uri with path (will be percent-encoded)
    pub fn push(&mut self, path: &str) {
        let mut p = String::new();
        p.push_str(self.path.borrow());
        if !p.ends_with('/') {
            p.push('/');
        }
        let path = utf8_percent_encode(path, NON_ALPHANUMERIC);
        p.push_str(&path.to_string());
        self.path = Cow::Owned(p);
    }

    /// Add extension to Uri path
    pub fn add_extension(&mut self, ext: &'static str) {
        let mut p = String::new();
        p.push_str(self.path.borrow());
        if !ext.starts_with('.') {
            p.push('.');
        }
        p.push_str(ext);
        self.path = Cow::Owned(p);
    }

    /// Extend Uri with query param/value (will be percent-encoded)
    pub fn query(&mut self, param: &str, value: &str) {
        let mut p = String::new();
        p.push_str(self.path.borrow());
        p.push('?');
        p.push_str(param);
        p.push('=');
        let value = utf8_percent_encode(value, NON_ALPHANUMERIC);
        p.push_str(&value.to_string());
        self.path = Cow::Owned(p);
    }

    /// Get URI as string slice
    pub fn as_str(&self) -> &str {
        self.path.borrow()
    }

    /// Fetch using "GET" method
    pub async fn get(&self) -> Result<String> {
        let resp = get_response(self)
            .await
            .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
        resp_status(resp.status())?;
        let text = resp
            .text()
            .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
        let value = JsFuture::from(text)
            .await
            .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
        value.as_string().ok_or(Error::WebSys("not String"))
    }
}

/// Fetch a GET response
async fn get_response(uri: &Uri) -> Result<Response> {
    let window = web_sys::window().ok_or(Error::WebSys("no window"))?;
    let req = Request::new_with_str(uri.as_str())
        .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
    req.headers()
        .set("Accept", "text/plain")
        .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
    let resp = JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
    resp.dyn_into::<Response>()
        .or(Err(Error::WebSys("dyn_into response")))
}

/// Perform a fetch request
async fn perform_fetch(method: &str, uri: &str) -> Result<Response> {
    let window = web_sys::window().ok_or(Error::WebSys("no window"))?;
    let ri = RequestInit::new();
    ri.set_method(method);
    let headers =
        Headers::new().map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
    headers
        .set("Content-Type", "text/plain")
        .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
    ri.set_headers(&headers);
    let req = Request::new_with_str_and_init(uri, &ri)
        .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
    let resp = JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(|e| Error::FetchRequest(format!("{e:?}")))?;
    resp.dyn_into::<Response>()
        .or(Err(Error::WebSys("dyn_into response")))
}

/// Check for errors in response status code
fn resp_status(sc: u16) -> Result<()> {
    match sc {
        200 | 201 | 202 | 204 => Ok(()),
        401 => Err(Error::FetchResponseUnauthorized()),
        403 => Err(Error::FetchResponseForbidden()),
        404 => Err(Error::FetchResponseNotFound()),
        409 => Err(Error::FetchResponseConflict()),
        _ => Err(Error::FetchResponseOther(sc)),
    }
}

// Copyright (C) 2026  Douglas Lau
//
use crate::error::{Error, Result};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use std::borrow::{Borrow, Cow};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response};

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

    /// Get URI as string slice
    pub fn as_str(&self) -> &str {
        self.path.borrow()
    }

    /// Fetch using "GET" method
    pub async fn get(&self) -> Result<String> {
        let resp = perform_get(self).await?;
        let text = resp
            .text()
            .map_err(|e| Error::FetchReq(format!("text: {e:?}")))?;
        let value = JsFuture::from(text)
            .await
            .map_err(|e| Error::FetchReq(format!("promise: {e:?}")))?;
        value.as_string().ok_or(Error::Other("not String"))
    }
}

/// Perform a GET request
async fn perform_get(uri: &Uri) -> Result<Response> {
    let window = web_sys::window().ok_or(Error::Other("no window"))?;
    let req = Request::new_with_str(uri.as_str())
        .map_err(|e| Error::FetchReq(format!("request: {e:?}")))?;
    req.headers()
        .set("Accept", "text/plain")
        .map_err(|e| Error::FetchReq(format!("headers: {e:?}")))?;
    let resp = JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(|e| Error::FetchReq(format!("fetch: {e:?}")))?;
    let resp = resp
        .dyn_into::<Response>()
        .or(Err(Error::Other("dyn_into response")))?;
    resp_status(resp.status())?;
    Ok(resp)
}

/// Check for errors in response status code
fn resp_status(sc: u16) -> Result<()> {
    match sc {
        200 | 201 | 202 | 204 => Ok(()),
        401 => Err(Error::HttpUnauthorized()),
        403 => Err(Error::HttpForbidden()),
        404 => Err(Error::HttpNotFound()),
        409 => Err(Error::HttpConflict()),
        _ => Err(Error::HttpOther(sc)),
    }
}

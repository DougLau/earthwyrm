// Copyright (C) 2026  Douglas Lau
//
use crate::error::Error;
use crate::fetch::Uri;
use futures_util::Future;
use futures_util::stream::FuturesUnordered;
use squarepeg::Peg;

/// Make tile fetcher
pub fn make_fetcher(
    peg_nw: Peg,
    peg_se: Peg,
    groups: &'static [&'static str],
    cycle: u32,
) -> FuturesUnordered<impl Future<Output = (&'static str, String)>> {
    let futures = FuturesUnordered::new();
    let zoom = peg_nw.z();
    for py in peg_nw.y()..=peg_se.y() {
        for px in peg_nw.x()..=peg_se.x() {
            if let Some(peg) = Peg::new(zoom, px, py) {
                let tx = (px - peg_nw.x()) as i32;
                let ty = (py - peg_nw.y()) as i32;
                for gr in groups {
                    futures.push(fetch_tile(gr, peg, cycle, tx, ty));
                }
            }
        }
    }
    futures
}

/// Fetch one wyrm tile
async fn fetch_tile(
    group: &'static str,
    peg: Peg,
    cycle: u32,
    tx: i32,
    ty: i32,
) -> (&'static str, String) {
    let mut uri = Uri::from("/");
    uri.push(group);
    uri.push(&peg.z().to_string());
    uri.push(&peg.x().to_string());
    uri.push(&peg.y().to_string());
    uri.add_extension(".wyrm");
    let wyrm = match uri.get().await {
        Ok(svg) => svg,
        Err(Error::HttpNotFound()) => String::new(),
        Err(e) => {
            log::warn!("fetch_tile {uri:?} {e:?}");
            String::new()
        }
    };
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
    (group, svg)
}

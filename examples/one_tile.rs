// one_tile.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use earthwyrm::{Error, WyrmCfg};
use postgres::{self, Client, NoTls};
use std::fs::File;

const MUON: &str = &r#"
bind_address:
document_root:
tile_extent: 4096
pixels: 256
buffer_pixels: 5
query_limit: 460000
table: polygon
  db_table: planet_osm_polygon
  id_column: osm_id
  geom_column: way
  geom_type: polygon
layer_group: tile
  layer: city
    table: polygon
    zoom: 1+
    tags: boundary=administrative admin_level=8 ?population
"#;

fn write_tile() -> Result<(), Error> {
    let cfg: WyrmCfg = muon_rs::from_str(MUON)?;
    let layer_group = &cfg.into_layer_groups()?[0];
    let username = whoami::username();
    // Format path for unix domain socket -- not worth using percent_encode
    let uds = format!("postgres://{:}@%2Frun%2Fpostgresql/earthwyrm", username);
    let mut file = File::create("./one_tile.mvt")?;
    let mut conn = Client::connect(&uds, NoTls)?;
    layer_group.write_tile(&mut file, &mut conn, 246, 368, 10)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    write_tile()?;
    Ok(())
}

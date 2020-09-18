// one_tile.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use earthwyrm::{TileId, Wyrm, WyrmCfg};
use postgres::{self, Client, NoTls};
use std::fs::File;

const MUON: &str = &r#"
bind_address:
document_root:
tile_extent: 256
edge_extent: 6
query_limit: 500000
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

fn write_tile() -> Result<(), Box<dyn std::error::Error>> {
    let wyrm_cfg: WyrmCfg = muon_rs::from_str(MUON)?;
    let wyrm = Wyrm::from_cfg(&wyrm_cfg)?;
    let username = whoami::username();
    // Format path for unix domain socket -- not worth using percent_encode
    let uds = format!("postgres://{:}@%2Frun%2Fpostgresql/earthwyrm", username);
    let mut file = File::create("./one_tile.mvt")?;
    let mut conn = Client::connect(&uds, NoTls)?;
    let tid = TileId::new(246, 368, 10)?;
    wyrm.fetch_tile(&mut file, &mut conn, "tile", tid)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    write_tile()?;
    Ok(())
}

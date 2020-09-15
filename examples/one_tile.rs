// one_tile.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use earthwyrm::{Error, TomlCfg};
use postgres::{self, Client, NoTls};
use std::fs::File;

const TOML: &str = &r#"
bind_address = ""
document_root = ""

[[table]]
name = "polygon"
db_table = "planet_osm_polygon"
id_column = "osm_id"
geom_column = "way"
geom_type = "polygon"

[[table]]
name = "line"
db_table = "planet_osm_line"
id_column = "osm_id"
geom_column = "way"
geom_type = "linestring"

[[table]]
name = "roads"
db_table = "planet_osm_roads"
id_column = "osm_id"
geom_column = "way"
geom_type = "linestring"

[[table]]
name = "point"
db_table = "planet_osm_point"
id_column = "osm_id"
geom_column = "way"
geom_type = "point"

[[layer_group]]
base_name = "tile"
rules_path = "./tile.rules"
"#;

fn write_tile() -> Result<(), Error> {
    let maker = &TomlCfg::from_str(TOML)?.into_tile_makers()?[0];
    let username = whoami::username();
    // Format path for unix domain socket -- not worth using percent_encode
    let uds = format!("postgres://{:}@%2Frun%2Fpostgresql/earthwyrm", username);

    let mut file = File::create("./one_tile.mvt")?;
    let mut conn = Client::connect(&uds, NoTls)?;
    maker.write_tile(&mut file, &mut conn, 246, 368, 10)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    write_tile()?;
    Ok(())
}

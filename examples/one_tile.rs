// one_tile.rs
//
// Copyright (c) 2019 Minnesota Department of Transportation
//
use earthwyrm::{TomlCfg, Error};
use postgres::{self, Connection, TlsMode};

const TOML: &str = & r#"
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
    let conn = Connection::connect(uds, TlsMode::None)?;
    maker.write_tile(&conn, 246, 368, 10)
}

fn main() {
    write_tile().unwrap();
}

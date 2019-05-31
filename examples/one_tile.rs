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
rules_path = "./earthwyrm.rules"
"#;

fn write_tile() -> Result<(), Error> {
    if let Some(username) = users::get_current_username() {
        let maker = &TomlCfg::from_str(TOML)?.into_tile_makers()?[0];

        // build path for unix domain socket
        let mut db_url = "postgres://".to_string();
        db_url.push_str(&username);
        // not worth using percent_encode
        db_url.push_str("@%2Frun%2Fpostgresql/earthwyrm");
        let conn = Connection::connect(db_url, TlsMode::None)?;
        maker.write_tile(&conn, 246, 368, 10)
    } else {
        Err(Error::Other("User name lookup error".to_string()))
    }
}

fn main() {
    write_tile().unwrap();
}

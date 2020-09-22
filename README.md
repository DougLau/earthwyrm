# EarthWyrm ![Logo](./earthwyrm.svg)

*EarthWyrm* is a Rust library for generating vector tiles in [MVT] format.
It can serve [OpenStreetMap] or other GIS data.

## Links

* [EarthWyrm-warp] map server
* Library [documentation]

## Database tables

GIS data is stored in a PostgreSQL database, using the PostGIS extension.
Each table contains one column containing `point`, `linestring` or `polygon`
data.  The geometry must use Web Mercator (EPSG:3857) projection.  The
`osm2pgsql` tool creates tables in the proper format.

[documentation]: https://docs.rs/earthwyrm
[EarthWyrm-warp]: earthwyrm-warp/
[MVT]: https://github.com/mapbox/vector-tile-spec
[OpenStreetMap]: https://www.openstreetmap.org/about

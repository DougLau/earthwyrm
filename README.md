# EarthWyrm ![Logo](./earthwyrm.svg)

*EarthWyrm* is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve [OpenStreetMap] or other GIS data in
[MVT] format.

EarthWyrm uses the pervasive [Web Mercator] projection (EPSG:3857), along with
the typical `Z/X/Y.mvt` tile naming convention.

## Setup

* Install linux and dependencies (available from linux repositories):
  - **PostgreSQL 9+**
  - **PostGIS 2.4+**
  - **osm2pgsql 0.96+**
  - **cargo 1.31+**
  - **nginx** (optional)

* Download OpenStreetMap data in _.osm.pbf_ (OSM protobuf) format.  See the
  [OSM wiki] for download options, such as [Geofabrik].

* Create database and import data
```
createdb earthwyrm
psql earthwyrm -c 'CREATE EXTENSION postgis'
time osm2pgsql -v --number-processes=8 -d earthwyrm --multi-geometry -s --drop ./[map-data].osm.pbf
```

* Grant select permissions to public
```
psql earthwyrm
GRANT SELECT ON planet_osm_polygon TO PUBLIC;
GRANT SELECT ON planet_osm_line TO PUBLIC;
GRANT SELECT ON planet_osm_roads TO PUBLIC;
GRANT SELECT ON planet_osm_point TO PUBLIC;
\q
```

* Configure PostgreSQL access for UNIX domain sockets

Edit `/var/lib/pgsql/data/pg_hba.conf`: change _local_ method to _peer_.
Restart PostgreSQL server.

* Build earthwyrm
```
git clone https://github.com/DougLau/earthwyrm.git
cd earthwyrm
cargo build --release
```

* Install (as root)
```
cp target/release/earthwyrm /usr/local/bin/
sh ./examples/site/install.sh
```

* Start the server
```
systemctl enable earthwyrm
systemctl start earthwyrm
systemctl status earthwyrm
```

## Customizing

By default, *EarthWyrm* will listen on the IPv4 loopback address.  This means
clients from other hosts will not be able to reach the server.  There are a
couple of options:

* Update `bind_address` in `/etc/earthwyrm/earthwyrm.muon`
* (Preferred option!)  Set up a reverse proxy, such as [nginx].  This has the
  advantage that caching can be enabled to improve latency.

In either case, the url in `/var/local/earthwyrm/static/map.js` will need to be
updated.

[Geofabrik]: http://download.geofabrik.de/
[MVT]: https://github.com/mapbox/vector-tile-spec
[nginx]: https://nginx.org/en/
[OpenStreetMap]: https://www.openstreetmap.org/about
[OSM wiki]: https://wiki.openstreetmap.org/wiki/Downloading_data
[Web Mercator]: https://en.wikipedia.org/wiki/Web_Mercator_projection

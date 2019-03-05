# EarthWyrm ![Logo](./logo.svg)

*EarthWyrm* is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve
[OpenStreetMap](https://www.openstreetmap.org/about) (and other) data in
[MVT](https://github.com/mapbox/vector-tile-spec) format.

## Setup

* Install linux and dependencies (available from linux repositories):
  - **PostgreSQL 9+**
  - **PostGIS 2.4+**
  - **osm2pgsql 0.96+**
  - **cargo 1.31+**
  - **nginx** (optional)

* Download OpenStreetMap data in _.osm.pbf_ (OSM protobuf) format.  See the
  [OSM wiki](https://wiki.openstreetmap.org/wiki/Downloading_data) for download
  options, such as [Geofabrik](http://download.geofabrik.de/).

* Create database and import data
```
createdb earthwyrm
psql earthwyrm -c 'CREATE EXTENSION postgis'
time osm2pgsql -v --number-processes=8 -d earthwyrm --multi-geometry -s --drop ./[map-data].osm.pbf
```

* Build and install
```
git clone https://github.com/DougLau/earthwyrm.git
cd earthwyrm
cargo build --release
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

* Update *bind_address* in /etc/earthwyrm/earthwyrm.toml
* (Preferred option!)  Set up a reverse proxy, such as
  [nginx](https://nginx.org/en/).  This has the advantage that caching can be
  enabled to improve latency.

In either case, the url in /var/lib/earthwyrm/map.js will need to be updated.

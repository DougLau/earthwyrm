# EarthWyrm

EarthWyrm is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve OpenStreetMap (or other) data in
[MVT](https://github.com/mapbox/vector-tile-spec) format.

## Setup

* Install linux with dependencies:  (available from linux repositories)
  - **PostgreSQL 9+**
  - **PostGIS 2.4+**
  - **osm2pgsql 0.96+**
  - **cargo 1.31+**

* Download OpenStreetMap data in _.osm.pbf_ (OSM protobuf) format.  See the
  [OSM wiki](https://wiki.openstreetmap.org/wiki/Downloading_data) for download
  options, such as [Geofabrik](http://download.geofabrik.de/).

* Create database and import data
```
createdb earthwyrm
psql earthwyrm -c 'CREATE EXTENSION postgis'
time osm2pgsql -v --number-processes=8 -d earthwyrm --multi-geometry -s --drop ./[map-data].osm.pbf
```

* Build earthwyrm
```
git clone https://github.com/DougLau/earthwyrm.git
cd earthwyrm
cargo build --release
```

* Install earthwyrm
```
cp target/release/earthwyrm /usr/local/bin/
mkdir /etc/earthwyrm
cp examples/site/earthwyrm.toml /etc/
cp examples/site/earthwyrm.rules /etc/
```

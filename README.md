# EarthWyrm

EarthWyrm is a vector tile server for openstreetmap data.


## Setup

### Install dependencies

These are available in linux repositories:

* PostgreSQL 9+
* PostGIS 2.4+
* osm2pgsql 0.96+

### Download OpenStreetMap data

Map data should be in .osm.pbf (OSM protobuf) format.  See the
[OSM wiki](https://wiki.openstreetmap.org/wiki/Downloading_data) for download
options, such as [Geofabrik](http://download.geofabrik.de/).

### Create database and import data

```
createdb earthwyrm
psql earthwyrm -c 'CREATE EXTENSION postgis'
time osm2pgsql -v --number-processes=8 -d earthwyrm -s --drop --multi-geometry ./[map-data].osm.pbf
```

### Install earthwyrm


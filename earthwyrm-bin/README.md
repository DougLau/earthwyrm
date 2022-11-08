# EarthWyrm ![Logo](../earthwyrm.svg)

*EarthWyrm* is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve [OpenStreetMap] or other GIS data in
[MVT] format.

EarthWyrm uses the pervasive [Web Mercator] projection (EPSG:3857), along with
the typical `Z/X/Y.mvt` tile naming convention.

## Setup

These instructions are for Linux (tested on Fedora)

* Download and build `earthwyrm-bin`

```bash
git clone https://github.com/DougLau/earthwyrm.git
cd earthwyrm/earthwyrm-bin
cargo build --release
```

* Install (as root)

```bash
sh ./static/install.sh
```

This file tree will be created:
```
/var/local/earthwyrm/
├── earthwyrm.muon
├── loam
├── osm
└── static
    ├── index.html
    ├── map.css
    └── map.js
```

* Download OpenStreetMap data for your region in `.osm.pbf` (OSM protobuf)
  format.  See the [OSM wiki] for download options, such as [Geofabrik].

* Put the OSM file in `/var/local/earthwyrm/osm/`

* Run `earthwyrm dig` (as earthwyrm user)

* Start the server
```bash
systemctl enable earthwyrm
systemctl start earthwyrm
systemctl status earthwyrm
```

## Customizing

The configuration file at `/var/local/earthwyrm/earthwyrm.muon` contains
customization instructions.


[Geofabrik]: http://download.geofabrik.de/
[MVT]: https://github.com/mapbox/vector-tile-spec
[OpenStreetMap]: https://www.openstreetmap.org/about
[OSM wiki]: https://wiki.openstreetmap.org/wiki/Downloading_data
[Web Mercator]: https://en.wikipedia.org/wiki/Web_Mercator_projection

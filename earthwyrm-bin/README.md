# EarthWyrm ![Logo](../earthwyrm.svg)

*EarthWyrm* is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve [OpenStreetMap] or other GIS data in
[MVT] format.

EarthWyrm uses the pervasive [Web Mercator] projection (EPSG:3857), along with
the typical `Z/X/Y.mvt` tile naming convention.

## Installation

These instructions are for Linux (tested on Fedora)

```bash
cargo install earthwyrm-bin
```

Then, as root:

```bash
install ~/.cargo/bin/earthwyrm /usr/local/bin/
useradd --system earthwyrm
mkdir /var/local/earthwyrm
chown earthwyrm.earthwyrm /var/local/earthwyrm
sudo -u earthwyrm /usr/local/bin/earthwyrm init
```

This file tree will be created:
```
/var/local/earthwyrm/
├── earthwyrm.muon
├── earthwyrm.service
├── loam
├── osm
└── static
    ├── index.html
    ├── map.css
    └── map.js
```

### Setup

* The configuration file at `/var/local/earthwyrm/earthwyrm.muon` contains
  customization instructions.

* Download OpenStreetMap data for your region in `.osm.pbf` (OSM protobuf)
  format.  See the [OSM wiki] for download options, such as [Geofabrik].

* Put the OSM file in `/var/local/earthwyrm/osm/`

* Run `earthwyrm dig` (as earthwyrm user)


### Configure as systemd service

As root:
```bash
cp /var/local/earthwyrm/earthwyrm.service /etc/systemd/system/
systemctl enable earthwyrm
systemctl start earthwyrm
```


[Geofabrik]: http://download.geofabrik.de/
[MVT]: https://github.com/mapbox/vector-tile-spec
[OpenStreetMap]: https://www.openstreetmap.org/about
[OSM wiki]: https://wiki.openstreetmap.org/wiki/Downloading_data
[Web Mercator]: https://en.wikipedia.org/wiki/Web_Mercator_projection

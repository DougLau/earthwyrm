# EarthWyrm ![Logo](../earthwyrm.svg)

*EarthWyrm* is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve [OpenStreetMap] or other GIS data in
[MVT] format.

EarthWyrm uses the pervasive [Web Mercator] projection (EPSG:3857), along with
the typical `Z/X/Y.mvt` tile naming convention.

## Setup

These instructions are for Linux (tested on Fedora)

* Download and build `earthwyrm-bin`
```
git clone https://github.com/DougLau/earthwyrm.git
cd earthwyrm/earthwyrm-bin
cargo build --release
```

* Install (as root)
```
sh ./static/install.sh
```

* Download OpenStreetMap data in `.osm.pbf` (OSM protobuf) format.  See the
  [OSM wiki] for download options, such as [Geofabrik].

* Put the OSM file in `/var/local/earthwyrm/osm/`

* Run `earthwyrm dig` (as earthwyrm user)

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

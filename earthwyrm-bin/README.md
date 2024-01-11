# ![Logo]

*EarthWyrm* is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve [OpenStreetMap] or other GIS data in
[MVT] format.

EarthWyrm uses the pervasive [Web Mercator] projection (EPSG:3857), with the
`Z/X/Y.mvt` tile naming convention.

## Installation

These instructions have been tested on Fedora Linux.

```bash
cargo install --locked earthwyrm-bin
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
├── loam/
└── osm/
```

Edit the __configuration__ file at `/var/local/earthwyrm/earthwyrm.muon`.  It
contains examples and instructions.

__Download__ an OpenStreetMap extract of your region in [PBF format] into the
`/var/local/earthwyrm/osm/` directory.  For example, files such as
`minnesota-latest.osm.pbf` are provided daily from [Geofabrik].

__Dig__ the configured layers into `.loam` cache files.  This step may take a
long time, depending on the region size.

```bash
sudo -u earthwyrm /usr/local/bin/earthwyrm dig
```

### Configure service

As root:
```bash
cp /var/local/earthwyrm/earthwyrm.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable earthwyrm
systemctl start earthwyrm
```

### Test

From the server host, browse to [127.0.0.1:3030](http://127.0.0.1:3030/)


[Geofabrik]: http://download.geofabrik.de/
[Logo]: https://github.com/DougLau/earthwyrm/earthwyrm.svg
[MVT]: https://github.com/mapbox/vector-tile-spec
[OpenStreetMap]: https://www.openstreetmap.org/about
[PBF format]: https://wiki.openstreetmap.org/wiki/PBF_Format
[Web Mercator]: https://en.wikipedia.org/wiki/Web_Mercator_projection

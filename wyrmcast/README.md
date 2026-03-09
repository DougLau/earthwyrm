*WyrmCast* is an open-source map server developed for the Minnesota Department
of Transportation (MnDOT).  It can serve GIS data from [OpenStreetMap] or other
sources.

GIS data is stored as R-Trees in a [rosewood] file for each layer.  They
contain `point`, `linestring` or `polygon` features, with associated tags.

Features:

- Layers configurable by zoom level
- [Web Mercator] projection (EPSG:3857)
- Vector tiles in [MVT] format, with `Z/X/Y.mvt` naming convention
- Quick setup in under 10 minutes

👉 Install using cargo (tested on Fedora Linux):

```bash
cd
cargo install wyrmcast
sudo bash
«enter password at prompt»
install .cargo/bin/wyrmcast /usr/local/bin/
useradd --system -m -b /var/local wyrmcast
sudo -i -u wyrmcast /usr/local/bin/wyrmcast init
```

This file tree will be created:
```
/var/local/wyrmcast/
├── wyrmcast.muon
├── wyrmcast.service
├── loam/
└── osm/
```

👉 __Edit__ the configuration file at `/var/local/wyrmcast/wyrmcast.muon`.  It
contains examples and instructions.

👉 __Download__ an OpenStreetMap extract of your region in [PBF format] into the
`/var/local/wyrmcast/osm/` directory.  For example, files such as
`minnesota-latest.osm.pbf` are provided daily from [Geofabrik].

👉 __Dig__ the configured layers into `.loam` cache files:

```bash
sudo -i -u wyrmcast /usr/local/bin/wyrmcast dig
```

NOTE: This step may take a while, depending on the region size.

👉 Configure [systemd] service

```bash
cp /var/local/wyrmcast/wyrmcast.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable wyrmcast
systemctl start wyrmcast
```

👉 Test

From the server host, browse to [127.0.0.1:3030](http://127.0.0.1:3030/)


[Geofabrik]: http://download.geofabrik.de/
[MVT]: https://github.com/mapbox/vector-tile-spec
[OpenStreetMap]: https://www.openstreetmap.org/about
[PBF format]: https://wiki.openstreetmap.org/wiki/PBF_Format
[rosewood]: https://docs.rs/rosewood
[systemd]: https://docs.fedoraproject.org/en-US/quick-docs/systemd-understanding-and-administering/
[Web Mercator]: https://en.wikipedia.org/wiki/Web_Mercator_projection

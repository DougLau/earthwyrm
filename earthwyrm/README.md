*EarthWyrm* is a Rust library for generating vector tiles in [MVT] format.
It can serve [OpenStreetMap] or other GIS data.

## Links

* [earthwyrm-bin] map server
* Library [documentation]

## Layers

GIS data is stored as R-Trees in a [rosewood] file for each layer.  They contain
`point`, `linestring` or `polygon` features, with associated tags.  The geometry
uses [Web Mercator] projection (EPSG:3857).


[documentation]: https://docs.rs/earthwyrm
[earthwyrm-bin]: https://github.com/DougLau/earthwyrm/tree/master/earthwyrm-bin/
[MVT]: https://github.com/mapbox/vector-tile-spec
[OpenStreetMap]: https://www.openstreetmap.org/about
[rosewood]: https://docs.rs/rosewood
[Web Mercator]: https://en.wikipedia.org/wiki/Web_Mercator_projection

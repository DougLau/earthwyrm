*EarthWyrm* is a Rust library for displaying vector tile maps in a WebAssembly
context.

Wyrm tiles are geographic vector maps designed for fluid browser interactivity.
A single tile is a "snippet" or fragment of [SVG] designed to be inserted into
a web page's DOM with no modification.

Geographic coordinates for all map features are converted to [Web Mercator].

Tiles are named using a [ZXY] naming scheme, with a `wyrm` file extension
(e.g. `https://example.com/wyrm/12/990/1450.wyrm`).

Tiles are square, scaled to 256x256 units, cropped at a margin of 8 around
each edge.  Path coordinates are rounded to the nearest integer.

_Example tile_: `14/3944/5895.wyrm`

```html
<g class="wyrm-county">
  <path class="osm-1795848"
        data-name="Hennepin County"
        data-population="1223149"
        d="m-8 -8h272v272h-272v-272z" />
</g>
<g class="wyrm-city">
  <path class="osm-136712"
        data-name="Minneapolis"
        data-population="429954"
        d="m129 264v-109l1 -102l118 -1v-59l16 -1v272h-136z" />
  <path class="osm-136699"
        data-name="Golden Valley"
        data-population="19921"
        d="m-8 -8l256 1v59l-118 1l-1 102v73h-34l-39 -1h-64v-235z" />
  <path class="osm-136701"
        data-name="Saint Louis Park"
        data-population="50010"
        d="m-8 227h64l39 1h34v36h-137v-155z" />
</g>
```

Layers are styled with standard CSS.

* Library [documentation]
* [WyrmCast] map server


[documentation]: https://docs.rs/earthwyrm
[SVG]: https://en.wikipedia.org/wiki/SVG
[Web Mercator]: https://en.wikipedia.org/wiki/Web_Mercator
[WyrmCast]: https://github.com/DougLau/earthwyrm/tree/master/wyrmcast/
[ZXY]: https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames

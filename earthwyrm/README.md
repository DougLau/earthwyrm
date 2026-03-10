*EarthWyrm* is a Rust library for displaying vector tiles in WebAssembly
contexts.

Wyrm tiles are geographic vector maps designed for fluid browser interactivity.
A single tile is a "snippet" or fragment of [SVG] designed to be inserted into
a web page's DOM with no modification.

Geographic coordinates for all map features are converted to [Web Mercator].

Tiles are named using a [ZXY] naming scheme, with a `wyrm` file extension
(e.g. `https://example.com/wyrm/12/990/1450.wyrm`).

Tiles are square and scaled to 256x256 pixels.  Path element coordinates are
rounded to the nearest integer.

_Example Wyrm tile_

```html
<g class="wyrm-123" data-name="Landmark X">
  <path d="M-8 108L-8 -8L264 -8L264 264L-8 264L-8 108L-8 108z" />
</g>
<g class="wyrm-321 wyrm-city" data-name="Place Name">
  <path d="M88 180L88 179L87 179L87 177L86 177L88 177L88 180L88 180z" />
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

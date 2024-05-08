function init_map() {
    let map = L.map('mapid', {
        center: [45, -93],
        zoom: 12,
    });
    const url = "http://127.0.0.1:3030/tile/{z}/{x}/{y}.mvt";
    const highlight_style = {
        fill: true,
        fillColor: 'red',
        fillOpacity: 0.1,
        radius: 6,
        color: 'red',
        opacity: 0.1,
    };
    let boundary = {
        fill: true,
        fillOpacity: 0.2,
        weight: 0.1,
        color: '#000',
        opacity: 0.6,
    };
    let water = {
        fill: true,
        fillOpacity: 0.8,
        fillColor: "#b5d0d0",
        stroke: false,
    };
    let wetland = {
        fill: true,
        fillOpacity: 0.8,
        fillColor: "#b8d0bd",
        stroke: false,
    };
    let leisure = {
        fill: true,
        fillOpacity: 0.6,
        fillColor: "#88cc88",
        weight: 0.1,
        color: '#000',
        opacity: 0.6,
    };
    let cemetery = {
        fill: true,
        fillOpacity: 0.6,
        fillColor: "#aaccaa",
        weight: 0.1,
        color: '#000',
        opacity: 0.6,
    };
    let building = {
        fill: true,
        fillOpacity: 0.7,
        fillColor: "#bca9a9",
        weight: 0.7,
        color: "#baa",
    };
    let retail = {
        fill: true,
        fillOpacity: 0.25,
        fillColor: "#b99",
        stroke: false,
    };
    let parking = {
        fill: true,
        fillOpacity: 0.6,
        fillColor: "#cca",
        stroke: false,
    };
    let path = {
        color: '#000',
        opacity: 0.5,
        weight: 1,
        dashArray: "1 3",
    };
    let railway = {
        color: '#642',
        opacity: 0.6,
        weight: 2.5,
        lineCap: "butt",
        dashArray: "1 1.5",
    };
    let styles = {
        county: Object.assign(boundary, { fillColor: '#f8f4f2' }),
        city: Object.assign(boundary, { fillColor: '#f1eee8' }),
        lake: water,
        river: water,
        water: water,
        pond: water,
        wetland: wetland,
        leisure: leisure,
        cemetery: cemetery,
        retail: retail,
        motorway: { color: "#ffd9a9", weight: 3 },
        trunk: { color: "#ffe0a9" },
        primary: { color: "#ffeaa9" },
        secondary: { color: "#fff4a9" },
        tertiary: { color: "#ffffa9" },
        road: { color: "#eee", weight: 2 },
        path: path,
        railway: railway,
        building: building,
        parking: parking,
    };
    let options = {
        renderFactory: L.svg.tile,
        interactive: true,
        vectorTileLayerStyles: styles,
        getFeatureId: function(feat) {
            return feat.properties.osm_id;
        },
        attribution: 'Map data © <a href="https://www.openstreetmap.org/">OpenStreetMap</a> contributors, <a href="https://creativecommons.org/licenses/by-sa/2.0/">CC-BY-SA</a>',
        maxNativeZoom: 18,
    };
    var highlight;
    var layers = L.vectorGrid.protobuf(url, options);
    layers.on('click', function(e) {
        var osm_id = e.layer.properties.osm_id;
        var change = (typeof osm_id != "undefined") && (osm_id != highlight);
        if (highlight) {
            layers.resetFeatureStyle(highlight);
            highlight = null;
        }
        if (change) {
            highlight = osm_id;
            layers.setFeatureStyle(highlight, highlight_style);
            var name = e.layer.properties.ref || e.layer.properties.name;
            if (typeof name != "undefined") {
                L.popup({ closeButton: false})
                 .setContent(name)
                 .setLatLng(e.latlng)
                 .openOn(map);
            };
        } else {
            map.closePopup();
        }
        L.DomEvent.stop(e);
    });
    layers.addTo(map);
}

window.onload = init_map;

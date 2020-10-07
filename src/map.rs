// map.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::config::{LayerGroupCfg, TableCfg, WyrmCfg};
use crate::geom::{lookup_geom_type, GeomRow};
use crate::layer::LayerDef;
use crate::Error;
use log::{debug, info, warn};
use mvt::{BBox, GeomType, Layer, MapGrid, Tile, TileId};
use pointy::Transform64;
use postgres::fallible_iterator::FallibleIterator;
use postgres::types::ToSql;
use postgres::Client;
use postgres::Row;
use std::io::Write;
use std::time::Instant;

/// Tile configuration
struct TileCfg {
    /// Tile extent; width and height
    tile_extent: u32,
    /// Extent outside tile edges
    edge_extent: u32,
    /// Query row limit
    query_limit: u32,
    /// Tile ID
    tid: TileId,
    /// Bounding box of tile
    bbox: BBox,
    /// Transform from spatial to tile coordinates
    transform: Transform64,
    /// Tolerance for snapping geometry to grid and simplifying
    tolerance: f64,
}

/// Query definition for one table (id_column, sql, etc)
#[derive(Clone, Debug)]
struct QueryDef {
    /// Table name
    name: String,
    /// ID column
    id_column: String,
    /// Geometry type
    geom_type: GeomType,
    /// SQL query string
    sql: String,
}

/// Group of layers for making tiles
#[derive(Clone)]
pub struct LayerGroup {
    /// Name of group
    name: String,
    /// Layer definitions
    layer_defs: Vec<LayerDef>,
    /// Query definitions
    queries: Vec<QueryDef>,
}

/// Wyrm tile fetcher.
///
/// To create:
/// * Use `serde` to deserialize a [WyrmCfg]
/// * `let wyrm = Wyrm::from_cfg(wyrm_cfg)?;`
///
/// [WyrmCfg]: struct.WyrmCfg.html
#[derive(Clone)]
pub struct Wyrm {
    /// Map grid configuration
    grid: MapGrid,
    /// Tile extent; width and height
    tile_extent: u32,
    /// Extent outside tile edges
    edge_extent: u32,
    /// Query row limit
    query_limit: u32,
    /// Tile layer groups
    groups: Vec<LayerGroup>,
}

impl QueryDef {
    /// Create a new query definition
    fn new(
        table_cfg: &TableCfg,
        srid: i32,
        layer_defs: &[LayerDef],
    ) -> Option<Self> {
        let geom_type = lookup_geom_type(&table_cfg.geom_type)?;
        let tags = QueryDef::table_tags(table_cfg, layer_defs);
        if !tags.is_empty() {
            let name = table_cfg.name.to_string();
            let id_column = table_cfg.id_column.to_string();
            let sql = QueryDef::build_sql(table_cfg, srid, &tags);
            Some(QueryDef {
                name,
                id_column,
                geom_type,
                sql,
            })
        } else {
            None
        }
    }

    /// Get the requested tags for the table from defined layers
    fn table_tags<'a>(
        table_cfg: &TableCfg,
        layer_defs: &'a [LayerDef],
    ) -> Vec<&'a str> {
        let mut tags = vec![];
        for ld in layer_defs {
            if ld.table() == table_cfg.name {
                for pattern in ld.patterns() {
                    let tag = pattern.tag();
                    if !tags.iter().any(|t| *t == tag) {
                        tags.push(tag);
                    }
                }
            }
        }
        tags
    }

    /// Build a SQL query for one table.
    ///
    /// * `table_cfg` Table configuration.
    /// * `srid` Spatial reference ID.
    /// * `tags` Columns to query.
    ///
    /// Query parameters:
    /// * `$1` Simplification tolerance
    /// * `$2` Edge buffer tolerance
    /// * `$3` Zoom level
    /// * `$4` Minimum X
    /// * `$5` Minimum Y
    /// * `$6` Maximum X
    /// * `$7` Maximum Y
    fn build_sql(table_cfg: &TableCfg, srid: i32, tags: &[&str]) -> String {
        let mut sql = "SELECT ".to_string();
        // id_column must be first (#0)
        sql.push_str(&table_cfg.id_column);
        sql.push_str(",ST_Multi(ST_SimplifyPreserveTopology(ST_SnapToGrid(");
        // geom_column must be second (#1)
        sql.push_str(&table_cfg.geom_column);
        sql.push_str(",$1),$1))");
        for tag in tags {
            sql.push_str(",\"");
            sql.push_str(tag);
            sql.push('"');
        }
        sql.push_str(" FROM ");
        sql.push_str(&table_cfg.db_table);
        sql.push_str(" WHERE ");
        match &table_cfg.zoom_column {
            Some(zoom_column) => {
                sql.push('(');
                sql.push_str(zoom_column);
                sql.push_str("=$3::INTEGER OR ");
                sql.push_str(zoom_column);
                sql.push_str(" IS NULL) AND ");
            }
            None => sql.push_str("$3::INTEGER IS NOT NULL AND "),
        }
        sql.push_str(&table_cfg.geom_column);
        sql.push_str(" && ST_Buffer(ST_MakeEnvelope($4,$5,$6,$7,");
        sql.push_str(&srid.to_string());
        sql.push_str("),$2)");
        sql
    }

    /// Query layers for one table
    fn query_layers(
        &self,
        client: &mut Client,
        tile_cfg: &TileCfg,
        layer_group: &LayerGroup,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        debug!("sql: {}", &self.sql);
        let mut trans = client.transaction()?;
        let stmt = trans.prepare(&self.sql)?;
        // Build query parameters
        let tolerance = tile_cfg.tolerance; // $1
        let radius = tolerance * tile_cfg.edge_extent as f64; // $2
        let zoom = tile_cfg.zoom() as i32; // $3
        let x_min = tile_cfg.bbox.x_min(); // $4
        let y_min = tile_cfg.bbox.y_min(); // $5
        let x_max = tile_cfg.bbox.x_max(); // $6
        let y_max = tile_cfg.bbox.y_max(); // $7
        let params: [&(dyn ToSql + Sync); 7] =
            [&tolerance, &radius, &zoom, &x_min, &y_min, &x_max, &y_max];
        debug!("params: {:?}", params);
        let portal = trans.bind(&stmt, &params[..])?;
        let mut remaining_limit = tile_cfg.query_limit;
        while remaining_limit > 0 {
            let before_limit = remaining_limit;
            // Fetch next set of rows from portal
            let mut rows = trans.query_portal_raw(&portal, 50)?;
            while let Some(row) = rows.next()? {
                self.add_layer_features(&row, tile_cfg, layer_group, layers)?;
                remaining_limit -= 1;
            }
            if before_limit == remaining_limit {
                break;
            }
        }
        if remaining_limit == 0 {
            warn!(
                "table {}, query limit reached: {}",
                &self.name, tile_cfg.query_limit
            );
        }
        Ok(())
    }

    /// Add features to layers in group
    fn add_layer_features(
        &self,
        row: &Row,
        tile_cfg: &TileCfg,
        layer_group: &LayerGroup,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        let table = &self.name;
        let grow = GeomRow::new(row, self.geom_type, &self.id_column);
        for layer in layers {
            if let Some(layer_def) = layer_group.find_layer(layer.name()) {
                if layer_def.check_table(table, tile_cfg.zoom())
                    && grow.matches_layer(layer_def)
                {
                    if let Some(geom) =
                        grow.get_geometry(&tile_cfg.transform)?
                    {
                        let lyr = std::mem::take(layer);
                        *layer = grow.add_feature(lyr, layer_def, geom);
                    }
                }
            }
        }
        Ok(())
    }
}

impl TileCfg {
    /// Get the zoom level
    fn zoom(&self) -> u32 {
        self.tid.z()
    }
}

impl LayerGroup {
    /// Build a `LayerGroup`
    fn from_cfg(
        table_cfgs: &[TableCfg],
        srid: i32,
        group_cfg: &LayerGroupCfg,
    ) -> Result<Self, Error> {
        let layer_defs = LayerDef::from_group_cfg(group_cfg)?;
        info!("{} layers in {}", layer_defs.len(), group_cfg);
        let queries = LayerGroup::build_queries(table_cfgs, srid, &layer_defs);
        let name = group_cfg.name.to_string();
        Ok(LayerGroup {
            name,
            layer_defs,
            queries,
        })
    }

    /// Build the queries
    fn build_queries(
        table_cfgs: &[TableCfg],
        srid: i32,
        layer_defs: &[LayerDef],
    ) -> Vec<QueryDef> {
        let mut queries = vec![];
        for table_cfg in table_cfgs {
            if let Some(query_def) = QueryDef::new(table_cfg, srid, layer_defs)
            {
                queries.push(query_def);
            }
        }
        queries
    }

    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Find a layer by name
    fn find_layer(&self, name: &str) -> Option<&LayerDef> {
        self.layer_defs.iter().find(|ld| ld.name() == name)
    }

    /// Create all layers for a tile
    fn create_layers(&self, tile: &Tile) -> Vec<Layer> {
        self.layer_defs
            .iter()
            .map(|ld| tile.create_layer(&ld.name()))
            .collect()
    }

    /// Check one query for matching layers
    fn check_layers(&self, query_def: &QueryDef, zoom: u32) -> bool {
        let table = &query_def.name;
        self.layer_defs.iter().any(|l| l.check_table(table, zoom))
    }

    /// Fetch a tile
    fn fetch_tile(
        &self,
        client: &mut Client,
        tile_cfg: &TileCfg,
    ) -> Result<Tile, Error> {
        let t = Instant::now();
        let tile = self.query_tile(client, tile_cfg)?;
        info!(
            "{} {}, fetched {} bytes in {:?}",
            self.name(),
            tile_cfg.tid,
            tile.compute_size(),
            t.elapsed()
        );
        Ok(tile)
    }

    /// Query one tile from DB
    fn query_tile(
        &self,
        client: &mut Client,
        tile_cfg: &TileCfg,
    ) -> Result<Tile, Error> {
        let mut tile = Tile::new(tile_cfg.tile_extent);
        let mut layers = self.create_layers(&tile);
        for query_def in &self.queries {
            if self.check_layers(query_def, tile_cfg.zoom()) {
                query_def.query_layers(client, tile_cfg, self, &mut layers)?;
            }
        }
        for layer in layers.drain(..) {
            if layer.num_features() > 0 {
                tile.add_layer(layer)?;
            }
        }
        Ok(tile)
    }

    /// Write a tile
    fn write_tile<W: Write>(
        &self,
        out: &mut W,
        client: &mut Client,
        tile_cfg: TileCfg,
    ) -> Result<(), Error> {
        let tile = self.fetch_tile(client, &tile_cfg)?;
        if tile.num_layers() > 0 {
            tile.write_to(out)?;
            Ok(())
        } else {
            debug!("tile {} empty (no layers)", tile_cfg.tid);
            Err(Error::TileEmpty())
        }
    }
}

impl Wyrm {
    /// Create a new Wyrm tile fetcher
    pub fn from_cfg(wyrm_cfg: &WyrmCfg) -> Result<Self, Error> {
        // Only Web Mercator supported for now
        let grid = MapGrid::default();
        let mut groups = vec![];
        for group in &wyrm_cfg.layer_group {
            groups.push(LayerGroup::from_cfg(
                &wyrm_cfg.table,
                grid.srid(),
                group,
            )?);
        }
        Ok(Wyrm {
            grid,
            tile_extent: wyrm_cfg.tile_extent,
            edge_extent: wyrm_cfg.edge_extent,
            query_limit: wyrm_cfg.query_limit,
            groups,
        })
    }

    /// Fetch one tile from a DB client.
    ///
    /// * `out` Writer to write MVT data.
    /// * `client` Postgres database client.
    /// * `group_name` Name of layer group.
    /// * `tid` Tile ID.
    pub fn fetch_tile<W: Write>(
        &self,
        out: &mut W,
        client: &mut Client,
        group_name: &str,
        tid: TileId,
    ) -> Result<(), Error> {
        for group in &self.groups {
            if group_name == group.name() {
                let tile_cfg = self.tile_config(tid);
                return group.write_tile(out, client, tile_cfg);
            }
        }
        debug!("unknown group name: {}", group_name);
        Err(Error::UnknownGroupName())
    }

    /// Create tile config for a tile ID
    fn tile_config(&self, tid: TileId) -> TileCfg {
        let tile_extent = self.tile_extent;
        let bbox = self.grid.tile_bbox(tid);
        let tile_sz = bbox.x_max() - bbox.x_min();
        let tolerance = tile_sz / tile_extent as f64;
        debug!("tile {}, tolerance {:?}", tid, tolerance);
        let ts = tile_extent as f64;
        let transform = self.grid.tile_transform(tid).scale(ts, ts);
        TileCfg {
            tile_extent,
            edge_extent: self.edge_extent,
            query_limit: self.query_limit,
            tid,
            bbox,
            transform,
            tolerance,
        }
    }
}

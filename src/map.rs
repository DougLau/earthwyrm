// map.rs
//
// Copyright (c) 2019-2020  Minnesota Department of Transportation
//
use crate::config::{LayerGroupCfg, TableCfg, WyrmCfg};
use crate::geom::{lookup_geom_type, GeomRow};
use crate::rules::LayerDef;
use crate::Error;
use log::{debug, info, warn};
use mvt::{BBox, GeomType, Layer, MapGrid, Tile, TileId, Transform};
use postgres::fallible_iterator::FallibleIterator;
use postgres::types::ToSql;
use postgres::Client;
use postgres::Row;
use std::io::Write;
use std::time::Instant;

/// Table definition (tags, sql query, etc)
#[derive(Clone, Debug)]
struct TableDef {
    /// Table name
    name: String,
    /// ID column
    id_column: String,
    /// Geometry type
    geom_type: GeomType,
    /// Tag patterns
    tags: Vec<String>,
    /// SQL query string
    sql: String,
}

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
    /// Bounding box
    bbox: BBox,
    /// Geometry transform
    transform: Transform,
    /// Tolerance for snapping geometry to grid and simplifying
    tolerance: f64,
}

/// Group of layers for making tiles
#[derive(Clone)]
pub struct LayerGroup {
    /// Name of group
    name: String,
    /// Layer definitions
    layer_defs: Vec<LayerDef>,
    /// Table definitions
    table_defs: Vec<TableDef>,
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

impl TableDef {
    /// Create a new table definition
    fn new(table_cfg: &TableCfg, layer_defs: &[LayerDef]) -> Option<Self> {
        let name = &table_cfg.name;
        let id_column = table_cfg.id_column.to_string();
        let geom_type = lookup_geom_type(&table_cfg.geom_type)?;
        let tags = TableDef::table_tags(name, layer_defs);
        if tags.len() > 0 {
            let name = name.to_string();
            let sql = TableDef::build_query_sql(table_cfg, &tags);
            Some(TableDef {
                name,
                id_column,
                geom_type,
                tags,
                sql,
            })
        } else {
            None
        }
    }

    /// Get the tags requested for the table from defined layers
    fn table_tags(name: &str, layer_defs: &[LayerDef]) -> Vec<String> {
        let mut tags = Vec::<String>::new();
        for ld in layer_defs {
            if ld.table() == name {
                for pattern in ld.patterns() {
                    let tag = pattern.tag();
                    if !tags.iter().any(|t| t == tag) {
                        tags.push(tag.to_string());
                    }
                }
            }
        }
        tags
    }

    /// Build SQL query.
    ///
    /// * `tags` Columns to query.
    ///
    /// Query parameters:
    /// * `$1` Simplification tolerance
    /// * `$2` Minimum X
    /// * `$3` Minimum Y
    /// * `$4` Maximum X
    /// * `$5` Maximum Y
    /// * `$6` Edge buffer tolerance
    fn build_query_sql(table_cfg: &TableCfg, tags: &Vec<String>) -> String {
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
        sql.push_str(&table_cfg.geom_column);
        sql.push_str(" && ST_Buffer(ST_MakeEnvelope($2,$3,$4,$5,3857),$6)");
        sql
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
        group_cfg: &LayerGroupCfg,
        table_cfgs: &[TableCfg],
    ) -> Result<Self, Error> {
        let layer_defs = LayerDef::from_group_cfg(group_cfg)?;
        info!("{} layers in {}", layer_defs.len(), group_cfg);
        let table_defs = LayerGroup::build_table_defs(&layer_defs, table_cfgs);
        let name = group_cfg.name.to_string();
        Ok(LayerGroup {
            name,
            layer_defs,
            table_defs,
        })
    }

    /// Build the table definitions
    fn build_table_defs(
        layer_defs: &[LayerDef],
        table_cfgs: &[TableCfg],
    ) -> Vec<TableDef> {
        let mut table_defs = vec![];
        for table_cfg in table_cfgs {
            if let Some(table_def) = TableDef::new(table_cfg, layer_defs) {
                table_defs.push(table_def);
            }
        }
        table_defs
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

    /// Check one table for matching layers
    fn check_layers(&self, table_def: &TableDef, zoom: u32) -> bool {
        let table = &table_def.name;
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
        for table_def in &self.table_defs {
            if self.check_layers(table_def, tile_cfg.zoom()) {
                self.query_layers(client, table_def, &mut layers, tile_cfg)?;
            }
        }
        for layer in layers.drain(..) {
            if layer.num_features() > 0 {
                tile.add_layer(layer)?;
            }
        }
        Ok(tile)
    }

    /// Query layers for one table
    fn query_layers(
        &self,
        client: &mut Client,
        table_def: &TableDef,
        layers: &mut Vec<Layer>,
        tile_cfg: &TileCfg,
    ) -> Result<(), Error> {
        debug!("sql: {}", &table_def.sql);
        let mut trans = client.transaction()?;
        let stmt = trans.prepare(&table_def.sql)?;
        let x_min = tile_cfg.bbox.x_min();
        let y_min = tile_cfg.bbox.y_min();
        let x_max = tile_cfg.bbox.x_max();
        let y_max = tile_cfg.bbox.y_max();
        let tolerance = tile_cfg.tolerance;
        let radius = tolerance * tile_cfg.edge_extent as f64;
        let params: Vec<&(dyn ToSql + Sync)> =
            vec![&tolerance, &x_min, &y_min, &x_max, &y_max, &radius];
        debug!("params: {:?}", params);
        let portal = trans.bind(&stmt, &params[..])?;
        let mut remaining_limit = tile_cfg.query_limit;
        while remaining_limit > 0 {
            let before_limit = remaining_limit;
            // Fetch next set of rows from portal
            let mut rows = trans.query_portal_raw(&portal, 50)?;
            while let Some(row) = rows.next()? {
                self.add_layer_features(table_def, &row, tile_cfg, layers)?;
                remaining_limit -= 1;
            }
            if before_limit == remaining_limit {
                break;
            }
        }
        if remaining_limit == 0 {
            warn!(
                "table {}, query limit reached: {}",
                &table_def.name, tile_cfg.query_limit
            );
        }
        Ok(())
    }

    /// Add features to a layer
    fn add_layer_features(
        &self,
        table_def: &TableDef,
        row: &Row,
        tile_cfg: &TileCfg,
        layers: &mut Vec<Layer>,
    ) -> Result<(), Error> {
        let table = &table_def.name;
        let grow = GeomRow::new(row, table_def.geom_type, &table_def.id_column);
        for layer in layers {
            if let Some(layer_def) = self.find_layer(layer.name()) {
                if layer_def.check_table(table, tile_cfg.zoom())
                    && grow.matches_layer(layer_def)
                {
                    if let Some(geom) =
                        grow.get_geometry(&tile_cfg.transform)?
                    {
                        let lyr = std::mem::replace(layer, Layer::default());
                        *layer = grow.add_feature(lyr, layer_def, geom);
                    }
                }
            }
        }
        Ok(())
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
        let grid = MapGrid::default();
        let mut groups = vec![];
        for group in &wyrm_cfg.layer_group {
            groups.push(LayerGroup::from_cfg(group, &wyrm_cfg.table)?);
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
                group.write_tile(out, client, tile_cfg)?;
                return Ok(());
            }
        }
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

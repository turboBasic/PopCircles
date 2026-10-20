// The two commands that are about the table itself rather than a circle over it: building one from a
// raster and publishing it, and querying a rectangle of one already built.

use std::path::Path;

use popcircles::raster::{PixelType, RasterSpec, geotiff::GeoTiffSource};
use popcircles::report::{Envelope, TableBuildReport, TableQueryReport};
use popcircles::table::cache::Cache;
use popcircles::table::{Decimation, Window, build};

use crate::args::{CachedTableArgs, GridArgs, TableArgs};
use crate::commands::{CachedTable, make_room_for, serialised};
use crate::failure::Failure;
use crate::observe::StderrProgress;
use crate::registry::{REGISTRY_PATH, Registry};

pub(crate) fn build_table(
    dataset: &str,
    overrides: GridArgs,
    nodata: Option<f32>,
    epsg: Option<u16>,
    table: &TableArgs,
) -> Result<String, Failure> {
    let row = Registry::load(Path::new(REGISTRY_PATH))
        .and_then(|registry| registry.raster(dataset))
        .map_err(|error| Failure::registry(&error))?;

    let raster = row.raster.as_path();
    let grid = overrides
        .resolve(Some(row.grid))?
        .grid()
        .map_err(|error| Failure::grid(&error))?;
    let decimation =
        Decimation::new(grid, table.decimate).map_err(|error| Failure::table(&error))?;
    let spec = RasterSpec {
        grid,
        epsg: epsg.unwrap_or(row.epsg),
        pixel: PixelType::Float32,
        nodata: nodata.unwrap_or(row.nodata),
    };
    let source = GeoTiffSource::open(raster, &spec).map_err(|error| Failure::raster(&error))?;

    let cache = Cache::new(&table.cache);
    make_room_for(&table.cache)?;

    // Box 6's other half, and the reason it is not `CachedTable::open`'s record: this command opens no
    // cache. After the file was opened rather than before, so the record names a raster that is there.
    log::info!(
        "reading {} at decimation {}",
        raster.display(),
        decimation.factor()
    );

    let mut writer = cache.writer().map_err(|error| Failure::cache(&error))?;
    let mut progress = StderrProgress::new();
    let built = build(source, decimation, &mut progress, |row| {
        writer.write_row(row)
    })
    .map_err(|error| Failure::build(&error))?;
    writer
        .publish(&built, Some(dataset))
        .map_err(|error| Failure::cache(&error))?;
    progress.finish();

    // After `finish`, so the meter's own line is closed rather than written over.
    log::info!(
        "published {} and {}",
        cache.header_path().display(),
        cache.payload_path().display()
    );

    serialised(serde_json::to_string(&Envelope::new(
        TableBuildReport::new(&built, cache.header_path(), cache.payload_path()),
    )))
}

pub(crate) fn query_table(
    cached: &CachedTableArgs,
    window: Option<Window>,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let grid = cached.grid;

    let (rows, cols) = match window {
        Some(window) => view.covering(window).ok_or_else(|| {
            Failure::bad_input(format!(
                "the window is not on a {} x {} grid whose origin is (lat {}, lon {}); a coordinate \
                 on the grid's outer southern or eastern boundary lies in no cell, and the whole \
                 extent is what the query does with no window at all",
                grid.width(),
                grid.height(),
                grid.origin().lat,
                grid.origin().lon
            ))
        })?,
        None => view.whole(),
    };
    let population = view.population(rows, cols);

    // No provenance block, and that is not an omission: this document's own payload carries the digest
    // and the grid, because the table is what the command is *about*. `report`'s module documentation
    // owns that distinction.
    serialised(serde_json::to_string(&Envelope::new(
        TableQueryReport::new(
            cached.identity.digest,
            &grid,
            window,
            rows,
            cols,
            population,
        ),
    )))
}

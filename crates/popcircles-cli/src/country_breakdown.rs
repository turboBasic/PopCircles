use std::path::Path;

use popcircles::circle;
use popcircles::country_breakdown::{
    CountryBreakdown, CountryBreakdownProvenance, CountryContribution,
};
use popcircles::domain::{Domain, FeatureId, Geometry};
use popcircles::geodesy::RadiusKm;
use popcircles::grid::{Col, Grid, Row};
use popcircles::kernel::Kernel;
use popcircles::ownership::CellOwnership;
use popcircles::raster::geotiff::GeoTiffSource;
use popcircles::raster::{CellTallies, RasterError, RasterRow, RasterSource};
use popcircles::table::CountryTable;
use popcircles::table::{ColSpan, CountryTableBuildError, Decimation, build_country_decimated};

use crate::boundaries::{self, BoundaryFeature};
use crate::failure::{EXIT_MISSING_DATA, Failure};
use crate::registry::{COUNTRY_BOUNDARY_DATASET, REGISTRY_PATH, Registry};

pub(crate) struct Prepared {
    source: Grid,
    effective: Grid,
    decimation: Decimation,
    rows: Vec<Vec<f32>>,
    tallies: CellTallies,
    ownership: CellOwnership,
    domains: Vec<Domain>,
    features: Vec<BoundaryFeature>,
    boundary_digest: String,
    boundary_revision: String,
    attribution: String,
}

impl Prepared {
    pub(crate) fn load(
        dataset_name: Option<&str>,
        source: Grid,
        effective: Grid,
        factor: u32,
    ) -> Result<Self, Failure> {
        let registry =
            Registry::load(Path::new(REGISTRY_PATH)).map_err(|error| Failure::registry(&error))?;
        let population = match dataset_name {
            Some(name) => registry.raster(name),
            None => registry.default_raster(),
        }
        .map_err(|error| Failure::registry(&error))?;
        let boundary = registry.boundary(COUNTRY_BOUNDARY_DATASET).ok_or_else(|| {
            Failure::bad_input(format!(
                "country boundary `{COUNTRY_BOUNDARY_DATASET}` is not registered"
            ))
        })?;
        let path = &boundary.path;
        let source_id_field = boundary.source_id_field.as_deref().ok_or_else(|| {
            Failure::bad_input(format!(
                "country boundary `{COUNTRY_BOUNDARY_DATASET}` has no source identifier field"
            ))
        })?;
        let features = boundaries::load(path, source_id_field).map_err(|error| {
            if path.exists() {
                Failure::bad_input(error.to_string())
            } else {
                Failure {
                    code: EXIT_MISSING_DATA,
                    message: format!(
                        "{error}; run `mise run data:get` to fetch the registered country boundary"
                    ),
                }
            }
        })?;
        let geometries: Vec<(FeatureId, Geometry)> = features
            .iter()
            .map(|feature| (feature.id.clone(), feature.geometry.clone()))
            .collect();
        let (ownership, _) = CellOwnership::rasterize_features(&geometries, source)
            .map_err(|error| Failure::bad_input(error.to_string()))?;
        let decimation = Decimation::new(source, factor)
            .map_err(|error| Failure::bad_input(error.to_string()))?;
        if *decimation.grid() != effective {
            return Err(Failure::bad_input(
                "country accounting decimation does not match the searched table",
            ));
        }
        let mut domains = Vec::with_capacity(features.len());
        for feature in &features {
            let (domain, _) = Domain::rasterize(feature.id.clone(), &feature.geometry, effective)
                .map_err(|error| Failure::bad_input(error.to_string()))?;
            domains.push(domain);
        }
        let spec = popcircles::raster::RasterSpec {
            grid: source,
            epsg: population.epsg,
            pixel: popcircles::raster::PixelType::Float32,
            nodata: population.nodata,
        };
        let mut raster = GeoTiffSource::open(&population.raster, &spec)
            .map_err(|error| Failure::raster(&error))?;
        let mut rows = Vec::with_capacity(source.height() as usize);
        while let Some(row) = raster.next_row() {
            rows.push(
                row.map_err(|error| Failure::raster(&error))?
                    .values
                    .to_vec(),
            );
        }
        let tallies = raster.finish();
        let boundary_digest = boundary.sha256.clone().ok_or_else(|| {
            Failure::bad_input(format!(
                "country boundary `{COUNTRY_BOUNDARY_DATASET}` has no registered digest"
            ))
        })?;
        Ok(Self {
            source,
            effective,
            decimation,
            rows,
            tallies,
            ownership,
            domains,
            features,
            boundary_digest,
            boundary_revision: boundary.revision.clone().unwrap_or_default(),
            attribution: boundary.attribution.clone().unwrap_or_default(),
        })
    }

    pub(crate) fn for_circle(
        &self,
        centre: (Row, Col),
        radius: RadiusKm,
        circle_population: f64,
        world_population: f64,
    ) -> Result<CountryBreakdown, Failure> {
        self.for_circles(&[(centre, radius, circle_population)], world_population)
            .map(|mut results| results.remove(0))
    }

    pub(crate) fn country_table(
        &self,
        feature: &BoundaryFeature,
    ) -> Result<Option<CountryTable>, Failure> {
        match build_country_decimated(
            Replay::new(self.source, &self.rows, self.tallies),
            &self.ownership,
            &feature.id,
            self.decimation,
            &self.boundary_digest,
        ) {
            Ok(table) => Ok(Some(table)),
            Err(CountryTableBuildError::NoOwnedCells(_)) => Ok(None),
            Err(error) => Err(Failure::bad_input(error.to_string())),
        }
    }

    pub(crate) fn for_circles(
        &self,
        circles: &[((Row, Col), RadiusKm, f64)],
        world_population: f64,
    ) -> Result<Vec<CountryBreakdown>, Failure> {
        let kernels = circles
            .iter()
            .map(|((row, _), radius, _)| {
                Kernel::new(self.effective, *row, *radius).map_err(Failure::kernel)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut contributions = vec![Vec::with_capacity(self.features.len()); circles.len()];
        let feature_count = self.features.len();
        for (index, (feature, domain)) in self.features.iter().zip(&self.domains).enumerate() {
            log::info!(
                "country accounting {}/{}: {}",
                index + 1,
                feature_count,
                feature.id.as_str()
            );
            let table = self.country_table(feature)?;
            let total = table.as_ref().map_or(0.0, CountryTable::total);
            for (index, (((_, centre), _radius, _), kernel)) in
                circles.iter().zip(&kernels).enumerate()
            {
                let inside = table
                    .as_ref()
                    .map(|table| circle::country_population(table, kernel, *centre))
                    .transpose()
                    .map_err(|error| Failure::bad_input(error.to_string()))?
                    .unwrap_or(0.0);
                let mut intersects = false;
                for placed in kernel
                    .place(*centre)
                    .map_err(|error| Failure::grid_index(&error))?
                {
                    let (row, span) = placed.map_err(|error| Failure::grid_index(&error))?;
                    if domain.row_intervals(row).is_some_and(|intervals| {
                        intervals.iter().any(|interval| match span {
                            ColSpan::FullTurn => true,
                            ColSpan::Through { west, east } if west.get() <= east.get() => {
                                interval.start() <= east.get() && interval.end() > west.get()
                            }
                            ColSpan::Through { west, east } => {
                                interval.start() <= east.get() || interval.end() > west.get()
                            }
                        })
                    }) {
                        intersects = true;
                        break;
                    }
                }
                contributions[index].push(CountryContribution {
                    id: feature.id.clone(),
                    name: feature.name.clone(),
                    inside,
                    total,
                    intersects,
                });
            }
        }
        let provenance = CountryBreakdownProvenance {
            boundary_dataset: COUNTRY_BOUNDARY_DATASET.to_owned(),
            boundary_revision: self.boundary_revision.clone(),
            boundary_digest: self.boundary_digest.clone(),
            attribution: self.attribution.clone(),
            effective_grid: self.effective,
            decimation: self.decimation.factor(),
            accounting_route: "unique-native-cell ownership; sequential cropped prefixes"
                .to_owned(),
        };
        circles
            .iter()
            .zip(contributions)
            .map(|((_, _, circle_population), countries)| {
                CountryBreakdown::compose(
                    *circle_population,
                    world_population,
                    &self.effective,
                    countries,
                    provenance.clone(),
                )
                .map_err(|error| Failure::bad_input(error.to_string()))
            })
            .collect()
    }
}

struct Replay<'a> {
    grid: Grid,
    rows: &'a [Vec<f32>],
    next: u32,
    tallies: CellTallies,
}

impl<'a> Replay<'a> {
    fn new(grid: Grid, rows: &'a [Vec<f32>], tallies: CellTallies) -> Self {
        Self {
            grid,
            rows,
            next: 0,
            tallies,
        }
    }
}

impl RasterSource for Replay<'_> {
    fn grid(&self) -> Grid {
        self.grid
    }

    fn next_row(&mut self) -> Option<Result<RasterRow<'_>, RasterError>> {
        let row = self.grid.row(self.next)?;
        let values = self.rows.get(self.next as usize)?;
        self.next += 1;
        Some(Ok(RasterRow { row, values }))
    }

    fn finish(self) -> CellTallies {
        self.tallies
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use popcircles::circle;
    use popcircles::domain::{Domain, FeatureId, Geometry, Polygon};
    use popcircles::geodesy::{LatLon, RadiusKm};
    use popcircles::grid::Grid;
    use popcircles::kernel::Kernel;

    use super::*;

    fn country(id: &str, west: f64, east: f64) -> BoundaryFeature {
        BoundaryFeature {
            id: FeatureId::new(id).unwrap(),
            name: id.to_owned(),
            geometry: Geometry::Polygon(Polygon {
                rings: vec![vec![
                    LatLon {
                        lat: 0.0,
                        lon: west,
                    },
                    LatLon {
                        lat: 0.0,
                        lon: east,
                    },
                    LatLon {
                        lat: 90.0,
                        lon: east,
                    },
                    LatLon {
                        lat: 90.0,
                        lon: west,
                    },
                    LatLon {
                        lat: 0.0,
                        lon: west,
                    },
                ]],
            }),
        }
    }

    #[test]
    fn selected_country_table_uses_only_its_owned_cells() {
        let grid = Grid::new(
            36,
            18,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .unwrap();
        let features = vec![country("AA", -180.0, 0.0), country("BB", 0.0, 180.0)];
        let geometries = features
            .iter()
            .map(|feature| (feature.id.clone(), feature.geometry.clone()))
            .collect::<Vec<_>>();
        let (ownership, _) = CellOwnership::rasterize_features(&geometries, grid).unwrap();
        let domains = features
            .iter()
            .map(|feature| {
                Domain::rasterize(feature.id.clone(), &feature.geometry, grid)
                    .unwrap()
                    .0
            })
            .collect();
        let prepared = Prepared {
            source: grid,
            effective: grid,
            decimation: Decimation::none(grid),
            rows: vec![vec![1.0; grid.width() as usize]; grid.height() as usize],
            tallies: CellTallies::default(),
            ownership,
            domains,
            features: features.clone(),
            boundary_digest: "fixture".to_owned(),
            boundary_revision: "fixture".to_owned(),
            attribution: "fixture".to_owned(),
        };

        let selected = boundaries::resolve(&features, "AA").unwrap();
        let table = prepared.country_table(selected).unwrap().unwrap();
        assert_eq!(table.total(), 9.0 * 18.0);

        let kernel = Kernel::new(grid, grid.row(4).unwrap(), RadiusKm::new(0.0).unwrap()).unwrap();
        assert_eq!(
            circle::country_population(&table, &kernel, grid.col(2).unwrap()).unwrap(),
            1.0
        );
        assert_eq!(
            circle::country_population(&table, &kernel, grid.col(22).unwrap()).unwrap(),
            0.0
        );
        assert_eq!(
            circle::country_population(&table, &kernel, grid.col(2).unwrap()).unwrap(),
            1.0
        );
        let south = Kernel::new(grid, grid.row(14).unwrap(), RadiusKm::new(0.0).unwrap()).unwrap();
        assert_eq!(
            circle::country_population(&table, &south, grid.col(2).unwrap()).unwrap(),
            0.0
        );
    }
}

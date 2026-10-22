//! Country contributions to one already-resolved world circle.

use crate::domain::FeatureId;
use crate::grid::Grid;
use crate::ownership::OWNERSHIP_RULE_VERSION;
use crate::table::COUNTRY_AGGREGATION_ORDER_VERSION;

#[derive(Debug, Clone, PartialEq)]
pub struct CountryContribution {
    pub id: FeatureId,
    pub name: String,
    pub inside: f64,
    pub total: f64,
    pub intersects: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CountryBreakdownProvenance {
    pub boundary_dataset: String,
    pub boundary_revision: String,
    pub boundary_digest: String,
    pub attribution: String,
    pub effective_grid: Grid,
    pub decimation: u32,
    pub accounting_route: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CountryBreakdownEntry {
    pub id: String,
    pub name: String,
    pub population_inside: f64,
    pub country_total_population: f64,
    pub fraction_of_country_population: Option<f64>,
    pub fraction_of_circle_population: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CountryBreakdown {
    pub entries: Vec<CountryBreakdownEntry>,
    pub unassigned_population: f64,
    pub bound_persons: f64,
    pub provenance: CountryBreakdownProvenance,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CountryBreakdownError {
    #[error("circle population must be finite and nonnegative")]
    InvalidCirclePopulation,
    #[error("world population must be finite and nonnegative")]
    InvalidWorldPopulation,
    #[error("country {id} has an invalid population")]
    InvalidCountryPopulation { id: String },
    #[error("country identifiers must be unique")]
    DuplicateIdentifier,
    #[error("country count exceeds the accounting bound's supported range")]
    TooManyCountries,
    #[error("country contributions exceed circle population by more than {bound} persons")]
    Reconciliation { bound: f64 },
}

impl CountryBreakdown {
    /// Combines completed country queries and cell-centre domain intersections for one circle.
    ///
    /// # Errors
    /// Rejects invalid populations, duplicate IDs and a reconciliation gap outside the arithmetic bound.
    pub fn compose(
        circle_population: f64,
        world_total_population: f64,
        grid: &Grid,
        mut countries: Vec<CountryContribution>,
        provenance: CountryBreakdownProvenance,
    ) -> Result<Self, CountryBreakdownError> {
        if !circle_population.is_finite() || circle_population < 0.0 {
            return Err(CountryBreakdownError::InvalidCirclePopulation);
        }
        if !world_total_population.is_finite() || world_total_population < 0.0 {
            return Err(CountryBreakdownError::InvalidWorldPopulation);
        }
        countries.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        if countries.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(CountryBreakdownError::DuplicateIdentifier);
        }
        for country in &countries {
            if !country.inside.is_finite()
                || country.inside < 0.0
                || !country.total.is_finite()
                || country.total < 0.0
            {
                return Err(CountryBreakdownError::InvalidCountryPopulation {
                    id: country.id.as_str().to_owned(),
                });
            }
        }

        let country_count =
            u32::try_from(countries.len()).map_err(|_| CountryBreakdownError::TooManyCountries)?;
        let rows = f64::from(grid.height());
        let country_query_bound = countries
            .iter()
            .map(|country| rows * 4.0 * query_error(country.total))
            .sum::<f64>();
        let world_query_bound = rows * 2.0 * query_error(world_total_population);
        let country_sum = countries.iter().map(|country| country.inside).sum::<f64>();
        let reduction_bound = f64::from(country_count) * f64::EPSILON * country_sum;
        let bound_persons = world_query_bound + country_query_bound + reduction_bound;
        let mut sum = 0.0;
        let mut entries = Vec::new();
        for country in countries
            .into_iter()
            .filter(|country| country.intersects || country.inside > 0.0)
        {
            sum += country.inside;
            entries.push(CountryBreakdownEntry {
                id: country.id.as_str().to_owned(),
                name: country.name,
                population_inside: country.inside,
                country_total_population: country.total,
                fraction_of_country_population: (country.total > 0.0)
                    .then_some(country.inside / country.total),
                fraction_of_circle_population: (circle_population > 0.0)
                    .then_some(country.inside / circle_population),
            });
        }
        let unassigned_population = circle_population - sum;
        if unassigned_population < -bound_persons {
            return Err(CountryBreakdownError::Reconciliation {
                bound: bound_persons,
            });
        }
        entries.sort_by(|a, b| {
            b.population_inside
                .total_cmp(&a.population_inside)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(Self {
            entries,
            unassigned_population: unassigned_population.max(0.0),
            bound_persons,
            provenance,
        })
    }
}

impl CountryBreakdownProvenance {
    #[must_use]
    pub const fn ownership_rule_version(&self) -> u32 {
        OWNERSHIP_RULE_VERSION
    }

    #[must_use]
    pub const fn aggregation_order_version(&self) -> u32 {
        COUNTRY_AGGREGATION_ORDER_VERSION
    }

    #[must_use]
    pub const fn membership_rule_version(&self) -> u32 {
        crate::domain::MEMBERSHIP_RULE_VERSION
    }
}

fn query_error(scale: f64) -> f64 {
    let magnitude = scale.abs();
    let next = magnitude.next_up();
    let ulp = if next.is_finite() {
        next - magnitude
    } else {
        magnitude - magnitude.next_down()
    };
    4.0 * ulp + f64::EPSILON * magnitude
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::circle::{country_population, population};
    use crate::domain::{Domain, Geometry, Polygon};
    use crate::geodesy::{LatLon, RadiusKm, great_circle_km};
    use crate::kernel::Kernel;
    use crate::ownership::{CellOwner, CellOwnership};
    use crate::raster::Synthetic;
    use crate::table::{Decimation, build, build_country_decimated};

    fn grid() -> Grid {
        Grid::new(
            2,
            2,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            180.0,
            -90.0,
        )
        .expect("fixture grid is valid")
    }

    fn contribution(id: &str, inside: f64, total: f64, intersects: bool) -> CountryContribution {
        CountryContribution {
            id: FeatureId::new(id).expect("fixture ID is valid"),
            name: id.to_owned(),
            inside,
            total,
            intersects,
        }
    }

    fn provenance() -> CountryBreakdownProvenance {
        CountryBreakdownProvenance {
            boundary_dataset: "fixture".to_owned(),
            boundary_revision: "v1".to_owned(),
            boundary_digest: "fixture-digest".to_owned(),
            attribution: "fixture credit".to_owned(),
            effective_grid: grid(),
            decimation: 1,
            accounting_route: "cropped-prefix-v1".to_owned(),
        }
    }

    fn rectangle(west: f64, east: f64) -> Geometry {
        Geometry::Polygon(Polygon {
            rings: vec![vec![
                LatLon {
                    lat: -80.0,
                    lon: west,
                },
                LatLon {
                    lat: -80.0,
                    lon: east,
                },
                LatLon {
                    lat: 80.0,
                    lon: east,
                },
                LatLon {
                    lat: 80.0,
                    lon: west,
                },
                LatLon {
                    lat: -80.0,
                    lon: west,
                },
            ]],
        })
    }

    fn synthetic(grid: Grid) -> Synthetic {
        let rows = (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| (row * grid.width() + col + 1) as f32)
                    .collect()
            })
            .collect();
        Synthetic::new(grid, -3.402_823e38, rows).expect("fixture rows match the grid")
    }

    fn cell_value(grid: &Grid, row: u32, col: u32) -> f64 {
        f64::from(row * grid.width() + col + 1)
    }

    fn native_circle_sums(
        source_grid: Grid,
        ownership: &CellOwnership,
        centre: LatLon,
        radius: RadiusKm,
        factor: u32,
        a: &FeatureId,
        b: &FeatureId,
    ) -> (f64, f64, f64) {
        let grid = *Decimation::new(source_grid, factor)
            .expect("factor divides grid")
            .grid();
        let mut sums = [0.0; 3];
        for row in 0..source_grid.height() {
            for col in 0..source_grid.width() {
                let effective_cell = grid.centre_of(
                    grid.row(row / factor).expect("effective row exists"),
                    grid.col(col / factor).expect("effective column exists"),
                );
                if great_circle_km(centre, effective_cell) > radius.km() {
                    continue;
                }
                let value = cell_value(&source_grid, row, col);
                let slot = match ownership
                    .owner_at(&source_grid, row, col)
                    .expect("native source cell is valid")
                {
                    CellOwner::Feature(owner) if owner == a => 0,
                    CellOwner::Feature(owner) if owner == b => 1,
                    CellOwner::Feature(_) | CellOwner::Unassigned(_) => 2,
                };
                sums[slot] += value;
            }
        }
        (sums[0], sums[1], sums[2])
    }

    #[test]
    fn composition_pins_distinct_denominators_order_and_circle_remainder() {
        let inputs = vec![
            contribution("B", 10.0, 20.0, true),
            contribution("A", 30.0, 100.0, true),
        ];
        let forward = CountryBreakdown::compose(50.0, 150.0, &grid(), inputs.clone(), provenance())
            .expect("country contributions reconcile");
        let reverse = CountryBreakdown::compose(
            50.0,
            150.0,
            &grid(),
            inputs.into_iter().rev().collect(),
            provenance(),
        )
        .expect("reversed contributions reconcile");
        assert_eq!(forward, reverse);
        assert_eq!(forward.entries[0].id, "A");
        assert_eq!(forward.entries[0].fraction_of_country_population, Some(0.3));
        assert_eq!(forward.entries[0].fraction_of_circle_population, Some(0.6));
        assert_eq!(forward.entries[1].fraction_of_country_population, Some(0.5));
        assert_eq!(forward.entries[1].fraction_of_circle_population, Some(0.2));
        assert_eq!(forward.unassigned_population, 10.0);
    }

    #[test]
    fn equal_contributions_use_identifier_order_and_intersections_keep_zeroes() {
        let result = CountryBreakdown::compose(
            20.0,
            20.0,
            &grid(),
            vec![
                contribution("B", 10.0, 10.0, true),
                contribution("A", 10.0, 10.0, true),
                contribution("Z", 0.0, 0.0, true),
                contribution("outside", 0.0, 1.0, false),
            ],
            provenance(),
        )
        .expect("country contributions reconcile");
        assert_eq!(
            result
                .entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "Z"]
        );
        assert_eq!(result.entries[2].fraction_of_country_population, None);
        assert_eq!(result.entries[2].fraction_of_circle_population, Some(0.0));
    }

    #[test]
    fn empty_circle_has_empty_breakdown_and_zero_denominators_stay_null() {
        let result = CountryBreakdown::compose(0.0, 0.0, &grid(), Vec::new(), provenance())
            .expect("an empty circle reconciles");
        assert_eq!(result.entries.len(), 0);
        assert_eq!(result.unassigned_population, 0.0);
        assert!(result.unassigned_population.is_finite());
    }

    #[test]
    fn reconciliation_bound_uses_world_and_country_prefix_scales() {
        let grid = grid();
        let result = CountryBreakdown::compose(
            1.0,
            1_000_000_000_000.0,
            &grid,
            vec![contribution("large", 1.0, 1_000_000_000_000.0, true)],
            provenance(),
        )
        .expect("the one-person circle reconciles");
        let circle_scaled =
            f64::from(grid.height()) * (4.0 * (1.0f64.next_up() - 1.0) + f64::EPSILON);
        assert!(result.bound_persons > circle_scaled);
        assert_eq!(result.unassigned_population, 0.0);
    }

    #[test]
    fn overlapping_and_decimated_queries_match_native_cell_sums() {
        let source_grid = Grid::new(
            36,
            18,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("whole-globe source grid is valid");
        let a = FeatureId::new("A").expect("fixture ID is valid");
        let b = FeatureId::new("B").expect("fixture ID is valid");
        let features = vec![
            (a.clone(), rectangle(-90.0, 10.0)),
            (b.clone(), rectangle(-10.0, 90.0)),
        ];
        let (ownership, _) = CellOwnership::rasterize_features(&features, source_grid)
            .expect("overlapping features produce explicit residual cells");
        let requested = LatLon {
            lat: -5.0,
            lon: 35.0,
        };
        let radius = RadiusKm::from(20_016u32);

        for factor in [1, 2] {
            let decimation = Decimation::new(source_grid, factor).expect("factor divides grid");
            let grid = *decimation.grid();
            let domains = features
                .iter()
                .map(|(id, geometry)| Domain::rasterize(id.clone(), geometry, grid))
                .collect::<Result<Vec<_>, _>>()
                .expect("effective masks rasterize");
            let cell = grid.cell_containing(requested).expect("centre is on grid");
            let centre = grid.centre_of(cell.0, cell.1);
            let kernel = Kernel::new(grid, cell.0, radius).expect("grid closes in longitude");
            let mut world_payload = Vec::new();
            build(synthetic(source_grid), decimation, &mut (), |row| {
                world_payload.extend_from_slice(row);
                Ok::<(), Infallible>(())
            })
            .expect("synthetic source and sink cannot fail");
            let world_table = crate::table::Table::new(grid, &world_payload)
                .expect("build emitted a padded table");
            let circle_total = population(&world_table, &kernel, cell.1);
            let (whole_rows, whole_cols) = world_table.whole();
            let world_total = world_table.population(whole_rows, whole_cols);
            let mut contributions = Vec::new();
            for ((id, _), (domain, _)) in features.iter().zip(domains) {
                let table = build_country_decimated(
                    synthetic(source_grid),
                    &ownership,
                    id,
                    decimation,
                    "fixture-boundary-digest",
                )
                .expect("each fixture country owns source cells");
                let inside = country_population(&table, &kernel, cell.1)
                    .expect("crop and circle use the same effective grid");
                let intersects = grid.rows().any(|row| {
                    grid.cols().any(|col| {
                        great_circle_km(centre, grid.centre_of(row, col)) <= radius.km()
                            && domain
                                .contains(&grid, row, col)
                                .expect("domain uses the effective grid")
                    })
                });
                contributions.push(CountryContribution {
                    id: id.clone(),
                    name: id.as_str().to_owned(),
                    inside,
                    total: table.total(),
                    intersects,
                });
            }
            let result = CountryBreakdown::compose(
                circle_total,
                world_total,
                &grid,
                contributions,
                provenance(),
            )
            .expect("circle contributions reconcile within their operation bound");

            let (direct_a, direct_b, direct_unassigned) =
                native_circle_sums(source_grid, &ownership, centre, radius, factor, &a, &b);
            let allowance = result.bound_persons;
            let measured = |id: &str| {
                result
                    .entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .map_or(0.0, |entry| entry.population_inside)
            };
            assert!((measured("A") - direct_a).abs() <= allowance);
            assert!((measured("B") - direct_b).abs() <= allowance);
            assert!((result.unassigned_population - direct_unassigned).abs() <= allowance);
        }
    }
}

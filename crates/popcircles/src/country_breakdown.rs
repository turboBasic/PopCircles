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
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::geodesy::LatLon;

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
}

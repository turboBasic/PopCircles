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
        grid: &Grid,
        mut countries: Vec<CountryContribution>,
        provenance: CountryBreakdownProvenance,
    ) -> Result<Self, CountryBreakdownError> {
        if !circle_population.is_finite() || circle_population < 0.0 {
            return Err(CountryBreakdownError::InvalidCirclePopulation);
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
        let queries = f64::from(grid.height()) * f64::from(country_count);
        let ulp = circle_population.next_up() - circle_population;
        let bound_persons = queries * (4.0 * ulp + f64::EPSILON * circle_population);
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

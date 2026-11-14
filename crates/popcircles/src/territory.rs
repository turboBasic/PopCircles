//! Area-weighted coverage of a circle over one or more effective-grid domains.

use crate::domain::Domain;
use crate::grid::{Col, GridIndexError};
use crate::kernel::Kernel;
use crate::table::ColSpan;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TerritoryShareError {
    #[error("a domain belongs to a different effective grid than the circle")]
    GridMismatch,
    #[error("the selected domain has zero area on the effective grid")]
    ZeroArea,
    #[error(transparent)]
    GridIndex(#[from] GridIndexError),
}

/// The fraction of the union of `domains` covered by the existing cell-centre circle kernel.
///
/// # Errors
/// Returns [`TerritoryShareError::GridMismatch`] for masks on another grid and
/// [`TerritoryShareError::ZeroArea`] when their union contains no cells, or
/// [`TerritoryShareError::GridIndex`] when a circle row is outside the grid.
pub fn territory_share(
    domains: &[Domain],
    kernel: &Kernel,
    centre: Col,
) -> Result<f64, TerritoryShareError> {
    let grid = kernel.grid();
    if domains.iter().any(|domain| domain.grid() != grid) {
        return Err(TerritoryShareError::GridMismatch);
    }

    let mut denominator = 0.0;
    for row in grid.rows() {
        let ranges = union_ranges(domains, row);
        let cells: f64 = ranges
            .iter()
            .map(|(start, end)| f64::from(end - start))
            .sum();
        denominator += cells * grid.cell_area_km2(row)?;
    }
    if denominator == 0.0 {
        return Err(TerritoryShareError::ZeroArea);
    }

    let mut numerator = 0.0;
    for placed in kernel.place(centre)? {
        let (row, circle) = placed?;
        let circle_ranges = span_ranges(circle, grid.width());
        let domain_ranges = union_ranges(domains, row);
        let cells: f64 = domain_ranges
            .iter()
            .flat_map(|&(start, end)| {
                circle_ranges
                    .iter()
                    .map(move |&(a, b)| (start.max(a), end.min(b)))
            })
            .map(|(start, end)| f64::from(end.saturating_sub(start)))
            .sum();
        numerator += cells * grid.cell_area_km2(row)?;
    }
    Ok((numerator / denominator).clamp(0.0, 1.0))
}

fn span_ranges(span: ColSpan, width: u32) -> Vec<(u32, u32)> {
    match span {
        ColSpan::FullTurn => vec![(0, width)],
        ColSpan::Through { west, east } if west.get() <= east.get() => {
            vec![(west.get(), east.get() + 1)]
        }
        ColSpan::Through { west, east } => {
            vec![(0, east.get() + 1), (west.get(), width)]
        }
    }
}

fn union_ranges(domains: &[Domain], row: crate::grid::Row) -> Vec<(u32, u32)> {
    let mut ranges = domains
        .iter()
        .filter_map(|domain| domain.row_intervals(row))
        .flat_map(|intervals| {
            intervals
                .iter()
                .map(|interval| (interval.start(), interval.end()))
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    let mut union: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some((_, previous_end)) = union.last_mut()
            && start <= *previous_end
        {
            *previous_end = (*previous_end).max(end);
        } else {
            union.push((start, end));
        }
    }
    union
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::domain::{ColumnInterval, FeatureId};
    use crate::geodesy::{LatLon, RadiusKm};
    use crate::grid::Grid;

    fn grid() -> Grid {
        Grid::new(
            4,
            2,
            LatLon {
                lat: 90.0,
                lon: 0.0,
            },
            90.0,
            -90.0,
        )
        .unwrap()
    }

    fn domain(grid: Grid, id: &str, rows: &[&[(u32, u32)]]) -> Domain {
        Domain::from_intervals(
            FeatureId::new(id).unwrap(),
            grid,
            rows.iter()
                .map(|intervals| {
                    intervals
                        .iter()
                        .map(|&(start, end)| ColumnInterval::new(start, end).unwrap())
                        .collect()
                })
                .collect(),
        )
        .unwrap()
    }

    fn kernel(grid: Grid, radius: f64) -> Kernel {
        Kernel::new(grid, grid.row(0).unwrap(), RadiusKm::new(radius).unwrap()).unwrap()
    }

    #[test]
    fn measures_zero_partial_and_full_coverage() {
        let grid = grid();
        let full = domain(grid, "full", &[&[(0, 4)], &[(0, 4)]]);
        let centre = grid.col(1).unwrap();
        assert_eq!(
            territory_share(std::slice::from_ref(&full), &kernel(grid, 0.0), centre),
            Ok(0.0)
        );
        let partial =
            territory_share(std::slice::from_ref(&full), &kernel(grid, 5_000.0), centre).unwrap();
        assert!(partial > 0.0 && partial < 1.0);
        assert_eq!(
            territory_share(std::slice::from_ref(&full), &kernel(grid, 20_016.0), centre),
            Ok(1.0)
        );
    }

    #[test]
    fn weights_covered_cells_by_their_row_area() {
        let grid = Grid::new(
            4,
            2,
            LatLon {
                lat: 90.0,
                lon: 0.0,
            },
            90.0,
            -45.0,
        )
        .unwrap();
        let domain = domain(grid, "unequal-rows", &[&[(1, 2)], &[(1, 2)]]);
        let centre = grid.col(1).unwrap();
        let kernel = Kernel::new(grid, grid.row(1).unwrap(), RadiusKm::new(1.0).unwrap()).unwrap();
        let north_area = grid.cell_area_km2(grid.row(0).unwrap()).unwrap();
        let south_area = grid.cell_area_km2(grid.row(1).unwrap()).unwrap();

        let actual = territory_share(std::slice::from_ref(&domain), &kernel, centre).unwrap();
        assert_eq!(actual, south_area / (north_area + south_area));
        assert_ne!(actual, 0.5);
    }

    #[test]
    fn overlapping_masks_are_counted_once() {
        let grid = grid();
        let left = domain(grid, "left", &[&[(0, 2)], &[]]);
        let overlap = domain(grid, "overlap", &[&[(1, 3)], &[]]);
        let whole = domain(grid, "whole", &[&[(0, 3)], &[]]);
        let centre = grid.col(1).unwrap();
        let circle = kernel(grid, 5_000.0);
        assert_eq!(
            territory_share(&[left, overlap], &circle, centre),
            territory_share(std::slice::from_ref(&whole), &circle, centre)
        );
    }

    #[test]
    fn empty_domain_has_no_fraction() {
        let grid = grid();
        let empty = domain(grid, "empty", &[&[], &[]]);
        assert_eq!(
            territory_share(
                std::slice::from_ref(&empty),
                &kernel(grid, 1.0),
                grid.col(1).unwrap()
            ),
            Err(TerritoryShareError::ZeroArea)
        );
    }

    #[test]
    fn rejects_a_domain_on_another_grid() {
        let grid = grid();
        let other = Grid::new(
            4,
            2,
            LatLon {
                lat: 90.0,
                lon: 1.0,
            },
            90.0,
            -90.0,
        )
        .unwrap();
        let domain = domain(other, "other", &[&[(0, 1)], &[]]);
        assert_eq!(
            territory_share(
                std::slice::from_ref(&domain),
                &kernel(grid, 1.0),
                grid.col(1).unwrap()
            ),
            Err(TerritoryShareError::GridMismatch)
        );
    }
}

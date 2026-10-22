//! Source-cell ownership for population accounting, separate from cell-centre domain masks.

use crate::domain::FeatureId;
use crate::grid::Grid;

/// Version of the rule that assigns each source cell to a unique intersecting feature.
pub const OWNERSHIP_RULE_VERSION: u32 = 1;

/// Why a source cell has no single country owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnassignedReason {
    /// The cell footprint has no positive-area intersection with any feature.
    OutsideFeatures,
    /// The cell footprint has positive-area intersection with multiple features.
    MultipleFeatures(Vec<FeatureId>),
}

/// A cell's unique owner, or the reason it remains in the unassigned residual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellOwner {
    Feature(FeatureId),
    Unassigned(UnassignedReason),
}

/// A non-empty half-open span of cells that share one ownership result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipInterval {
    start: u32,
    end: u32,
    owner: CellOwner,
}

impl OwnershipInterval {
    /// Constructs a non-empty ownership interval.
    #[must_use]
    pub fn new(start: u32, end: u32, owner: CellOwner) -> Option<Self> {
        (start < end).then_some(Self { start, end, owner })
    }

    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }

    #[must_use]
    pub const fn owner(&self) -> &CellOwner {
        &self.owner
    }
}

/// A complete, canonical ownership partition of one effective grid.
#[derive(Debug, Clone, PartialEq)]
pub struct CellOwnership {
    grid: Grid,
    rows: Vec<Vec<OwnershipInterval>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OwnershipError {
    #[error("ownership has {found} row partitions, but its grid has {expected} rows")]
    RowCount { expected: usize, found: usize },
    #[error("row {row} has an invalid or unordered ownership interval [{start}, {end})")]
    InvalidInterval { row: u32, start: u32, end: u32 },
    #[error("row {row} ownership partition expects column {expected}, found {found}")]
    IncompletePartition { row: u32, expected: u32, found: u32 },
    #[error("row {row} has adjacent intervals with the same owner; merge them canonically")]
    AdjacentEqualOwners { row: u32 },
    #[error("ambiguous ownership on row {row} has fewer than two distinct, sorted features")]
    InvalidAmbiguousFeatures { row: u32 },
    #[error("ownership belongs to a different effective grid")]
    GridMismatch,
    #[error("cell ({row}, {col}) is outside the effective grid")]
    CellOutsideGrid { row: u32, col: u32 },
}

impl CellOwnership {
    /// Constructs a complete grid partition from sorted, canonical row intervals.
    ///
    /// Every cell must appear exactly once. Adjacent intervals with the same owner have one
    /// canonical representation and must be merged. Ambiguous feature identities must be sorted
    /// and distinct, so source feature iteration order cannot affect the encoded result.
    ///
    /// # Errors
    /// Rejects a row count or grid partition mismatch, unordered or out-of-grid spans, and
    /// malformed ambiguous feature lists.
    pub fn from_intervals(
        grid: Grid,
        rows: Vec<Vec<OwnershipInterval>>,
    ) -> Result<Self, OwnershipError> {
        if rows.len() != grid.height() as usize {
            return Err(OwnershipError::RowCount {
                expected: grid.height() as usize,
                found: rows.len(),
            });
        }

        for (row, intervals) in grid.rows().zip(&rows) {
            let mut expected_start = 0;
            let mut previous_owner: Option<&CellOwner> = None;
            for interval in intervals {
                if interval.start >= interval.end || interval.end > grid.width() {
                    return Err(OwnershipError::InvalidInterval {
                        row: row.get(),
                        start: interval.start,
                        end: interval.end,
                    });
                }
                if interval.start != expected_start {
                    return Err(OwnershipError::IncompletePartition {
                        row: row.get(),
                        expected: expected_start,
                        found: interval.start,
                    });
                }
                if previous_owner == Some(&interval.owner) {
                    return Err(OwnershipError::AdjacentEqualOwners { row: row.get() });
                }
                if let CellOwner::Unassigned(UnassignedReason::MultipleFeatures(features)) =
                    &interval.owner
                    && (features.len() < 2
                        || !features
                            .windows(2)
                            .all(|pair| pair[0].as_str() < pair[1].as_str()))
                {
                    return Err(OwnershipError::InvalidAmbiguousFeatures { row: row.get() });
                }
                expected_start = interval.end;
                previous_owner = Some(&interval.owner);
            }
            if expected_start != grid.width() {
                return Err(OwnershipError::IncompletePartition {
                    row: row.get(),
                    expected: grid.width(),
                    found: expected_start,
                });
            }
        }

        Ok(Self { grid, rows })
    }

    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    #[must_use]
    pub fn row_intervals(&self, row: u32) -> Option<&[OwnershipInterval]> {
        self.rows.get(row as usize).map(Vec::as_slice)
    }

    /// Returns the owner of one cell.
    ///
    /// # Errors
    /// Returns [`OwnershipError::GridMismatch`] when queried with another effective grid.
    /// Returns [`OwnershipError::CellOutsideGrid`] if either index is outside this grid.
    pub fn owner_at(&self, grid: &Grid, row: u32, col: u32) -> Result<&CellOwner, OwnershipError> {
        if &self.grid != grid {
            return Err(OwnershipError::GridMismatch);
        }
        if row >= grid.height() || col >= grid.width() {
            return Err(OwnershipError::CellOutsideGrid { row, col });
        }
        let intervals = &self.rows[row as usize];
        let index = intervals.partition_point(|interval| interval.end <= col);
        intervals
            .get(index)
            .filter(|interval| interval.start <= col)
            .map(|interval| &interval.owner)
            .ok_or(OwnershipError::CellOutsideGrid { row, col })
    }
}

// Test fixtures use expect to keep invalid setup distinct from assertions about ownership.
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{CellOwner, CellOwnership, OwnershipError, OwnershipInterval, UnassignedReason};
    use crate::domain::FeatureId;
    use crate::geodesy::LatLon;
    use crate::grid::Grid;

    fn grid(width: u32, height: u32) -> Grid {
        Grid::new(width, height, LatLon { lat: 1.0, lon: 0.0 }, 1.0, -1.0)
            .expect("fixture grid is valid")
    }

    fn feature(value: &str) -> FeatureId {
        FeatureId::new(value).expect("fixture feature ID is valid")
    }

    fn interval(start: u32, end: u32, owner: CellOwner) -> OwnershipInterval {
        OwnershipInterval::new(start, end, owner).expect("fixture interval is non-empty")
    }

    #[test]
    fn domain_ownership_accepts_a_complete_canonical_partition() {
        let grid = grid(4, 1);
        let ownership = CellOwnership::from_intervals(
            grid,
            vec![vec![
                interval(0, 2, CellOwner::Feature(feature("A"))),
                interval(
                    2,
                    3,
                    CellOwner::Unassigned(UnassignedReason::OutsideFeatures),
                ),
                interval(
                    3,
                    4,
                    CellOwner::Unassigned(UnassignedReason::MultipleFeatures(vec![
                        feature("B"),
                        feature("C"),
                    ])),
                ),
            ]],
        )
        .expect("partition is valid");

        assert_eq!(
            ownership.owner_at(&grid, 0, 3),
            Ok(&CellOwner::Unassigned(UnassignedReason::MultipleFeatures(
                vec![feature("B"), feature("C"),]
            )))
        );
    }

    #[test]
    fn domain_ownership_rejects_an_overlapping_or_unordered_partition() {
        let grid = grid(4, 1);
        let result = CellOwnership::from_intervals(
            grid,
            vec![vec![
                interval(0, 3, CellOwner::Feature(feature("A"))),
                interval(2, 4, CellOwner::Feature(feature("B"))),
            ]],
        );

        assert!(matches!(
            result,
            Err(OwnershipError::IncompletePartition { .. })
        ));
    }

    #[test]
    fn domain_ownership_rejects_a_gap_or_missing_tail() {
        let grid = grid(4, 1);
        let result = CellOwnership::from_intervals(
            grid,
            vec![vec![interval(0, 2, CellOwner::Feature(feature("A")))]],
        );

        assert!(matches!(
            result,
            Err(OwnershipError::IncompletePartition { .. })
        ));
    }

    #[test]
    fn domain_ownership_rejects_ambiguous_cells_without_two_ordered_features() {
        let grid = grid(2, 1);
        let result = CellOwnership::from_intervals(
            grid,
            vec![vec![
                interval(
                    0,
                    1,
                    CellOwner::Unassigned(UnassignedReason::MultipleFeatures(vec![feature("A")])),
                ),
                interval(1, 2, CellOwner::Feature(feature("B"))),
            ]],
        );

        assert!(matches!(
            result,
            Err(OwnershipError::InvalidAmbiguousFeatures { .. })
        ));
    }

    #[test]
    fn domain_ownership_is_bound_to_the_exact_grid() {
        let grid = grid(2, 1);
        let other_grid = self::grid(1, 2);
        let ownership = CellOwnership::from_intervals(
            grid,
            vec![vec![interval(0, 2, CellOwner::Feature(feature("A")))]],
        )
        .expect("partition is valid");

        assert_eq!(
            ownership.owner_at(&other_grid, 0, 0),
            Err(OwnershipError::GridMismatch)
        );
    }

    #[test]
    fn domain_ownership_rejects_an_index_outside_the_grid() {
        let grid = grid(2, 1);
        let ownership = CellOwnership::from_intervals(
            grid,
            vec![vec![interval(0, 2, CellOwner::Feature(feature("A")))]],
        )
        .expect("partition is valid");

        assert_eq!(
            ownership.owner_at(&grid, 0, 2),
            Err(OwnershipError::CellOutsideGrid { row: 0, col: 2 })
        );
    }
}

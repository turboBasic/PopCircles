//! Cell membership for a named region on one effective population grid.

use std::fmt;

use crate::geodesy::LatLon;
use crate::grid::{Col, Grid, Row};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FeatureId(String);

impl FeatureId {
    /// # Errors
    /// Returns [`DomainError::EmptyFeatureId`] for an empty or whitespace-only identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyFeatureId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A half-open span of columns in one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnInterval {
    start: u32,
    end: u32,
}

impl ColumnInterval {
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }
}

/// Membership in one domain, stored as sorted, non-overlapping row intervals.
#[derive(Debug, Clone, PartialEq)]
pub struct Domain {
    feature_id: FeatureId,
    grid: Grid,
    rows: Vec<Vec<ColumnInterval>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("feature identifier is empty")]
    EmptyFeatureId,
    #[error("domain has {found} row masks, but its grid has {expected} rows")]
    RowCount { expected: usize, found: usize },
    #[error("row {row} has an invalid or unordered column interval [{start}, {end})")]
    InvalidInterval { row: u32, start: u32, end: u32 },
    #[error("domain belongs to a different effective grid")]
    GridMismatch,
    #[error("cell ({row}, {col}) is outside the effective grid")]
    CellOutsideGrid { row: u32, col: u32 },
}

impl Domain {
    /// # Errors
    /// Rejects a row count that differs from the grid, intervals outside the grid, and intervals
    /// that are unordered, overlapping or adjacent. Adjacent spans have one canonical encoding.
    pub fn from_intervals(
        feature_id: FeatureId,
        grid: Grid,
        rows: Vec<Vec<ColumnInterval>>,
    ) -> Result<Self, DomainError> {
        if rows.len() != grid.height() as usize {
            return Err(DomainError::RowCount {
                expected: grid.height() as usize,
                found: rows.len(),
            });
        }
        for (row, intervals) in grid.rows().zip(&rows) {
            let mut previous_end = None;
            for interval in intervals {
                if interval.start >= interval.end
                    || interval.end > grid.width()
                    || previous_end.is_some_and(|end| interval.start <= end)
                {
                    return Err(DomainError::InvalidInterval {
                        row: row.get(),
                        start: interval.start,
                        end: interval.end,
                    });
                }
                previous_end = Some(interval.end);
            }
        }
        Ok(Self {
            feature_id,
            grid,
            rows,
        })
    }

    #[must_use]
    pub const fn feature_id(&self) -> &FeatureId {
        &self.feature_id
    }

    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    #[must_use]
    pub fn row_intervals(&self, row: Row) -> Option<&[ColumnInterval]> {
        self.rows.get(row.get() as usize).map(Vec::as_slice)
    }

    /// Whether a cell belongs to this domain.
    ///
    /// # Errors
    /// Returns [`DomainError::GridMismatch`] if queried against another effective grid, or
    /// [`DomainError::CellOutsideGrid`] if either typed index came from a larger grid.
    pub fn contains(&self, grid: &Grid, row: Row, col: Col) -> Result<bool, DomainError> {
        if &self.grid != grid {
            return Err(DomainError::GridMismatch);
        }
        if row.get() >= grid.height() || col.get() >= grid.width() {
            return Err(DomainError::CellOutsideGrid {
                row: row.get(),
                col: col.get(),
            });
        }
        Ok(self.rows[row.get() as usize]
            .iter()
            .any(|interval| (interval.start..interval.end).contains(&col.get())))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    /// The first ring is the exterior; subsequent rings are holes.
    pub rings: Vec<Vec<LatLon>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    Polygon(Polygon),
    MultiPolygon(Vec<Polygon>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryDiagnostic {
    pub feature_id: FeatureId,
    pub component: usize,
    pub reason: DegenerateReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegenerateReason {
    MissingExteriorRing,
    TooFewDistinctVertices,
    CollinearVertices,
}

impl fmt::Display for DegenerateReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingExteriorRing => f.write_str("missing exterior ring"),
            Self::TooFewDistinctVertices => f.write_str("fewer than three distinct vertices"),
            Self::CollinearVertices => f.write_str("all exterior vertices are collinear"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::geodesy::LatLon;

    fn make_grid(width: u32, height: u32) -> Grid {
        Grid::new(width, height, LatLon { lat: 2.0, lon: 0.0 }, 1.0, -1.0)
            .expect("test grid is valid")
    }

    fn id(value: &str) -> FeatureId {
        FeatureId::new(value).expect("test identifier is nonempty")
    }

    #[test]
    fn feature_identity_and_domain_shape_are_validated() {
        assert_eq!(FeatureId::new("  "), Err(DomainError::EmptyFeatureId));
        assert!(matches!(
            Domain::from_intervals(id("A"), make_grid(2, 2), vec![vec![]]),
            Err(DomainError::RowCount {
                expected: 2,
                found: 1
            })
        ));
        assert!(matches!(
            Domain::from_intervals(
                id("A"),
                make_grid(2, 1),
                vec![vec![ColumnInterval { start: 0, end: 3 }]],
            ),
            Err(DomainError::InvalidInterval { .. })
        ));
    }

    #[test]
    fn membership_requires_the_same_grid_and_valid_indices() {
        let grid = make_grid(2, 1);
        let domain = Domain::from_intervals(
            id("A"),
            grid,
            vec![vec![ColumnInterval { start: 1, end: 2 }]],
        )
        .expect("interval fits grid");
        let row = grid.row(0).expect("row exists");
        let col = grid.col(1).expect("column exists");
        assert!(domain.contains(&grid, row, col).expect("same grid"));
        assert!(matches!(
            domain.contains(
                &Grid::new(2, 1, LatLon { lat: 1.0, lon: 0.0 }, 1.0, -1.0,)
                    .expect("different valid grid"),
                row,
                col,
            ),
            Err(DomainError::GridMismatch)
        ));
        let larger_col = make_grid(3, 1)
            .col(2)
            .expect("column exists on larger grid");
        assert!(matches!(
            domain.contains(&grid, row, larger_col),
            Err(DomainError::CellOutsideGrid { .. })
        ));
    }
}

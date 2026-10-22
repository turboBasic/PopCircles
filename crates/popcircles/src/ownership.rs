//! Source-cell ownership for population accounting, separate from cell-centre domain masks.

use std::collections::HashSet;

use crate::domain::{DomainError, FeatureId, Geometry, GeometryDiagnostic};
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OwnershipRasterizeError {
    #[error("duplicate feature identifier {0}")]
    DuplicateFeatureId(String),
    #[error(transparent)]
    Geometry(#[from] DomainError),
    #[error(transparent)]
    Ownership(#[from] OwnershipError),
}

impl CellOwnership {
    /// Classifies positive-area cell contact for each feature into a complete ownership grid.
    ///
    /// # Errors
    /// Returns an error for duplicate feature IDs, invalid geometry coordinates, or a malformed
    /// resulting row partition.
    pub fn rasterize_features(
        features: &[(FeatureId, Geometry)],
        grid: Grid,
    ) -> Result<(Self, Vec<GeometryDiagnostic>), OwnershipRasterizeError> {
        let mut seen = HashSet::with_capacity(features.len());
        let mut rasterized = Vec::with_capacity(features.len());
        let mut diagnostics = Vec::new();
        for (feature_id, geometry) in features {
            if !seen.insert(feature_id.as_str()) {
                return Err(OwnershipRasterizeError::DuplicateFeatureId(
                    feature_id.as_str().to_owned(),
                ));
            }
            let (rows, feature_diagnostics) = geometry.positive_area_intervals(feature_id, grid)?;
            rasterized.push((feature_id.clone(), rows));
            diagnostics.extend(feature_diagnostics);
        }
        diagnostics.sort_unstable_by(|first, second| {
            (first.feature_id.as_str(), first.component)
                .cmp(&(second.feature_id.as_str(), second.component))
        });

        let mut rows = Vec::with_capacity(grid.height() as usize);
        for row_index in 0..grid.height() {
            let mut events = Vec::new();
            for (feature_index, (_, feature_rows)) in rasterized.iter().enumerate() {
                if let Some(intervals) = feature_rows.get(row_index as usize) {
                    for interval in intervals {
                        events.push((interval.start(), feature_index, true));
                        events.push((interval.end(), feature_index, false));
                    }
                }
            }
            events.sort_unstable_by_key(|event| (event.0, event.1, event.2));
            let mut active = vec![false; rasterized.len()];
            let mut ownership = Vec::new();
            let mut previous_column = 0;
            let mut event_index = 0;
            while event_index < events.len() {
                let column = events[event_index].0;
                if previous_column < column {
                    let owner = owner_for_active(&rasterized, &active);
                    push_ownership_interval(&mut ownership, previous_column, column, owner);
                }
                while event_index < events.len() && events[event_index].0 == column {
                    let (_, feature_index, starts) = events[event_index];
                    active[feature_index] = starts;
                    event_index += 1;
                }
                previous_column = column;
            }
            if previous_column < grid.width() {
                let owner = owner_for_active(&rasterized, &active);
                push_ownership_interval(&mut ownership, previous_column, grid.width(), owner);
            }
            rows.push(ownership);
        }
        Ok((Self::from_intervals(grid, rows)?, diagnostics))
    }

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

fn owner_for_active(
    rasterized: &[(FeatureId, Vec<Vec<crate::domain::ColumnInterval>>)],
    active: &[bool],
) -> CellOwner {
    let mut features: Vec<_> = rasterized
        .iter()
        .zip(active)
        .filter_map(|((feature_id, _), is_active)| is_active.then_some(feature_id.clone()))
        .collect();
    match features.len() {
        0 => CellOwner::Unassigned(UnassignedReason::OutsideFeatures),
        1 => CellOwner::Feature(features.remove(0)),
        _ => {
            features.sort_unstable_by(|first, second| first.as_str().cmp(second.as_str()));
            CellOwner::Unassigned(UnassignedReason::MultipleFeatures(features))
        }
    }
}

fn push_ownership_interval(
    intervals: &mut Vec<OwnershipInterval>,
    start: u32,
    end: u32,
    owner: CellOwner,
) {
    if let Some(previous) = intervals.last_mut()
        && previous.end() == start
        && previous.owner() == &owner
    {
        previous.end = end;
        return;
    }
    intervals.push(OwnershipInterval { start, end, owner });
}

// Test fixtures use expect to keep invalid setup distinct from assertions about ownership.
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{CellOwner, CellOwnership, OwnershipError, OwnershipInterval, UnassignedReason};
    use crate::domain::{DegenerateReason, FeatureId, Geometry, Polygon};
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

    fn rectangle(west: f64, south: f64, east: f64, north: f64) -> Polygon {
        Polygon {
            rings: vec![vec![
                LatLon {
                    lat: south,
                    lon: west,
                },
                LatLon {
                    lat: south,
                    lon: east,
                },
                LatLon {
                    lat: north,
                    lon: east,
                },
                LatLon {
                    lat: north,
                    lon: west,
                },
                LatLon {
                    lat: south,
                    lon: west,
                },
            ]],
        }
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

    #[test]
    fn domain_ownership_counts_a_footprint_when_its_centre_is_outside() {
        let grid = grid(2, 1);
        let features = vec![(
            feature("A"),
            Geometry::Polygon(rectangle(0.1, 0.1, 0.4, 0.9)),
        )];
        let (ownership, diagnostics) =
            CellOwnership::rasterize_features(&features, grid).expect("valid feature rasterizes");

        assert_eq!(diagnostics.len(), 0);
        assert_eq!(
            ownership.owner_at(&grid, 0, 0),
            Ok(&CellOwner::Feature(feature("A")))
        );
        assert_eq!(
            ownership.owner_at(&grid, 0, 1),
            Ok(&CellOwner::Unassigned(UnassignedReason::OutsideFeatures))
        );
    }

    #[test]
    fn domain_ownership_leaves_shared_cells_ambiguous_and_edge_only_contact_outside() {
        let grid = grid(3, 1);
        let features = vec![
            (
                feature("B"),
                Geometry::Polygon(rectangle(0.4, 0.0, 1.0, 1.0)),
            ),
            (
                feature("A"),
                Geometry::Polygon(rectangle(0.0, 0.0, 0.4, 1.0)),
            ),
            (
                feature("C"),
                Geometry::Polygon(rectangle(2.0, 0.0, 3.0, 1.0)),
            ),
        ];
        let (ownership, _) =
            CellOwnership::rasterize_features(&features, grid).expect("valid feature rasterizes");

        assert_eq!(
            ownership.owner_at(&grid, 0, 0),
            Ok(&CellOwner::Unassigned(UnassignedReason::MultipleFeatures(
                vec![feature("A"), feature("B"),]
            )))
        );
        assert_eq!(
            ownership.owner_at(&grid, 0, 1),
            Ok(&CellOwner::Unassigned(UnassignedReason::OutsideFeatures))
        );
        assert_eq!(
            ownership.owner_at(&grid, 0, 2),
            Ok(&CellOwner::Feature(feature("C")))
        );
    }

    #[test]
    fn domain_ownership_subtracts_holes_and_combines_multipart_islands() {
        let grid = Grid::new(3, 3, LatLon { lat: 3.0, lon: 0.0 }, 1.0, -1.0)
            .expect("fixture grid is valid");
        let mut polygon = rectangle(0.0, 0.0, 3.0, 3.0);
        polygon.rings.push(vec![
            LatLon { lat: 1.0, lon: 1.0 },
            LatLon { lat: 1.0, lon: 2.0 },
            LatLon { lat: 2.0, lon: 2.0 },
            LatLon { lat: 2.0, lon: 1.0 },
            LatLon { lat: 1.0, lon: 1.0 },
        ]);
        let features = vec![(feature("A"), Geometry::Polygon(polygon))];
        let (ownership, _) =
            CellOwnership::rasterize_features(&features, grid).expect("valid feature rasterizes");
        assert_eq!(
            ownership.owner_at(&grid, 1, 1),
            Ok(&CellOwner::Unassigned(UnassignedReason::OutsideFeatures))
        );

        let multipart_grid = self::grid(3, 1);
        let multipart = Geometry::MultiPolygon(vec![
            rectangle(0.1, 0.1, 0.4, 0.9),
            rectangle(2.1, 0.1, 2.9, 0.9),
        ]);
        let (ownership, _) =
            CellOwnership::rasterize_features(&[(feature("M"), multipart)], multipart_grid)
                .expect("valid multiple-polygon geometry rasterizes");
        assert_eq!(
            ownership.owner_at(&multipart_grid, 0, 0),
            Ok(&CellOwner::Feature(feature("M")))
        );
        assert_eq!(
            ownership.owner_at(&multipart_grid, 0, 1),
            Ok(&CellOwner::Unassigned(UnassignedReason::OutsideFeatures))
        );
        assert_eq!(
            ownership.owner_at(&multipart_grid, 0, 2),
            Ok(&CellOwner::Feature(feature("M")))
        );
    }

    #[test]
    fn domain_ownership_handles_antimeridian_and_self_intersection() {
        let seam_grid = Grid::new(
            4,
            1,
            LatLon {
                lat: 1.0,
                lon: 179.0,
            },
            1.0,
            -1.0,
        )
        .expect("fixture grid is valid");
        let seam = Geometry::Polygon(rectangle(179.5, 0.1, -178.5, 0.9));
        let (ownership, _) = CellOwnership::rasterize_features(&[(feature("S"), seam)], seam_grid)
            .expect("antimeridian feature rasterizes");
        for col in 0..3 {
            assert_eq!(
                ownership.owner_at(&seam_grid, 0, col),
                Ok(&CellOwner::Feature(feature("S")))
            );
        }
        assert_eq!(
            ownership.owner_at(&seam_grid, 0, 3),
            Ok(&CellOwner::Unassigned(UnassignedReason::OutsideFeatures))
        );

        let grid = Grid::new(2, 2, LatLon { lat: 2.0, lon: 0.0 }, 1.0, -1.0)
            .expect("fixture grid is valid");
        let bow_tie = Polygon {
            rings: vec![vec![
                LatLon { lat: 0.0, lon: 0.0 },
                LatLon { lat: 2.0, lon: 2.0 },
                LatLon { lat: 2.0, lon: 0.0 },
                LatLon { lat: 0.0, lon: 2.0 },
                LatLon { lat: 0.0, lon: 0.0 },
            ]],
        };
        let (ownership, _) =
            CellOwnership::rasterize_features(&[(feature("X"), Geometry::Polygon(bow_tie))], grid)
                .expect("self-intersecting ring rasterizes with even-odd membership");
        for row in 0..2 {
            for col in 0..2 {
                assert_eq!(
                    ownership.owner_at(&grid, row, col),
                    Ok(&CellOwner::Feature(feature("X")))
                );
            }
        }
    }

    #[test]
    fn domain_ownership_is_independent_of_feature_input_order() {
        let grid = grid(2, 1);
        let first = (
            feature("A"),
            Geometry::Polygon(rectangle(0.0, 0.0, 1.0, 1.0)),
        );
        let second = (
            feature("B"),
            Geometry::Polygon(rectangle(0.5, 0.0, 1.5, 1.0)),
        );
        let (forward, _) =
            CellOwnership::rasterize_features(&[first.clone(), second.clone()], grid)
                .expect("valid features rasterize");
        let (reverse, _) = CellOwnership::rasterize_features(&[second, first], grid)
            .expect("valid features rasterize in reverse order");

        assert_eq!(forward, reverse);
    }

    #[test]
    fn domain_ownership_reports_degenerate_components_by_feature() {
        let grid = grid(2, 1);
        let degenerate = Polygon {
            rings: vec![vec![
                LatLon { lat: 0.0, lon: 0.0 },
                LatLon { lat: 0.5, lon: 0.5 },
                LatLon { lat: 1.0, lon: 1.0 },
                LatLon { lat: 0.0, lon: 0.0 },
            ]],
        };
        let geometry = Geometry::MultiPolygon(vec![degenerate, rectangle(1.0, 0.0, 2.0, 1.0)]);
        let (ownership, diagnostics) =
            CellOwnership::rasterize_features(&[(feature("A"), geometry)], grid)
                .expect("degenerate component is diagnostic, not fatal");

        assert_eq!(
            diagnostics,
            vec![crate::domain::GeometryDiagnostic {
                feature_id: feature("A"),
                component: 0,
                reason: DegenerateReason::CollinearVertices,
            }]
        );
        assert_eq!(
            ownership.owner_at(&grid, 0, 1),
            Ok(&CellOwner::Feature(feature("A")))
        );
    }
}

//! Cell membership for a named region on one effective population grid.

use std::fmt;

use crate::geodesy::LatLon;
use crate::grid::{BOUNDARY_TOLERANCE_DEG, Col, Grid, Row};

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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
    #[error(
        "feature {feature_id} component {component} ring {ring} vertex {vertex} has an invalid coordinate"
    )]
    InvalidCoordinate {
        feature_id: String,
        component: usize,
        ring: usize,
        vertex: usize,
    },
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

    /// Rasterize one feature by classifying effective-grid cell centres with even-odd ring
    /// membership. Multipart components are combined and boundary centres are included.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidCoordinate`] when a ring contains a non-finite coordinate or
    /// a coordinate beyond the geographic range and its boundary tolerance.
    pub fn rasterize(
        feature_id: FeatureId,
        geometry: &Geometry,
        grid: Grid,
    ) -> Result<(Self, Vec<GeometryDiagnostic>), DomainError> {
        let polygons = match geometry {
            Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
            Geometry::MultiPolygon(polygons) => polygons,
        };
        let mut prepared = Vec::with_capacity(polygons.len());
        let mut diagnostics = Vec::new();
        for (component, polygon) in polygons.iter().enumerate() {
            if polygon.rings.is_empty() {
                diagnostics.push(GeometryDiagnostic {
                    feature_id: feature_id.clone(),
                    component,
                    reason: DegenerateReason::MissingExteriorRing,
                });
                continue;
            }
            for (ring_index, ring) in polygon.rings.iter().enumerate() {
                for (vertex, point) in ring.iter().enumerate() {
                    if !point.lat.is_finite()
                        || !point.lon.is_finite()
                        || point.lat < -90.0 - BOUNDARY_TOLERANCE_DEG
                        || point.lat > 90.0 + BOUNDARY_TOLERANCE_DEG
                        || point.lon < -180.0 - BOUNDARY_TOLERANCE_DEG
                        || point.lon > 180.0 + BOUNDARY_TOLERANCE_DEG
                    {
                        return Err(DomainError::InvalidCoordinate {
                            feature_id: feature_id.as_str().to_owned(),
                            component,
                            ring: ring_index,
                            vertex,
                        });
                    }
                }
            }
            let exterior = &polygon.rings[0];
            let Some(reason) = degenerate_reason(exterior) else {
                prepared.push(PreparedPolygon::new(polygon));
                continue;
            };
            diagnostics.push(GeometryDiagnostic {
                feature_id: feature_id.clone(),
                component,
                reason,
            });
        }

        let mut rows = Vec::with_capacity(grid.height() as usize);
        for row in grid.rows() {
            let latitude = grid.centre_lat(row);
            let mut selected = Vec::new();
            for polygon in &prepared {
                if latitude < polygon.south || latitude > polygon.north {
                    continue;
                }
                for (start, end) in candidate_columns(polygon, latitude, &grid) {
                    for col_index in start..end {
                        let Some(col) = grid.col(col_index) else {
                            continue;
                        };
                        let longitude = grid.centre_of(row, col).lon;
                        if polygon.contains(latitude, longitude) {
                            selected.push(col_index);
                        }
                    }
                }
            }
            selected.sort_unstable();
            selected.dedup();
            rows.push(columns_to_intervals(selected));
        }
        let domain = Self::from_intervals(feature_id, grid, rows)?;
        Ok((domain, diagnostics))
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

#[derive(Debug)]
struct PreparedPolygon {
    rings: Vec<Vec<LatLon>>,
    south: f64,
    north: f64,
    west: f64,
    east: f64,
}

impl PreparedPolygon {
    fn new(polygon: &Polygon) -> Self {
        let rings: Vec<_> = polygon.rings.iter().map(|ring| unwrap_ring(ring)).collect();
        let exterior = &rings[0];
        let south = exterior
            .iter()
            .map(|point| point.lat)
            .fold(f64::INFINITY, f64::min);
        let north = exterior
            .iter()
            .map(|point| point.lat)
            .fold(f64::NEG_INFINITY, f64::max);
        let west = exterior
            .iter()
            .map(|point| point.lon)
            .fold(f64::INFINITY, f64::min);
        let east = exterior
            .iter()
            .map(|point| point.lon)
            .fold(f64::NEG_INFINITY, f64::max);
        Self {
            rings,
            south,
            north,
            west,
            east,
        }
    }

    fn contains(&self, latitude: f64, longitude: f64) -> bool {
        let Some(exterior) = self.rings.first() else {
            return false;
        };
        if !ring_contains(exterior, latitude, longitude) {
            return false;
        }
        !self
            .rings
            .iter()
            .skip(1)
            .any(|hole| ring_contains(hole, latitude, longitude))
    }
}

fn unwrap_ring(ring: &[LatLon]) -> Vec<LatLon> {
    let Some(first) = ring.first() else {
        return Vec::new();
    };
    let mut result = Vec::with_capacity(ring.len());
    result.push(*first);
    for point in &ring[1..] {
        let previous = result[result.len() - 1].lon;
        let mut longitude = point.lon;
        while longitude - previous > 180.0 {
            longitude -= 360.0;
        }
        while longitude - previous < -180.0 {
            longitude += 360.0;
        }
        result.push(LatLon {
            lat: point.lat,
            lon: longitude,
        });
    }
    result
}

fn ring_contains(ring: &[LatLon], latitude: f64, longitude: f64) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let reference = ring[0].lon;
    let longitude = longitude + 360.0 * ((reference - longitude) / 360.0).round();
    let point = LatLon {
        lat: latitude,
        lon: longitude,
    };
    let mut inside = false;
    let mut previous = ring[ring.len() - 1];
    for current in ring {
        if point_on_segment(point, previous, *current) {
            return true;
        }
        if (previous.lat > latitude) != (current.lat > latitude) {
            let crossing = previous.lon
                + (latitude - previous.lat) * (current.lon - previous.lon)
                    / (current.lat - previous.lat);
            if longitude < crossing {
                inside = !inside;
            }
        }
        previous = *current;
    }
    inside
}

fn point_on_segment(point: LatLon, start: LatLon, end: LatLon) -> bool {
    let dx = end.lon - start.lon;
    let dy = end.lat - start.lat;
    let cross = (point.lon - start.lon) * dy - (point.lat - start.lat) * dx;
    let scale = dx.abs() + dy.abs() + 1.0;
    let tolerance = 32.0 * f64::EPSILON * scale * scale;
    cross.abs() <= tolerance
        && point.lon >= start.lon.min(end.lon) - tolerance
        && point.lon <= start.lon.max(end.lon) + tolerance
        && point.lat >= start.lat.min(end.lat) - tolerance
        && point.lat <= start.lat.max(end.lat) + tolerance
}

fn degenerate_reason(ring: &[LatLon]) -> Option<DegenerateReason> {
    let mut vertices = ring.to_vec();
    if vertices.len() > 1 && vertices.first() == vertices.last() {
        vertices.pop();
    }
    vertices.dedup();
    if vertices.len() < 3 {
        return Some(DegenerateReason::TooFewDistinctVertices);
    }
    let first = vertices[0];
    let second = vertices.iter().copied().find(|point| *point != first)?;
    let has_area = vertices.iter().skip(1).any(|point| {
        let cross = (second.lon - first.lon) * (point.lat - first.lat)
            - (second.lat - first.lat) * (point.lon - first.lon);
        cross != 0.0
    });
    (!has_area).then_some(DegenerateReason::CollinearVertices)
}

fn candidate_columns(polygon: &PreparedPolygon, latitude: f64, grid: &Grid) -> Vec<(u32, u32)> {
    if latitude < polygon.south || latitude > polygon.north {
        return Vec::new();
    }
    let origin = grid.origin().lon;
    let step = grid.lon_step();
    let mut ranges = Vec::new();
    // Geographic coordinates and a grid spanning at most one turn keep ordinary rings in range,
    // including rings that unwrap across the antimeridian.
    for turn in -4..=4 {
        let shift = f64::from(turn) * 360.0;
        let first = ((polygon.west + shift - origin) / step - 0.5)
            .min((polygon.east + shift - origin) / step - 0.5)
            .ceil();
        let last = ((polygon.west + shift - origin) / step - 0.5)
            .max((polygon.east + shift - origin) / step - 0.5)
            .floor();
        if last < 0.0 || first >= f64::from(grid.width()) {
            continue;
        }
        // `first` is integral after ceil, clamped into [0, width), and width is u32.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start = first.max(0.0) as u32;
        // `last` is integral after floor, clamped into [0, width - 1].
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end = (last.min(f64::from(grid.width() - 1)) as u32) + 1;
        if start < end {
            ranges.push((start, end));
        }
    }
    ranges.sort_unstable();
    ranges.dedup();
    ranges
}

fn columns_to_intervals(columns: Vec<u32>) -> Vec<ColumnInterval> {
    let mut intervals = Vec::new();
    let mut start = None;
    let mut previous = 0;
    for column in columns {
        match start {
            None => start = Some(column),
            Some(run_start) if column != previous + 1 => {
                intervals.push(ColumnInterval {
                    start: run_start,
                    end: previous + 1,
                });
                start = Some(column);
            }
            Some(_) => {}
        }
        previous = column;
    }
    if let Some(start) = start {
        intervals.push(ColumnInterval {
            start,
            end: previous + 1,
        });
    }
    intervals
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

impl fmt::Display for GeometryDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "feature {} component {}: {}",
            self.feature_id.as_str(),
            self.component,
            self.reason
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geodesy::LatLon;

    // Fixture construction must fail at its source so tests cannot silently use invalid grids.
    #[allow(clippy::expect_used)]
    fn make_grid(width: u32, height: u32) -> Grid {
        Grid::new(width, height, LatLon { lat: 2.0, lon: 0.0 }, 1.0, -1.0)
            .expect("test grid is valid")
    }

    // Fixture construction must fail at its source so tests cannot silently use invalid identities.
    #[allow(clippy::expect_used)]
    fn id(value: &str) -> FeatureId {
        FeatureId::new(value).expect("test identifier is nonempty")
    }

    fn point(lon: f64, lat: f64) -> LatLon {
        LatLon { lat, lon }
    }

    fn rectangle(west: f64, south: f64, east: f64, north: f64) -> Polygon {
        Polygon {
            rings: vec![vec![
                point(west, south),
                point(east, south),
                point(east, north),
                point(west, north),
                point(west, south),
            ]],
        }
    }

    #[test]
    // Expectations here make invalid constructor fixtures fail at the assertion that creates them.
    #[allow(clippy::expect_used)]
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
    // Expectations here make invalid constructor fixtures fail at the assertion that creates them.
    #[allow(clippy::expect_used)]
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

    #[test]
    // Expectations here make invalid geometry fixtures fail at the assertion that creates them.
    #[allow(clippy::expect_used)]
    fn holes_are_subtracted_and_multipart_components_are_combined() {
        let grid = make_grid(5, 2);
        let mut with_hole = rectangle(0.0, 0.0, 4.0, 2.0);
        with_hole.rings.push(vec![
            point(1.0, 1.0),
            point(3.0, 1.0),
            point(3.0, 1.9),
            point(1.0, 1.9),
            point(1.0, 1.0),
        ]);
        let geometry = Geometry::MultiPolygon(vec![with_hole, rectangle(4.0, 0.0, 5.0, 2.0)]);
        let (domain, diagnostics) =
            Domain::rasterize(id("A"), &geometry, grid).expect("valid polygons rasterize");
        assert_eq!(diagnostics, vec![]);
        assert_eq!(
            domain.row_intervals(grid.row(0).expect("row exists")),
            Some(
                [
                    ColumnInterval { start: 0, end: 1 },
                    ColumnInterval { start: 3, end: 5 },
                ]
                .as_slice()
            )
        );
        assert_eq!(
            domain.row_intervals(grid.row(1).expect("row exists")),
            Some([ColumnInterval { start: 0, end: 5 }].as_slice())
        );
    }

    #[test]
    // Expectations here make invalid geometry fixtures fail at the assertion that creates them.
    #[allow(clippy::expect_used)]
    fn a_cell_centre_on_a_shared_edge_belongs_to_both_domains() {
        let grid = Grid::new(2, 1, point(-0.5, 1.0), 1.0, -1.0).expect("shared-edge grid is valid");
        let left = Domain::rasterize(
            id("left"),
            &Geometry::Polygon(rectangle(-1.0, 0.0, 1.0, 1.0)),
            grid,
        )
        .expect("left polygon rasterizes")
        .0;
        let right = Domain::rasterize(
            id("right"),
            &Geometry::Polygon(rectangle(1.0, 0.0, 3.0, 1.0)),
            grid,
        )
        .expect("right polygon rasterizes")
        .0;
        let row = grid.row(0).expect("row exists");
        let shared = grid.col(1).expect("shared centre column exists");
        assert!(
            left.contains(&grid, row, shared)
                .expect("left grid matches")
        );
        assert!(
            right
                .contains(&grid, row, shared)
                .expect("right grid matches")
        );
    }

    #[test]
    // Expectations here make invalid geometry fixtures fail at the assertion that creates them.
    #[allow(clippy::expect_used)]
    fn antimeridian_crossing_geometry_selects_both_sides_of_the_seam() {
        let grid = Grid::new(4, 1, point(178.0, 1.0), 1.0, -1.0).expect("seam grid is valid");
        let geometry = Geometry::Polygon(rectangle(179.0, 0.0, -179.0, 1.0));
        let (domain, _) =
            Domain::rasterize(id("seam"), &geometry, grid).expect("seam polygon rasterizes");
        assert_eq!(
            domain.row_intervals(grid.row(0).expect("row exists")),
            Some([ColumnInterval { start: 1, end: 3 }].as_slice())
        );
    }

    #[test]
    // Expectations here make invalid geometry fixtures fail at the assertion that creates them.
    #[allow(clippy::expect_used)]
    fn even_odd_membership_keeps_self_intersections_and_rasterizes_again_identically() {
        let grid = make_grid(4, 4);
        let geometry = Geometry::Polygon(Polygon {
            rings: vec![vec![
                point(0.0, 0.0),
                point(4.0, 4.0),
                point(0.0, 4.0),
                point(4.0, 0.0),
                point(0.0, 0.0),
            ]],
        });
        let first = Domain::rasterize(id("self-crossing"), &geometry, grid)
            .expect("self-intersection is evaluated");
        let second =
            Domain::rasterize(id("self-crossing"), &geometry, grid).expect("same input rasterizes");
        assert_eq!(first, second);
        assert_eq!(first.1, vec![]);
        assert_eq!(
            first.0.row_intervals(grid.row(0).expect("row exists")),
            Some([ColumnInterval { start: 1, end: 3 }].as_slice())
        );
        assert_eq!(
            first.0.row_intervals(grid.row(1).expect("row exists")),
            Some([ColumnInterval { start: 0, end: 4 }].as_slice())
        );
    }

    #[test]
    // Expectations here make invalid geometry fixtures fail at the assertion that creates them.
    #[allow(clippy::expect_used)]
    fn polygons_without_cell_centres_are_empty_and_degenerate_components_are_reported() {
        let grid = make_grid(2, 2);
        let (empty, diagnostics) = Domain::rasterize(
            id("tiny"),
            &Geometry::Polygon(rectangle(0.1, 0.1, 0.4, 0.4)),
            grid,
        )
        .expect("small valid polygon rasterizes");
        assert_eq!(diagnostics, vec![]);
        assert!(empty.rows.iter().all(Vec::is_empty));

        let degenerate = Geometry::MultiPolygon(vec![Polygon {
            rings: vec![vec![point(0.0, 0.0), point(1.0, 1.0), point(2.0, 2.0)]],
        }]);
        let (empty, diagnostics) =
            Domain::rasterize(id("bad-feature"), &degenerate, grid).expect("degenerate is noted");
        assert!(empty.rows.iter().all(Vec::is_empty));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].feature_id.as_str(), "bad-feature");
        assert_eq!(diagnostics[0].component, 0);
        assert_eq!(diagnostics[0].reason, DegenerateReason::CollinearVertices);
        assert!(diagnostics[0].to_string().contains("bad-feature"));
    }

    #[test]
    #[allow(clippy::expect_used)] // The fixture pins the registered source's floating-point edge case.
    fn coordinate_boundary_tolerance_accepts_source_rounding_without_repair() {
        let grid = Grid::new(2, 1, point(179.0, -77.0), 1.0, -1.0)
            .expect("small antimeridian grid is valid");
        let rounded_edge = 180.000_000_000_000_1;
        let geometry = Geometry::Polygon(Polygon {
            rings: vec![vec![
                point(179.0, -78.0),
                point(rounded_edge, -78.0),
                point(rounded_edge, -77.0),
                point(179.0, -78.0),
            ]],
        });
        assert!(Domain::rasterize(id("AQ"), &geometry, grid).is_ok());

        let outside_tolerance = Geometry::Polygon(rectangle(181.0, -78.0, 182.0, -77.0));
        assert!(matches!(
            Domain::rasterize(id("invalid"), &outside_tolerance, grid),
            Err(DomainError::InvalidCoordinate { .. })
        ));
    }
}

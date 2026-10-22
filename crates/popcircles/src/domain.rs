//! Cell membership for a named region on one effective population grid.

use std::fmt;

use crate::geodesy::LatLon;
use crate::grid::{BOUNDARY_TOLERANCE_DEG, Col, Grid, Row};

/// Version of the cell-membership rule used to produce domain intervals.
///
/// Persistence layers include this value in their cache identity so a rule change cannot reuse
/// intervals produced under older semantics.
pub const MEMBERSHIP_RULE_VERSION: u32 = 1;

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
    /// Constructs a non-empty half-open column interval.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Option<Self> {
        if start < end {
            Some(Self { start, end })
        } else {
            None
        }
    }

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
        let (prepared, diagnostics) = prepare_geometry(&feature_id, geometry)?;

        let mut rows = Vec::with_capacity(grid.height() as usize);
        for row in grid.rows() {
            let latitude = grid.centre_lat(row);
            let mut selected = Vec::new();
            for polygon in &prepared {
                if latitude < polygon.south || latitude > polygon.north {
                    continue;
                }
                selected.extend(polygon.row_intervals(latitude, &grid));
            }
            rows.push(merge_intervals(selected));
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

impl Geometry {
    /// Rasterizes positive-area feature contact without changing cell-centre domain masks.
    ///
    /// A horizontal sweep splits each row at ring vertices and edge intersections. Within each
    /// resulting latitude band, polygon edges are linear and preserve their order, so the swept
    /// longitude intervals identify every cell with positive-area interior contact.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidCoordinate`] for coordinates outside the geographic range or
    /// non-finite coordinates.
    pub(crate) fn positive_area_intervals(
        &self,
        feature_id: &FeatureId,
        grid: Grid,
    ) -> Result<(Vec<Vec<ColumnInterval>>, Vec<GeometryDiagnostic>), DomainError> {
        let (prepared, diagnostics) = prepare_geometry(feature_id, self)?;
        let mut rows = Vec::with_capacity(grid.height() as usize);
        for row in grid.rows() {
            let mut intervals = Vec::new();
            for polygon in &prepared {
                intervals.extend(polygon.positive_area_row_intervals(row, &grid));
            }
            rows.push(merge_intervals(intervals));
        }
        Ok((rows, diagnostics))
    }
}

fn prepare_geometry(
    feature_id: &FeatureId,
    geometry: &Geometry,
) -> Result<(Vec<PreparedPolygon>, Vec<GeometryDiagnostic>), DomainError> {
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
    Ok((prepared, diagnostics))
}

#[derive(Debug)]
struct PreparedPolygon {
    rings: Vec<Vec<LatLon>>,
    scan_edges: Vec<Vec<ScanEdge>>,
    south: f64,
    north: f64,
}

impl PreparedPolygon {
    fn new(polygon: &Polygon) -> Self {
        let mut rings: Vec<_> = polygon.rings.iter().map(|ring| unwrap_ring(ring)).collect();
        if let Some(exterior_longitude) = rings.first().and_then(|ring| ring.first()).map(|p| p.lon)
        {
            for hole in rings.iter_mut().skip(1) {
                align_ring_longitude_frame(hole, exterior_longitude);
            }
        }
        let exterior = &rings[0];
        let south = exterior
            .iter()
            .map(|point| point.lat)
            .fold(f64::INFINITY, f64::min);
        let north = exterior
            .iter()
            .map(|point| point.lat)
            .fold(f64::NEG_INFINITY, f64::max);
        let scan_edges = rings.iter().map(|ring| scan_edges(ring)).collect();
        Self {
            rings,
            scan_edges,
            south,
            north,
        }
    }

    #[cfg(test)]
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

    fn row_intervals(&self, latitude: f64, grid: &Grid) -> Vec<ColumnInterval> {
        let Some(exterior) = self.rings.first() else {
            return Vec::new();
        };
        let mut intervals = ring_row_intervals(exterior, latitude, grid);
        for hole in self.rings.iter().skip(1) {
            intervals = subtract_intervals(intervals, &ring_row_intervals(hole, latitude, grid));
        }
        intervals
    }

    fn positive_area_row_intervals(&self, row: Row, grid: &Grid) -> Vec<ColumnInterval> {
        let centre = grid.centre_lat(row);
        let half_height = grid.lat_step().abs() / 2.0;
        let south = (centre - half_height).max(-90.0);
        let north = (centre + half_height).min(90.0);
        if north <= self.south || south >= self.north {
            return Vec::new();
        }
        let mut edges: Vec<ScanEdge> = self
            .scan_edges
            .iter()
            .flatten()
            .copied()
            .filter(|edge| edge.max_lat() > south && edge.min_lat() < north)
            .collect();
        if edges.is_empty() {
            return Vec::new();
        }
        edges.sort_unstable_by(|first, second| first.min_lon().total_cmp(&second.min_lon()));

        let mut events = vec![south, north];
        for edge in &edges {
            for latitude in [edge.a.lat, edge.b.lat] {
                if south < latitude && latitude < north {
                    events.push(latitude);
                }
            }
        }
        for (index, first) in edges.iter().enumerate() {
            for second in &edges[index + 1..] {
                if second.min_lon() > first.max_lon() {
                    break;
                }
                if let Some(latitude) = segment_intersection_lat(*first, *second)
                    && south < latitude
                    && latitude < north
                {
                    events.push(latitude);
                }
            }
        }
        events.sort_by(f64::total_cmp);
        events.dedup_by(|first, second| first.total_cmp(second).is_eq());

        let mut intervals = Vec::new();
        for band in events.windows(2) {
            let low = band[0];
            let high = band[1];
            if low >= high {
                continue;
            }
            let latitude = low + (high - low) / 2.0;
            let mut filled = ring_band_spans(&self.scan_edges[0], latitude);
            for hole in self.scan_edges.iter().skip(1) {
                for removed in ring_band_spans(hole, latitude) {
                    filled = subtract_band_spans(filled, removed, latitude);
                }
            }
            for span in filled {
                let west = span
                    .left
                    .longitude_at(low)
                    .min(span.left.longitude_at(high));
                let east = span
                    .right
                    .longitude_at(low)
                    .max(span.right.longitude_at(high));
                append_positive_area_span(west, east, grid, &mut intervals);
            }
        }
        merge_intervals(intervals)
    }
}

#[derive(Debug, Clone, Copy)]
struct ScanEdge {
    a: LatLon,
    b: LatLon,
}

impl ScanEdge {
    fn min_lat(self) -> f64 {
        self.a.lat.min(self.b.lat)
    }

    fn max_lat(self) -> f64 {
        self.a.lat.max(self.b.lat)
    }

    fn min_lon(self) -> f64 {
        self.a.lon.min(self.b.lon)
    }

    fn max_lon(self) -> f64 {
        self.a.lon.max(self.b.lon)
    }

    fn longitude_at(self, latitude: f64) -> f64 {
        self.a.lon + (latitude - self.a.lat) * (self.b.lon - self.a.lon) / (self.b.lat - self.a.lat)
    }
}

#[derive(Debug, Clone, Copy)]
struct BandSpan {
    left: ScanEdge,
    right: ScanEdge,
}

fn scan_edges(ring: &[LatLon]) -> Vec<ScanEdge> {
    if ring.len() < 2 {
        return Vec::new();
    }
    let mut edges: Vec<_> = ring
        .windows(2)
        .map(|pair| ScanEdge {
            a: pair[0],
            b: pair[1],
        })
        .collect();
    if ring.first() != ring.last() {
        let Some(last) = ring.last() else {
            return edges;
        };
        edges.push(ScanEdge {
            a: *last,
            b: ring[0],
        });
    }
    edges
}

fn ring_band_spans(edges: &[ScanEdge], latitude: f64) -> Vec<BandSpan> {
    let mut crossings: Vec<_> = edges
        .iter()
        .copied()
        .filter(|edge| (edge.a.lat > latitude) != (edge.b.lat > latitude))
        .map(|edge| (edge.longitude_at(latitude), edge))
        .collect();
    crossings.sort_by(|first, second| first.0.total_cmp(&second.0));
    crossings
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| BandSpan {
            left: pair[0].1,
            right: pair[1].1,
        })
        .collect()
}

fn subtract_band_spans(source: Vec<BandSpan>, removed: BandSpan, latitude: f64) -> Vec<BandSpan> {
    let removed_left = removed.left.longitude_at(latitude);
    let removed_right = removed.right.longitude_at(latitude);
    let mut result = Vec::new();
    for span in source {
        let left = span.left.longitude_at(latitude);
        let right = span.right.longitude_at(latitude);
        if removed_right <= left || removed_left >= right {
            result.push(span);
            continue;
        }
        if removed_left > left {
            result.push(BandSpan {
                left: span.left,
                right: removed.left,
            });
        }
        if removed_right < right {
            result.push(BandSpan {
                left: removed.right,
                right: span.right,
            });
        }
    }
    result
}

#[allow(clippy::float_cmp)] // Parallel edges do not exchange order inside a scan band.
fn segment_intersection_lat(first: ScanEdge, second: ScanEdge) -> Option<f64> {
    let first_lon = first.b.lon - first.a.lon;
    let first_lat = first.b.lat - first.a.lat;
    let second_lon = second.b.lon - second.a.lon;
    let second_lat = second.b.lat - second.a.lat;
    let denominator = first_lon * second_lat - first_lat * second_lon;
    if denominator == 0.0 {
        return None;
    }
    let offset_lon = second.a.lon - first.a.lon;
    let offset_lat = second.a.lat - first.a.lat;
    let first_fraction = (offset_lon * second_lat - offset_lat * second_lon) / denominator;
    let second_fraction = (offset_lon * first_lat - offset_lat * first_lon) / denominator;
    if !(0.0..=1.0).contains(&first_fraction) || !(0.0..=1.0).contains(&second_fraction) {
        return None;
    }
    Some(first.a.lat + first_fraction * first_lat)
}

fn append_positive_area_span(
    west: f64,
    east: f64,
    grid: &Grid,
    intervals: &mut Vec<ColumnInterval>,
) {
    if west >= east {
        return;
    }
    let origin = grid.origin().lon;
    let step = grid.lon_step();
    let grid_other_edge = origin + step * f64::from(grid.width());
    let grid_west = origin.min(grid_other_edge);
    let grid_east = origin.max(grid_other_edge);
    let first_turn = ((grid_west - east) / 360.0).floor() + 1.0;
    let last_turn = ((grid_east - west) / 360.0).ceil() - 1.0;
    let mut turn = first_turn;
    while turn <= last_turn {
        let shift = turn * 360.0;
        let first_position = (west + shift - origin) / step;
        let last_position = (east + shift - origin) / step;
        let low = first_position.min(last_position);
        let high = first_position.max(last_position);
        if high > low {
            // The bounds are clamped to the grid before conversion, so both are integral indices.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let start = low.floor().max(0.0) as u32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let end = high.ceil().min(f64::from(grid.width())) as u32;
            if start < end {
                intervals.push(ColumnInterval { start, end });
            }
        }
        turn += 1.0;
    }
}

#[allow(clippy::float_cmp)] // Exact horizontal edges and vertices define boundary-cell membership.
fn ring_row_intervals(ring: &[LatLon], latitude: f64, grid: &Grid) -> Vec<ColumnInterval> {
    if ring.len() < 3 {
        return Vec::new();
    }
    let mut crossings = Vec::new();
    let mut boundary = Vec::new();
    let mut previous = ring[ring.len() - 1];
    for &current in ring {
        if (previous.lat > latitude) != (current.lat > latitude) {
            let longitude = previous.lon
                + (latitude - previous.lat) * (current.lon - previous.lon)
                    / (current.lat - previous.lat);
            crossings.push(longitude);
        }
        if previous.lat == current.lat && previous.lat == latitude {
            append_longitude_span(
                previous.lon.min(current.lon),
                previous.lon.max(current.lon),
                grid,
                &mut boundary,
            );
        } else if previous.lat != current.lat
            && (previous.lat.min(current.lat)..=previous.lat.max(current.lat)).contains(&latitude)
        {
            let longitude = previous.lon
                + (latitude - previous.lat) * (current.lon - previous.lon)
                    / (current.lat - previous.lat);
            append_longitude_span(longitude, longitude, grid, &mut boundary);
        }
        previous = current;
    }
    crossings.sort_by(f64::total_cmp);
    let mut intervals = boundary;
    let (pairs, _) = crossings.as_chunks::<2>();
    for pair in pairs {
        append_longitude_span(pair[0], pair[1], grid, &mut intervals);
    }
    merge_intervals(intervals)
}

fn append_longitude_span(west: f64, east: f64, grid: &Grid, intervals: &mut Vec<ColumnInterval>) {
    let origin = grid.origin().lon;
    let step = grid.lon_step();
    for turn in -4..=4 {
        let shift = f64::from(turn) * 360.0;
        let first = ((west + shift - origin) / step - 0.5)
            .min((east + shift - origin) / step - 0.5)
            .ceil();
        let last = ((west + shift - origin) / step - 0.5)
            .max((east + shift - origin) / step - 0.5)
            .floor();
        if last < 0.0 || first >= f64::from(grid.width()) {
            continue;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start = first.max(0.0) as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end = (last.min(f64::from(grid.width() - 1)) as u32) + 1;
        if start < end {
            intervals.push(ColumnInterval { start, end });
        }
    }
}

fn merge_intervals(mut intervals: Vec<ColumnInterval>) -> Vec<ColumnInterval> {
    intervals.sort_unstable_by_key(|interval| (interval.start, interval.end));
    let mut merged: Vec<ColumnInterval> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        if let Some(previous) = merged.last_mut()
            && interval.start <= previous.end
        {
            previous.end = previous.end.max(interval.end);
            continue;
        }
        merged.push(interval);
    }
    merged
}

fn subtract_intervals(
    source: Vec<ColumnInterval>,
    removed: &[ColumnInterval],
) -> Vec<ColumnInterval> {
    let mut result = Vec::new();
    let mut removed_index = 0;
    for interval in source {
        let mut start = interval.start;
        while removed_index < removed.len() && removed[removed_index].end <= start {
            removed_index += 1;
        }
        let mut index = removed_index;
        while index < removed.len() && removed[index].start < interval.end {
            if removed[index].start > start {
                result.push(ColumnInterval {
                    start,
                    end: removed[index].start,
                });
            }
            start = start.max(removed[index].end);
            if start >= interval.end {
                break;
            }
            index += 1;
        }
        if start < interval.end {
            result.push(ColumnInterval {
                start,
                end: interval.end,
            });
        }
    }
    result
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

fn align_ring_longitude_frame(ring: &mut [LatLon], reference_longitude: f64) {
    let Some(first) = ring.first() else {
        return;
    };
    let offset = 360.0 * ((reference_longitude - first.lon) / 360.0).round();
    for point in ring {
        point.lon += offset;
    }
}

#[cfg(test)]
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

#[cfg(test)]
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
    fn id(value: impl Into<String>) -> FeatureId {
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
    #[allow(clippy::expect_used)] // The slow point classifier is the reference for row intervals.
    fn row_intervals_match_point_classification_for_supported_rings() {
        let grid = Grid::new(16, 8, point(-180.0, 90.0), 22.5, -22.5)
            .expect("global fixture grid is valid");
        let mut holed = rectangle(-90.0, -45.0, 90.0, 45.0);
        holed.rings.push(vec![
            point(-22.5, -22.5),
            point(22.5, -22.5),
            point(22.5, 22.5),
            point(-22.5, 22.5),
            point(-22.5, -22.5),
        ]);
        let geometries = [
            Geometry::Polygon(holed),
            Geometry::Polygon(rectangle(157.5, -45.0, -157.5, 45.0)),
            Geometry::Polygon(Polygon {
                rings: vec![vec![
                    point(-90.0, -45.0),
                    point(90.0, 45.0),
                    point(-90.0, 45.0),
                    point(90.0, -45.0),
                    point(-90.0, -45.0),
                ]],
            }),
        ];

        for (index, geometry) in geometries.iter().enumerate() {
            let (domain, _) = Domain::rasterize(id(format!("row-mask-{index}")), geometry, grid)
                .expect("reference geometry rasterizes");
            let polygons = match geometry {
                Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
                Geometry::MultiPolygon(polygons) => polygons,
            };
            let prepared: Vec<_> = polygons.iter().map(PreparedPolygon::new).collect();
            for row in grid.rows() {
                for col in grid.cols() {
                    let latitude = grid.centre_lat(row);
                    let longitude = grid.centre_of(row, col).lon;
                    let expected = prepared
                        .iter()
                        .any(|polygon| polygon.contains(latitude, longitude));
                    assert_eq!(
                        domain
                            .contains(&grid, row, col)
                            .expect("matching grid indices"),
                        expected,
                        "geometry {index}, row {}, column {}",
                        row.get(),
                        col.get()
                    );
                }
            }
        }
    }

    #[test]
    #[allow(clippy::expect_used)] // A vertex-centered cell must match explicit boundary inclusion.
    fn row_mask_keeps_a_cell_center_on_an_isolated_ring_vertex() {
        let grid = make_grid(5, 2);
        let polygon = Polygon {
            rings: vec![vec![
                point(0.0, 0.0),
                point(5.0, 0.0),
                point(2.5, 1.5),
                point(0.0, 0.0),
            ]],
        };
        let geometry = Geometry::Polygon(polygon.clone());
        let (domain, _) =
            Domain::rasterize(id("vertex"), &geometry, grid).expect("triangle rasterizes");
        let row = grid.row(0).expect("center row exists");
        let vertex_col = grid.col(2).expect("vertex center column exists");
        assert!(
            domain
                .contains(&grid, row, vertex_col)
                .expect("matching grid index")
        );
        let prepared = PreparedPolygon::new(&polygon);
        for row in grid.rows() {
            for col in grid.cols() {
                let latitude = grid.centre_lat(row);
                let longitude = grid.centre_of(row, col).lon;
                assert_eq!(
                    domain
                        .contains(&grid, row, col)
                        .expect("matching grid indices"),
                    prepared.contains(latitude, longitude)
                );
            }
        }
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

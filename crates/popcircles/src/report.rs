//! The published shape of a result, and the only place in this crate a serde derive appears. ADR 0004:
//! the domain types change when the search changes, so what is serialised is a separate
//! representation with its own version, and a field here is a promise to two renderers and two command
//! surfaces.
//!
//! # The envelope
//!
//! Every document is an [`Envelope`], whose keys come in declaration order because that is the order serde
//! emits them in. `schema_version` is first so a consumer reads the version before anything it might not
//! understand, and `provenance` precedes `result` for the same reason one step out: what produced a
//! document is read before the document.
//!
//! # The earth model, and why it is published
//!
//! [`EarthModel`] attests **which sphere every distance in this document was measured on**. A radius, a
//! great-circle distance and a cap's boundary are all that number's, so a consumer drawing the answer on
//! another earth model is drawing a different figure — and drawing it without complaint, which is why the
//! model is a field rather than a thing a renderer assumes. `geodesy.rs` owns the model
//! ([`EARTH_RADIUS_KM`]); this block is a publication of that owner, so a consumer needs no copy of it.
//!
//! # Provenance, and what it does not attest
//!
//! [`Provenance`] is where a document's **identity** lives: which table it was answered from, where that
//! table sits, and which registered dataset its cells came from. It is absent — the key omitted, not null —
//! from a document whose command read no cached table.
//!
//! A payload's own `digest` or `grid` is a different thing and not a second answer to the same question.
//! [`TableQueryReport`] is the one place the distinction is visible: it carries both in its payload and
//! carries no provenance, because there the table is what the command is *about* rather than what it was
//! answered from.
//!
//! **The `grid` in provenance is attested, in the same sense `digest` and `decimation` are.** The cache
//! header binds the whole geometry, per ADR 0005, so opening one compares it and a table built over
//! another grid is refused rather than answered from. What a document publishes is the caller's spelling
//! of that geometry, which the header accepted within the tolerance the raster reader grants.
//!
//! # The documents
//!
//! `kind` is the envelope's `document` key, which is what a consumer branches on before it reads anything
//! under `result`. [`Document`] is where a payload type declares it.
//!
//! | Payload | `kind` | What it answers |
//! | --- | --- | --- |
//! | [`DistanceReport`] | `distance` | a great-circle distance between two coordinates |
//! | [`GridSummary`] | `grid` | the geometry of a declared grid |
//! | [`TableBuildReport`] | `table-build` | what a summation table build settled, and where it went |
//! | [`TableQueryReport`] | `table-query` | the population of one rectangle of a table |
//! | [`CircleReport`] | `circle` | the population inside one circle a caller named |
//! | [`MostPopulousReport`] | `most-populous` | the most populous circle of a fixed radius |
//! | [`SmallestReport`] | `smallest-circle` | one smallest circle, with no ledger around it |
//! | [`SmallestDocument`] | `smallest` | the smallest circle reaching one share |
//! | [`SweepDocument`] | `sweep` | the smallest circle for each of a range of shares |
//! | [`CountryListDocument`] | `country-list` | the available country identifiers and names |
//!
//! The last two are documents rather than bare payloads because a ledger belongs to the run and not to any
//! one circle, and because a payload meaning either "a circle" or "some circles" would leave a consumer
//! branching on which it got. `smallest-circle` is the circle inside the second of them, published on its
//! own by a snapshot rather than by a command, and it carries a kind because a payload type that can be
//! wrapped is one a consumer can be handed.
//!
//! **A [`SweepDocument`]'s `records` ascend by `target.share`.** That is part of the contract and held by
//! construction, so a renderer plotting share against radius may read them in the order it gets them.
//!
//! # What these numbers are accurate to
//!
//! This sits here rather than in a document of its own: what a published figure is
//! accurate to is what a consumer of this format needs to know, and every field it composes is one this
//! module publishes. Nothing below is a tolerance this crate applied — no candidate is ever discarded by
//! one — so each is a bound on the arithmetic beneath an answer, not a margin around it.
//!
//! **A population is accurate to the slack the document carries.** One rectangle query is within 4 ulp of
//! the magnitude it sums, which is about 4e-6 persons at the registry raster's
//! 7.76e9. A circle is a sum of one such query per grid row it spans, added in
//! [`crate::circle::population`]'s fixed order, so the error composes rather than cancelling:
//! [`crate::smallest::predicate_slack_persons`] is that composition, and it is what
//! `predicate_slack_persons` publishes. Measured on this dataset's half-world circle it is 0.012 persons
//! over the 5 arcmin grid and 0.12 over the 30 arcsec one — against populations of 3.9e9, which is eleven
//! significant figures.
//!
//! **`tolerance_persons` is zero, and it means what it says.** The fixed-radius search reports the exact
//! maximum over the grid's cell centres: refinement runs to single cells, the pruning bound is rounded
//! outward and no tie is discarded (issue #6). So the figure is not "zero because nobody measured" — it is
//! the statement that nothing was traded away, and what separates a reported maximum from the truth is the
//! summation slack above and nothing else.
//!
//! **A radius is a whole kilometre, and the answer is a bracket rather than a point.** The search over
//! radius steps in kilometres, so `radius_km` is the smallest whole kilometre that reaches the target and
//! `short_below` is the kilometre beneath it that does not. Both were measured. Where a probe's margin
//! falls inside the slack the comparison could have gone either way, and then `ambiguity` is present and
//! names the span of probed radii that cannot be separated — a floor on it, because the climb doubles and
//! the radii between two probes were never measured, which is ADR 0007. An absent `ambiguity` is the
//! stronger statement.
//!
//! **The centre is a cell centre, and that is a property of the question rather than an error in the
//! answer.** Every search maximises over the grid's cell centres, so a circle whose centre lies between
//! them is not a candidate: at 30 arcsec that is a 926 m mesh at the equator, at 5 arcmin a 9.3 km one. A
//! consumer wanting the continuum's answer should read a published centre as the best cell and the grid in
//! `provenance` as the resolution that claim is made at. [`CircleReport`] is where the distinction is
//! visible, publishing the coordinate asked for beside the cell it was snapped to.
//!
//! **Distances are on the sphere `earth_model` names**, which is the section above and the reason that
//! field is not optional.
//!
//! # Growth
//!
//! The format grows **additively**: a new field or a new payload type owes no version bump, and
//! [`SCHEMA_VERSION`] rises only for a change an existing consumer would misread — a renamed or removed
//! field, or one whose meaning moved. A consumer should therefore ignore keys it does not know rather than
//! refuse a document carrying them.
//!
//! A present `country_breakdown` lists intersecting domains by descending population contribution,
//! then canonical identifier. Its ratios are unit fractions: inside / country total and inside / circle
//! population. Reconciliation carries the circle-specific remainder and arithmetic bound; provenance
//! carries boundary credit and membership, ownership and aggregation context. An empty `entries` list
//! means accounting ran and found no intersecting domain.

use std::path::Path;

use serde::Serialize;

use crate::country_breakdown::{CountryBreakdown, CountryBreakdownEntry};
use crate::domain::FeatureId;
use crate::geodesy::{EARTH_RADIUS_KM, LatLon, RadiusKm, wrap_lon};
use crate::grid::{Col, Grid, GridIndexError, Row};
use crate::raster::CellTallies;
use crate::search::{MostPopulous, SearchStats};
use crate::settlement::Context;
use crate::smallest::{Ambiguity, Smallest, SmallestStats, Target, share_of};
use crate::table::cache::Identity;
use crate::table::{BuiltTable, ColSpan, RowBand, Window};

/// Bumped when a change to a document below is not additive — a renamed or removed field, or one
/// whose meaning moved. A new field does not bump it.
pub const SCHEMA_VERSION: u32 = 1;

/// The kind a payload type is, published as the envelope's `document` key.
///
/// An associated constant rather than an argument to [`Envelope::new`], so a kind is a property of the
/// payload type and no call site can wrap one payload under another's name. What the trait cannot do is
/// make the kinds distinct — nothing checks two implementations for a constant they share — which is why
/// `ten_payload_types_carry_ten_distinct_kinds` collects them into a set and counts it, and why another
/// payload type owes that test a line as well as the table above.
pub trait Document {
    /// What a consumer branches on before it reads anything under `result`.
    const KIND: &'static str;
}

/// The registered country features available to country commands.
#[derive(Debug, Clone, Serialize)]
pub struct CountryListDocument {
    countries: Vec<CountryListEntry>,
}

impl Document for CountryListDocument {
    const KIND: &'static str = "country-list";
}

impl CountryListDocument {
    #[must_use]
    pub fn new(countries: Vec<(FeatureId, String)>) -> Self {
        Self {
            countries: countries
                .into_iter()
                .map(|(id, name)| CountryListEntry {
                    id: id.as_str().to_owned(),
                    name,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CountryListEntry {
    id: String,
    name: String,
}

/// The earth model every distance in a document was measured on.
///
/// `model` names the shape and `radius_km` is that shape's one parameter, so a consumer reads the pair
/// rather than inferring an ellipsoid from a missing field. Both are read from `geodesy.rs`, which is why
/// `Self::SPHERE` is the only value this crate constructs.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct EarthModel {
    model: &'static str,
    radius_km: f64,
}

impl EarthModel {
    /// The model `geodesy.rs` holds, read from its constant rather than restated.
    const SPHERE: Self = Self {
        model: "sphere",
        radius_km: EARTH_RADIUS_KM,
    };
}

/// Every document the program writes.
///
/// `schema_version` is declared first because serde emits struct fields in declaration order, so a
/// consumer reads the version before anything it might not understand, and `document` second because
/// which document this is decides what the rest of it means. `provenance` precedes `result` for the same
/// reason one step out: what produced a document is read before the document.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope<T> {
    schema_version: u32,
    document: &'static str,
    tool: &'static str,
    tool_version: &'static str,
    earth_model: EarthModel,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<Provenance>,
    result: T,
}

impl<T: Document> Envelope<T> {
    /// A document with no provenance to declare, which is every command that reads no cached table.
    #[must_use]
    pub const fn new(result: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            document: T::KIND,
            // This crate, not whichever binary is writing: the format is the library's, and stamping
            // the caller's name would make one document's producer unidentifiable from another's.
            tool: env!("CARGO_PKG_NAME"),
            tool_version: env!("CARGO_PKG_VERSION"),
            earth_model: EarthModel::SPHERE,
            provenance: None,
            result,
        }
    }

    /// The fields are spelled out rather than updated over [`Self::new`]: a functional update would drop
    /// the `None` it replaces, and dropping a value with glue is not something a `const fn` may do.
    #[must_use]
    pub const fn with_provenance(result: T, provenance: Provenance) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            document: T::KIND,
            tool: env!("CARGO_PKG_NAME"),
            tool_version: env!("CARGO_PKG_VERSION"),
            earth_model: EarthModel::SPHERE,
            provenance: Some(provenance),
            result,
        }
    }
}

/// The table a command answered from, and where it sits.
///
/// `digest`, `decimation` and `grid` are what opening a cache **attested** to, because the header binds the
/// whole geometry and compares it (ADR 0005). The geometry
/// is compared within `BOUNDARY_TOLERANCE_DEG`, so what a document names is the caller's spelling of a
/// grid the header accepted rather than the header's own bits.
///
/// `dataset` is not attested in that sense: it is the registry key the cache header records, so it names
/// what the cells came from rather than what they are, and it is what a consumer reads to credit the raster
/// a figure was drawn from — a key and not a citation, because `data/registry.toml` owns the wording each
/// licence requires and this format is not a second place it lives. Absent — the key omitted, not null —
/// from a table built without one.
#[derive(Debug, Clone, Serialize)]
pub struct Provenance {
    digest: String,
    decimation: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    dataset: Option<String>,
    grid: GridSummary,
    cache: CacheFiles,
}

impl Provenance {
    #[must_use]
    pub fn new(identity: &Identity, dataset: Option<&str>, header: &Path, payload: &Path) -> Self {
        Self {
            digest: hexadecimal(identity.digest),
            decimation: identity.decimation.factor(),
            dataset: dataset.map(str::to_owned),
            grid: GridSummary::from(identity.decimation.grid()),
            cache: CacheFiles::new(header, payload),
        }
    }
}

/// A coordinate as published. Longitude is reduced here, which is the reduction
/// [`Grid::centre_of`] leaves to whatever presents its result.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Coordinate {
    lat: f64,
    lon: f64,
}

impl From<LatLon> for Coordinate {
    fn from(at: LatLon) -> Self {
        Self {
            lat: at.lat,
            lon: wrap_lon(at.lon),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DistanceReport {
    from: Coordinate,
    to: Coordinate,
    great_circle_km: f64,
}

impl Document for DistanceReport {
    const KIND: &'static str = "distance";
}

impl DistanceReport {
    #[must_use]
    pub fn new(from: LatLon, to: LatLon, great_circle_km: f64) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            great_circle_km,
        }
    }
}

/// What a grid is, for a caller that has to decide whether it is the grid it meant. The cell area is
/// the middle row's because area varies by row: one figure stands for the grid's scale only if it
/// says which row it came from, and the middle is the row furthest from both degenerate ends.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GridSummary {
    width: u32,
    height: u32,
    origin: Coordinate,
    lon_step: f64,
    lat_step: f64,
    spans_full_turn: bool,
    middle_row_cell_area_km2: f64,
}

impl Document for GridSummary {
    const KIND: &'static str = "grid";
}

impl From<&Grid> for GridSummary {
    fn from(grid: &Grid) -> Self {
        Self {
            width: grid.width(),
            height: grid.height(),
            origin: grid.origin().into(),
            lon_step: grid.lon_step(),
            lat_step: grid.lat_step(),
            spans_full_turn: grid.spans_full_turn(),
            middle_row_cell_area_km2: grid.middle_row_cell_area_km2(),
        }
    }
}

/// Where every cell of a drained raster went, as published.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CellTalliesReport {
    nodata: u64,
    unexpected_negative: u64,
    zero: u64,
    populated: u64,
    total: u64,
}

impl From<CellTallies> for CellTalliesReport {
    fn from(tallies: CellTallies) -> Self {
        Self {
            nodata: tallies.nodata,
            unexpected_negative: tallies.unexpected_negative,
            zero: tallies.zero,
            populated: tallies.populated,
            total: tallies.total(),
        }
    }
}

/// What a summation table build settled, and where it published the table.
///
/// The digest is a string of hexadecimal rather than a number: it is an identity and not a quantity, and
/// a `u64` past 2^53 does not survive a JSON consumer that parses numbers as doubles. `digest` is what a
/// later query passes back to name the table it wants.
#[derive(Debug, Clone, Serialize)]
pub struct TableBuildReport {
    digest: String,
    decimation: u32,
    grid: GridSummary,
    total_population: f64,
    cells: CellTalliesReport,
    cache: CacheFiles,
}

impl Document for TableBuildReport {
    const KIND: &'static str = "table-build";
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheFiles {
    header: String,
    payload: String,
}

impl CacheFiles {
    /// A path is published with whatever is not UTF-8 replaced, because a document a renderer parses is
    /// UTF-8 and a path is not promised to be.
    fn new(header: &Path, payload: &Path) -> Self {
        Self {
            header: header.to_string_lossy().into_owned(),
            payload: payload.to_string_lossy().into_owned(),
        }
    }
}

impl TableBuildReport {
    #[must_use]
    pub fn new(built: &BuiltTable, header: &Path, payload: &Path) -> Self {
        Self {
            digest: hexadecimal(built.digest),
            decimation: built.decimation.factor(),
            grid: GridSummary::from(built.decimation.grid()),
            total_population: built.total,
            cells: built.tallies.into(),
            cache: CacheFiles::new(header, payload),
        }
    }
}

/// The population of one rectangle of a table, with the rectangle the table resolved the request to.
///
/// `window` is absent when the request was the table's whole extent, which is not a window any pair of
/// coordinates expresses — [`Table::whole`](crate::table::Table::whole) says why.
#[derive(Debug, Clone, Serialize)]
pub struct TableQueryReport {
    digest: String,
    grid: GridSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<WindowReport>,
    rows: RowRange,
    columns: ColRange,
    population: f64,
}

impl Document for TableQueryReport {
    const KIND: &'static str = "table-query";
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WindowReport {
    north: f64,
    south: f64,
    west: f64,
    east: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RowRange {
    north: u32,
    south: u32,
}

/// The columns covered, with the full turn stated rather than left to be inferred from `west` and
/// `east`: on a grid whose columns close, one column and all of them are the same pair of indices.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ColRange {
    west: u32,
    east: u32,
    full_turn: bool,
}

impl TableQueryReport {
    #[must_use]
    pub fn new(
        digest: u64,
        grid: &Grid,
        window: Option<Window>,
        rows: RowBand,
        cols: ColSpan,
        population: f64,
    ) -> Self {
        let columns = match cols {
            ColSpan::FullTurn => ColRange {
                west: 0,
                east: grid.width() - 1,
                full_turn: true,
            },
            ColSpan::Through { west, east } => ColRange {
                west: west.get(),
                east: east.get(),
                full_turn: false,
            },
        };
        Self {
            digest: hexadecimal(digest),
            grid: GridSummary::from(grid),
            window: window.map(|window| WindowReport {
                north: window.north,
                south: window.south,
                west: window.west,
                east: window.east,
            }),
            rows: RowRange {
                north: rows.north().get(),
                south: rows.south().get(),
            },
            columns,
            population,
        }
    }
}

/// One circle a caller named, and the population it holds.
///
/// Both coordinates are published because they are different questions. `requested` is what was asked
/// for; `centre` is the centre of the cell the grid resolved it to, which is where the circle actually
/// sits — up to half a cell away, and 500 m of that at the registry raster's resolution.
#[derive(Debug, Clone, Serialize)]
pub struct CircleReport {
    requested: Coordinate,
    centre: Coordinate,
    row: u32,
    col: u32,
    radius_km: f64,
    population: f64,
    total_population: f64,
    share_of_total: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    nearest_settlement: Option<NearestSettlementReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_breakdown: Option<CountryBreakdownReport>,
}

impl Document for CircleReport {
    const KIND: &'static str = "circle";
}

impl CircleReport {
    /// # Errors
    /// Returns [`GridIndexError`] if `cell` is outside `grid`.
    pub fn new(
        requested: LatLon,
        cell: (Row, Col),
        grid: &Grid,
        radius: RadiusKm,
        population: f64,
        total: f64,
    ) -> Result<Self, GridIndexError> {
        let (row, col) = cell;
        Ok(Self {
            requested: requested.into(),
            centre: grid.centre_of(row, col)?.into(),
            row: row.get(),
            col: col.get(),
            radius_km: radius.km(),
            population,
            total_population: total,
            share_of_total: share_of(population, total),
            nearest_settlement: None,
            country_breakdown: None,
        })
    }

    /// Adds already-computed place context; the report layer does not perform lookups.
    #[must_use]
    pub fn with_nearest_settlement(mut self, context: Option<&Context>) -> Self {
        self.nearest_settlement = context.map(Into::into);
        self
    }

    #[must_use]
    pub fn with_country_breakdown(mut self, breakdown: Option<&CountryBreakdown>) -> Self {
        self.country_breakdown = breakdown.map(Into::into);
        self
    }
}

/// A selected place and the distance policy decision, copied into the published contract.
#[derive(Debug, Clone, Serialize)]
pub struct NearestSettlementReport {
    name: String,
    country: String,
    distance_km: f64,
    short_form: bool,
}

impl From<&Context> for NearestSettlementReport {
    fn from(context: &Context) -> Self {
        Self {
            name: context.name().to_owned(),
            country: context.country().to_owned(),
            distance_km: context.distance_km(),
            short_form: context.short_form(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CountryBreakdownReport {
    entries: Vec<CountryBreakdownEntryReport>,
    reconciliation: CountryReconciliationReport,
    provenance: CountryBreakdownProvenanceReport,
}

#[derive(Debug, Clone, Serialize)]
struct CountryBreakdownEntryReport {
    id: String,
    name: String,
    population_inside: f64,
    country_total_population: f64,
    fraction_of_country_population: Option<f64>,
    fraction_of_circle_population: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct CountryReconciliationReport {
    unassigned_population: f64,
    bound_persons: f64,
}

#[derive(Debug, Clone, Serialize)]
struct CountryBreakdownProvenanceReport {
    boundary_dataset: String,
    boundary_revision: String,
    boundary_digest: String,
    attribution: String,
    effective_grid: GridSummary,
    decimation: u32,
    accounting_route: String,
    membership_rule_version: u32,
    ownership_rule_version: u32,
    aggregation_order_version: u32,
}

impl From<&CountryBreakdown> for CountryBreakdownReport {
    fn from(value: &CountryBreakdown) -> Self {
        let context = &value.provenance;
        Self {
            entries: value.entries.iter().map(Into::into).collect(),
            reconciliation: CountryReconciliationReport {
                unassigned_population: value.unassigned_population,
                bound_persons: value.bound_persons,
            },
            provenance: CountryBreakdownProvenanceReport {
                boundary_dataset: context.boundary_dataset.clone(),
                boundary_revision: context.boundary_revision.clone(),
                boundary_digest: context.boundary_digest.clone(),
                attribution: context.attribution.clone(),
                effective_grid: GridSummary::from(&context.effective_grid),
                decimation: context.decimation,
                accounting_route: context.accounting_route.clone(),
                membership_rule_version: context.membership_rule_version(),
                ownership_rule_version: context.ownership_rule_version(),
                aggregation_order_version: context.aggregation_order_version(),
            },
        }
    }
}

impl From<&CountryBreakdownEntry> for CountryBreakdownEntryReport {
    fn from(value: &CountryBreakdownEntry) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            population_inside: value.population_inside,
            country_total_population: value.country_total_population,
            fraction_of_country_population: value.fraction_of_country_population,
            fraction_of_circle_population: value.fraction_of_circle_population,
        }
    }
}

/// What a fixed-radius search did beside answering, as published.
///
/// All five counters, because the pair that matters is a ratio: `blocks_pruned` against
/// `blocks_examined` is whether the bound bit at all, and a document carrying only the answer cannot say
/// whether it came from a branch and bound or from a scan wearing its name.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SearchStatsReport {
    levels: u32,
    blocks_examined: u64,
    blocks_pruned: u64,
    circles_evaluated: u64,
    kernels_built: u64,
}

impl From<SearchStats> for SearchStatsReport {
    fn from(stats: SearchStats) -> Self {
        Self {
            levels: stats.levels,
            blocks_examined: stats.blocks_examined,
            blocks_pruned: stats.blocks_pruned,
            circles_evaluated: stats.circles_evaluated,
            kernels_built: stats.kernels_built,
        }
    }
}

/// The most populous circle of a fixed radius, as published.
#[derive(Debug, Clone, Serialize)]
pub struct MostPopulousReport {
    centre: Coordinate,
    row: u32,
    col: u32,
    radius_km: f64,
    population: f64,
    total_population: f64,
    share_of_total: f64,
    tolerance_persons: f64,
    stats: SearchStatsReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    nearest_settlement: Option<NearestSettlementReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_breakdown: Option<CountryBreakdownReport>,
}

impl Document for MostPopulousReport {
    const KIND: &'static str = "most-populous";
}

impl MostPopulousReport {
    /// `total` is the table's own whole extent, which is what makes `share_of_total` this table's answer
    /// rather than a figure from somewhere else.
    /// # Errors
    /// Returns [`GridIndexError`] if the found centre is outside `grid`.
    pub fn new(found: &MostPopulous, grid: &Grid, total: f64) -> Result<Self, GridIndexError> {
        let centre = found.centre;
        Ok(Self {
            centre: grid.centre_of(centre.row, centre.col)?.into(),
            row: centre.row.get(),
            col: centre.col.get(),
            radius_km: found.radius.km(),
            population: centre.population,
            total_population: total,
            share_of_total: share_of(centre.population, total),
            tolerance_persons: found.tolerance_persons,
            stats: found.stats.into(),
            nearest_settlement: None,
            country_breakdown: None,
        })
    }

    /// Adds already-computed place context; the report layer does not perform lookups.
    #[must_use]
    pub fn with_nearest_settlement(mut self, context: Option<&Context>) -> Self {
        self.nearest_settlement = context.map(Into::into);
        self
    }

    #[must_use]
    pub fn with_country_breakdown(mut self, breakdown: Option<&CountryBreakdown>) -> Self {
        self.country_breakdown = breakdown.map(Into::into);
        self
    }
}

/// The share a circle was asked for, resolved against the population it is a share of.
///
/// All three, because two of them are derived and a consumer checking the third against the raster's own
/// total is the check that catches a document answered from the wrong table.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TargetReport {
    share: f64,
    persons: f64,
    total: f64,
}

impl From<Target> for TargetReport {
    fn from(target: Target) -> Self {
        Self {
            share: target.share.get(),
            persons: target.persons,
            total: target.total,
        }
    }
}

/// The radius one kilometre short of the answer, and what it held.
///
/// The other end of the bracket, and the field that makes minimality readable off the document: this
/// population is under the target and the answer's is not, both measured rather than inferred.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ShortBelowReport {
    radius_km: u32,
    population: f64,
}

/// The probed radii the arithmetic could not separate from the target, as published.
///
/// **A floor on the ambiguity rather than the interval**, for the reason [`Ambiguity`] states: the ends are
/// the widest pair a run measured, and the radii between them mostly were not measured at all. A consumer
/// reading this as the interval is reading it as a stronger claim than it is.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct AmbiguityReport {
    lowest_km: u32,
    highest_km: u32,
    /// How many probed radii fell inside, which is what separates a wide span from a sparsely probed one.
    radii: u32,
}

impl From<Ambiguity> for AmbiguityReport {
    fn from(span: Ambiguity) -> Self {
        Self {
            lowest_km: span.lowest_km,
            highest_km: span.highest_km,
            radii: span.radii,
        }
    }
}

/// What a search over radius did beside answering.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SmallestStatsReport {
    radii_evaluated: u64,
    /// Radii answered from a previous run's record instead of searched. The two counters sum to the radii
    /// the run settled, so a resumed run reads as the first falling and this one rising by as much.
    radii_reused: u64,
    searched: SearchStatsReport,
}

impl From<SmallestStats> for SmallestStatsReport {
    fn from(stats: SmallestStats) -> Self {
        Self {
            radii_evaluated: stats.radii_evaluated,
            radii_reused: stats.radii_reused,
            searched: stats.searched.into(),
        }
    }
}

/// One smallest circle, as published: the answer, the bracket that proved it, and what the arithmetic
/// beneath it can and cannot separate.
///
/// The radius leads because it is the answer — every other field is what the answer rests on.
#[derive(Debug, Clone, Serialize)]
pub struct SmallestReport {
    radius_km: u32,
    centre: Coordinate,
    row: u32,
    col: u32,
    population: f64,
    target: TargetReport,
    share_achieved: f64,
    /// Absent, rather than null, when the answer is 0 km and there is no radius below it to have proved
    /// short — [`TableQueryReport::window`]'s convention.
    #[serde(skip_serializing_if = "Option::is_none")]
    short_below: Option<ShortBelowReport>,
    covers_whole_grid: bool,
    predicate_slack_persons: f64,
    /// Absent when every radius the run probed was separated from the target by more than the slack above,
    /// which is [`Self::short_below`]'s convention and keeps the ordinary document byte-identical.
    ///
    /// Present, it says `radius_km` is one radius from a span this arithmetic cannot order, so a consumer
    /// reporting it as the minimum is reporting more than the search proved.
    #[serde(skip_serializing_if = "Option::is_none")]
    ambiguity: Option<AmbiguityReport>,
    tolerance_persons: f64,
    stats: SmallestStatsReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    nearest_settlement: Option<NearestSettlementReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_breakdown: Option<CountryBreakdownReport>,
}

impl Document for SmallestReport {
    const KIND: &'static str = "smallest-circle";
}

impl SmallestReport {
    /// # Errors
    /// Returns [`GridIndexError`] if the found centre is outside `grid`.
    pub fn new(found: &Smallest, grid: &Grid) -> Result<Self, GridIndexError> {
        let centre = found.centre;
        Ok(Self {
            radius_km: found.radius_km,
            centre: grid.centre_of(centre.row, centre.col)?.into(),
            row: centre.row.get(),
            col: centre.col.get(),
            population: centre.population,
            target: found.target.into(),
            share_achieved: found.share_achieved,
            short_below: found
                .short_below
                .map(|(radius_km, population)| ShortBelowReport {
                    radius_km,
                    population,
                }),
            covers_whole_grid: found.covers_whole_grid,
            predicate_slack_persons: found.predicate_slack_persons,
            ambiguity: found.ambiguity.map(Into::into),
            tolerance_persons: found.tolerance_persons,
            stats: found.stats.into(),
            nearest_settlement: None,
            country_breakdown: None,
        })
    }

    /// Adds already-computed place context; the report layer does not perform lookups.
    #[must_use]
    pub fn with_nearest_settlement(mut self, context: Option<&Context>) -> Self {
        self.nearest_settlement = context.map(Into::into);
        self
    }

    #[must_use]
    pub fn with_country_breakdown(mut self, breakdown: Option<&CountryBreakdown>) -> Self {
        self.country_breakdown = breakdown.map(Into::into);
        self
    }
}

/// The record of what every radius a run probed held, and where it sits.
///
/// A document-level block rather than a field of [`SmallestReport`], because one run opens one of these
/// and what it holds is a property of the table rather than of any share: a sweep of ninety shares
/// shares one, and putting its path in each circle would write the same path ninety times.
#[derive(Debug, Clone, Serialize)]
pub struct LedgerReport {
    path: String,
    /// How many radii the file holds, which is the only figure that says whether resumption is working.
    radii: usize,
}

impl LedgerReport {
    /// `CacheFiles::new`'s treatment of a path, and for its reason.
    #[must_use]
    pub fn new(path: &Path, radii: usize) -> Self {
        Self {
            path: path.to_string_lossy().into_owned(),
            radii,
        }
    }
}

/// What one command asking for one smallest circle publishes.
///
/// A document of its own rather than a bare [`SmallestReport`], because the ledger belongs at this level
/// and not inside the circle. And a document of its own rather than one shared with [`SweepDocument`]: the
/// two differ in how many circles they carry, so a single payload meaning either "a circle" or "some
/// circles" would leave a consumer branching on which it got.
#[derive(Debug, Clone, Serialize)]
pub struct SmallestDocument {
    ledger: LedgerReport,
    circle: SmallestReport,
}

impl Document for SmallestDocument {
    const KIND: &'static str = "smallest";
}

impl SmallestDocument {
    #[must_use]
    pub const fn new(ledger: LedgerReport, circle: SmallestReport) -> Self {
        Self { ledger, circle }
    }
}

/// The shares a sweep walked, in whole percent, which is the unit the flags take.
///
/// Whole percent rather than the fractions the records carry: a step of a tenth accumulated in f64 makes
/// the third share `0.30000000000000004`, and this block is what a consumer reads to know the walk was
/// over integers.
// The shared suffix is the unit, and these are published field names: `from`, `to` and `step` beside a
// `target.share` that is a fraction is exactly the ambiguity this block exists to remove.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SweepShares {
    from_percent: u32,
    to_percent: u32,
    step_percent: u32,
}

impl SweepShares {
    #[must_use]
    pub const fn new(from_percent: u32, to_percent: u32, step_percent: u32) -> Self {
        Self {
            from_percent,
            to_percent,
            step_percent,
        }
    }
}

/// What one command sweeping a range of shares publishes.
///
/// One `ledger` block for the whole document, because one run opens one: what a ledger records is the
/// maximum at a radius, a property of the table alone, so every share in the sweep reuses what the
/// others paid for.
///
/// **`records` ascends by `target.share`**, and that is part of the contract rather than an accident of
/// how a caller iterated — [`Self::new`] orders them, so a renderer plotting share against radius may
/// read them in the order it gets them.
#[derive(Debug, Clone, Serialize)]
pub struct SweepDocument {
    ledger: LedgerReport,
    shares: SweepShares,
    records: Vec<SmallestReport>,
}

impl Document for SweepDocument {
    const KIND: &'static str = "sweep";
}

impl SweepDocument {
    /// Ordering here rather than asking a caller for it, so the contract above holds by construction.
    /// `total_cmp` because a share is an ordinary finite proportion and a total order needs no case for
    /// the `NaN` [`Share`](crate::smallest::Share) refuses.
    #[must_use]
    pub fn new(
        ledger: LedgerReport,
        shares: SweepShares,
        mut records: Vec<SmallestReport>,
    ) -> Self {
        records.sort_by(|a, b| a.target.share.total_cmp(&b.target.share));
        Self {
            ledger,
            shares,
            records,
        }
    }
}

/// The one spelling of a digest, so the string a build publishes is the string a query accepts.
fn hexadecimal(digest: u64) -> String {
    format!("{digest:#018x}")
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp is the point rather than a
// concession: the fixture's cells are small distinct integers, so every figure a document below carries
// is an exact f64 and a tolerance would let a dropped row pass. cast_precision_loss likewise — the
// largest cell is 648, which u32 -> f32 holds exactly.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::convert::Infallible;
    use std::num::NonZeroU32;

    use super::*;
    use crate::circle;
    use crate::country_breakdown::{CountryBreakdownEntry, CountryBreakdownProvenance};
    use crate::geodesy::great_circle_km;
    use crate::kernel::Kernel;
    use crate::raster::Synthetic;
    use crate::search::{self, Candidate};
    use crate::settlement::Place;
    use crate::smallest::{self, RadiusLedger, Share};
    use crate::table::{Decimation, Table, build};

    /// The three tests below are about the envelope and carry no payload, so the unit they wrap needs a
    /// kind to satisfy the bound. `cfg(test)` because a document with no result is not one this crate
    /// publishes, and an implementation outside the tests would say it could.
    impl Document for () {
        const KIND: &'static str = "envelope-fixture";
    }

    #[test]
    fn ten_payload_types_carry_ten_distinct_kinds() {
        // The trait cannot check this: two implementations sharing a constant compile. So the set is
        // built and counted, which turns a copied `impl` block into a failure here rather than into two
        // documents a consumer cannot tell apart. `()`'s kind is deliberately absent — it is a fixture,
        // not a payload.
        let kinds = BTreeSet::from([
            DistanceReport::KIND,
            GridSummary::KIND,
            TableBuildReport::KIND,
            TableQueryReport::KIND,
            CircleReport::KIND,
            MostPopulousReport::KIND,
            SmallestReport::KIND,
            SmallestDocument::KIND,
            SweepDocument::KIND,
            CountryListDocument::KIND,
        ]);
        assert_eq!(kinds.len(), 10, "{kinds:?}");
    }

    #[test]
    fn the_envelope_leads_with_its_schema_version() {
        // Asserted on the text rather than on a parsed value, because what needs pinning is that
        // the version is the *first* key: a consumer streaming the document reads it before it has
        // to understand anything else.
        let json = serde_json::to_string(&Envelope::new(())).unwrap();
        assert!(json.starts_with(r#"{"schema_version":1,"#), "{json}");
    }

    /// The provenance of a table over [`degree_grid`], for the tests that want one and vary nothing about it
    /// but the dataset.
    fn provenance(dataset: Option<&str>) -> Provenance {
        Provenance::new(
            &Identity {
                digest: 0x3a5d_5e3b_082f_2fb7,
                decimation: Decimation::none(degree_grid()),
            },
            dataset,
            Path::new("out/table.header.json"),
            Path::new("out/table.payload.bin"),
        )
    }

    #[test]
    fn an_envelope_without_provenance_carries_no_key_for_it() {
        // The absent case as a substring rather than as a parsed value: what the skip promises is that
        // the key is not there at all, and a consumer distinguishing absent from null reads the text.
        let json = serde_json::to_string(&Envelope::new(())).unwrap();
        assert!(!json.contains("provenance"), "{json}");
    }

    #[test]
    fn a_provenance_naming_a_dataset_publishes_it_and_one_naming_none_omits_the_key() {
        // The additive half of the field, asserted on the text for
        // `an_envelope_without_provenance_carries_no_key_for_it`'s reason: a document from a table built
        // before this field existed is byte-identical to what it was, so a consumer ignoring the key reads
        // it exactly as before.
        let named = serde_json::to_string(&Envelope::with_provenance(
            (),
            provenance(Some("population-count-2020-30arcsec")),
        ))
        .unwrap();
        assert!(
            named.contains(r#""dataset":"population-count-2020-30arcsec""#),
            "{named}"
        );

        let nameless =
            serde_json::to_string(&Envelope::with_provenance((), provenance(None))).unwrap();
        assert!(!nameless.contains("dataset"), "{nameless}");
    }

    #[test]
    fn provenance_is_published_before_the_result_it_produced() {
        let json = serde_json::to_string(&Envelope::with_provenance((), provenance(None))).unwrap();
        let at = json.find(r#""provenance":"#).expect("the key is emitted");
        let result = json.find(r#""result":"#).expect("the payload is emitted");
        assert!(at < result, "{json}");
    }

    #[test]
    fn a_published_longitude_is_reduced() {
        // Grid::centre_of returns longitudes past 180 for a window crossing the antimeridian, so
        // this conversion is the seam where that stops being the consumer's problem.
        let json = serde_json::to_string(&Coordinate::from(LatLon {
            lat: 12.5,
            lon: 190.0,
        }))
        .unwrap();
        assert_eq!(json, r#"{"lat":12.5,"lon":-170.0}"#);
    }

    // The snapshots below are the wire format itself: they fail on a renamed field, a reordered one
    // and a changed number alike, which is what a document read by two renderers and written by two
    // command surfaces needs. Each input is fixed and named for what makes it a good witness.
    #[test]
    fn the_distance_document_holds_its_shape() {
        // The quarter circumference: a value checkable against the sphere by hand, unlike a pair of
        // cities, so a snapshot accepted by mistake is visible as a wrong number rather than only as
        // a diff.
        let from = LatLon { lat: 0.0, lon: 0.0 };
        let to = LatLon {
            lat: 0.0,
            lon: 90.0,
        };
        // Rounded because sin, cos and atan2 are not bit-identical across libm implementations: the
        // full expansion of this f64 differs in its last digits between arm64 and x86_64, so pinning
        // it would make the snapshot a test of the host rather than of the document. Six decimals is
        // a millimetre, far below anything the sphere model itself is good for. The wire format still
        // carries the unrounded value.
        insta::assert_json_snapshot!(
            Envelope::new(DistanceReport::new(from, to, great_circle_km(from, to))),
            { ".result.great_circle_km" => insta::rounded_redaction(6) }
        );
    }

    /// A one-degree whole-globe grid, small enough to read and closed in longitude, so the full turn in
    /// the query document below is a case this grid actually has.
    fn degree_grid() -> Grid {
        Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a 1 degree whole-globe grid is valid")
    }

    #[test]
    fn the_table_build_document_holds_its_shape() {
        // Assembled rather than built, so the numbers are the ones a reader can check against the fields
        // they land in: the tallies sum to the grid's cell count, and the digest is the value a query
        // has to pass back verbatim.
        let built = BuiltTable {
            digest: 0x3a5d_5e3b_082f_2fb7,
            tallies: CellTallies {
                nodata: 40_000,
                unexpected_negative: 0,
                zero: 8_000,
                populated: 16_800,
            },
            total: 7_757_982_599.32,
            decimation: Decimation::none(degree_grid()),
        };
        insta::assert_json_snapshot!(Envelope::new(TableBuildReport::new(
            &built,
            Path::new("out/table.header.json"),
            Path::new("out/table.payload.bin"),
        )));
    }

    #[test]
    fn the_table_query_document_holds_its_shape() {
        let grid = degree_grid();
        let rows = RowBand::new(
            grid.row(0).expect("a row of the fixture"),
            grid.row(179).expect("a row of the fixture"),
        );
        // The full turn, because that is the case whose `west` and `east` a consumer cannot infer.
        insta::assert_json_snapshot!(Envelope::new(TableQueryReport::new(
            0x3a5d_5e3b_082f_2fb7,
            &grid,
            None,
            rows,
            ColSpan::FullTurn,
            7_757_982_599.32,
        )));
    }

    const FIXTURE_WIDTH: u32 = 36;
    const FIXTURE_HEIGHT: u32 = 18;

    /// The registry raster's sentinel, so a fixture reaches the table by the path a real raster takes.
    const NODATA: f32 = -3.402_823e38;

    /// The fixture every search document below is built over: ten degrees a side, closing in longitude
    /// because [`Kernel::new`] refuses a grid that does not, and small enough that a whole search over it
    /// is a fraction of a second. It is `circle.rs`'s and `search.rs`'s shape, so a figure a document
    /// here carries can be read against the tests that pinned the computation.
    fn fixture_grid() -> Grid {
        Grid::new(
            FIXTURE_WIDTH,
            FIXTURE_HEIGHT,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("a 36 x 18 whole-globe grid is valid")
    }

    /// Distinct at every position and no larger than 648, so every partial sum is exact in f64: a cell
    /// counted twice or dropped moves a published figure rather than hiding in a rounding.
    fn fixture_cell(row: u32, col: u32) -> f32 {
        (row * FIXTURE_WIDTH + col + 1) as f32
    }

    /// The padded payload a real build emits over `cell`, rather than one written out by hand: the
    /// fixture is then the path the search takes and not a second construction of it.
    fn payload_over(grid: &Grid, cell: impl Fn(u32, u32) -> f32) -> Vec<f64> {
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| (0..grid.width()).map(|col| cell(row, col)).collect())
            .collect();
        let source = Synthetic::new(*grid, NODATA, rows).expect("the rows are the grid's shape");
        let mut payload = Vec::new();
        build(source, Decimation::none(*grid), &mut (), |row| {
            payload.extend_from_slice(row);
            Ok::<(), Infallible>(())
        })
        .expect("neither a synthetic source nor this sink can fail");
        payload
    }

    fn fixture_payload() -> Vec<f64> {
        payload_over(&fixture_grid(), fixture_cell)
    }

    fn country_breakdown(grid: Grid, entries: Vec<CountryBreakdownEntry>) -> CountryBreakdown {
        CountryBreakdown {
            entries,
            unassigned_population: 10.0,
            bound_persons: 0.001,
            provenance: CountryBreakdownProvenance {
                boundary_dataset: "gisco-countries-2024-1m".to_owned(),
                boundary_revision: "2024".to_owned(),
                boundary_digest: "sha256:fixture".to_owned(),
                attribution: "GISCO, European Commission".to_owned(),
                effective_grid: grid,
                decimation: 1,
                accounting_route: "sequential-cropped-prefix-v1".to_owned(),
            },
        }
    }

    #[test]
    fn country_breakdown_is_additive_at_every_circle_payload_boundary() {
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("fixture table is valid");
        let requested = LatLon {
            lat: 45.0,
            lon: 15.0,
        };
        let (cell, radius, population, total) = circle_at(&table, requested, 1200.0);
        let most =
            search::most_populous(&table, RadiusKm::from(3000u32), Some(spacing(4)), &mut ())
                .expect("fixture grid can be searched");
        let mut ledger = Recorded::default();
        let found = smallest::smallest(&table, share(0.2), Some(spacing(4)), &mut ledger, &mut ())
            .expect("fixture grid can be searched");
        let breakdown = country_breakdown(
            grid,
            vec![
                CountryBreakdownEntry {
                    id: "A".to_owned(),
                    name: "A".to_owned(),
                    population_inside: 30.0,
                    country_total_population: 100.0,
                    fraction_of_country_population: Some(0.3),
                    fraction_of_circle_population: Some(0.6),
                },
                CountryBreakdownEntry {
                    id: "B".to_owned(),
                    name: "B".to_owned(),
                    population_inside: 10.0,
                    country_total_population: 20.0,
                    fraction_of_country_population: Some(0.5),
                    fraction_of_circle_population: Some(0.2),
                },
                CountryBreakdownEntry {
                    id: "Z".to_owned(),
                    name: "Zero".to_owned(),
                    population_inside: 0.0,
                    country_total_population: 0.0,
                    fraction_of_country_population: None,
                    fraction_of_circle_population: Some(0.0),
                },
            ],
        );
        let empty = country_breakdown(grid, Vec::new());
        let circle = CircleReport::new(requested, cell, &grid, radius, population, total)
            .expect("circle indices belong to the grid")
            .with_country_breakdown(Some(&breakdown));
        let most_report = MostPopulousReport::new(&most, &grid, total)
            .expect("search centre belongs to the grid")
            .with_country_breakdown(Some(&breakdown));
        let smallest_report = SmallestReport::new(&found, &grid)
            .expect("search centre belongs to the grid")
            .with_country_breakdown(Some(&breakdown));
        let reports = serde_json::json!({
            "circle": serde_json::to_value(Envelope::new(circle)).unwrap(),
            "most_populous": serde_json::to_value(Envelope::new(most_report)).unwrap(),
            "smallest_circle": serde_json::to_value(Envelope::new(smallest_report.clone())).unwrap(),
            "smallest": serde_json::to_value(Envelope::new(SmallestDocument::new(
                LedgerReport::new(ledger_path(), ledger.entries.len()), smallest_report.clone(),
            ))).unwrap(),
            "sweep": serde_json::to_value(Envelope::new(SweepDocument::new(
                LedgerReport::new(ledger_path(), ledger.entries.len()),
                SweepShares::new(20, 20, 10), vec![smallest_report],
            ))).unwrap(),
            "legacy_circle": serde_json::to_value(Envelope::new(
                CircleReport::new(requested, cell, &grid, radius, population, total)
                    .expect("circle indices belong to the grid"),
            )).unwrap(),
            "empty": serde_json::to_value(Envelope::new(
                CircleReport::new(requested, cell, &grid, radius, population, total)
                    .expect("circle indices belong to the grid")
                    .with_country_breakdown(Some(&empty)),
            )).unwrap(),
        });
        insta::assert_json_snapshot!(reports);
    }

    /// The population inside a circle of `radius_km` about the cell holding `at`, with the cell.
    ///
    /// The path `population-at` takes: snap the coordinate, build the cap, fold it. So the document below
    /// is over the computation the command performs rather than over an assembled figure.
    fn circle_at(
        table: &Table<'_>,
        at: LatLon,
        radius_km: f64,
    ) -> ((Row, Col), RadiusKm, f64, f64) {
        let grid = *table.grid();
        let cell = grid
            .cell_containing(at)
            .expect("the fixture spans the globe");
        let radius = RadiusKm::new(radius_km).expect("a fixture radius is a length");
        let kernel = Kernel::new(grid, cell.0, radius).expect("a whole-globe grid");
        let (rows, cols) = table.whole();
        (
            cell,
            radius,
            circle::population(table, &kernel, cell.1).expect("circle centre belongs to the grid"),
            table.population(rows, cols),
        )
    }

    #[test]
    fn the_circle_document_holds_its_shape() {
        // A request that lands nowhere near a cell centre, which is the case the two coordinates exist
        // to separate: (48 N, 11 E) falls in the cell centred on (45 N, 15 E), three degrees away in one
        // extent and four in the other. A request already on a centre would pass with the fields swapped.
        //
        // 1200 km about that centre is 10.79 degrees of arc, and on this grid it admits five cells — the
        // centre and its four neighbours, the diagonals being 14.1 degrees away and out:
        //
        //   (3, 19) = 128, (4, 18) = 163, (4, 19) = 164, (4, 20) = 165, (5, 19) = 200
        //
        // which sum to 820 of the fixture's 210 276 people.
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let requested = LatLon {
            lat: 48.0,
            lon: 11.0,
        };
        let (cell, radius, population, total) = circle_at(&table, requested, 1200.0);

        assert_eq!((cell.0.get(), cell.1.get()), (4, 19));
        assert_eq!(population, 820.0);
        assert_eq!(total, 210_276.0);

        insta::assert_json_snapshot!(Envelope::new(
            CircleReport::new(requested, cell, &grid, radius, population, total)
                .expect("circle indices belong to the grid")
        ));
    }

    #[test]
    fn nearest_settlement_is_additive_at_each_circle_payload_boundary() {
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let centre = grid
            .centre_of(grid.row(4).unwrap(), grid.col(19).unwrap())
            .expect("centre belongs to the grid");
        let (cell, radius, population, _) = circle_at(&table, centre, 1200.0);

        let (rows, cols) = table.whole();
        let total = table.population(rows, cols);
        let most_found =
            search::most_populous(&table, RadiusKm::from(3000u32), Some(spacing(4)), &mut ())
                .expect("a whole-globe fixture has kernels");
        let most_centre = grid
            .centre_of(most_found.centre.row, most_found.centre.col)
            .expect("search centre belongs to the grid");

        let mut ledger = Recorded::default();
        let found = smallest::smallest(&table, share(0.2), Some(spacing(4)), &mut ledger, &mut ())
            .expect("a whole-globe fixture and a ledger cannot fail");
        let smallest_centre = grid
            .centre_of(found.centre.row, found.centre.col)
            .expect("search centre belongs to the grid");

        let places = [
            Place::new("Circle", "Example", centre).unwrap(),
            Place::new("Most", "Example", most_centre).unwrap(),
            Place::new("Smallest", "Example", smallest_centre).unwrap(),
        ];
        let circle_context = crate::settlement::nearest_context(centre, &places)
            .unwrap()
            .unwrap();
        let most_context = crate::settlement::nearest_context(most_centre, &places)
            .unwrap()
            .unwrap();
        let smallest_context = crate::settlement::nearest_context(smallest_centre, &places)
            .unwrap()
            .unwrap();

        let circle = Envelope::new(
            CircleReport::new(centre, cell, &grid, radius, population, total)
                .expect("circle indices belong to the grid")
                .with_nearest_settlement(Some(&circle_context)),
        );
        let most_populous = Envelope::new(
            MostPopulousReport::new(&most_found, &grid, total)
                .expect("search centre belongs to the grid")
                .with_nearest_settlement(Some(&most_context)),
        );
        let smallest_report = SmallestReport::new(&found, &grid)
            .expect("search centre belongs to the grid")
            .with_nearest_settlement(Some(&smallest_context));
        let smallest = Envelope::new(SmallestDocument::new(
            LedgerReport::new(ledger_path(), ledger.entries.len()),
            smallest_report.clone(),
        ));
        let sweep = Envelope::new(SweepDocument::new(
            LedgerReport::new(ledger_path(), ledger.entries.len()),
            SweepShares::new(20, 20, 10),
            vec![smallest_report],
        ));

        let ocean = Envelope::new(
            CircleReport::new(centre, cell, &grid, radius, population, total)
                .expect("circle indices belong to the grid")
                .with_nearest_settlement(None),
        );

        insta::assert_json_snapshot!(serde_json::json!({
            "circle": serde_json::to_value(circle).unwrap(),
            "most_populous": serde_json::to_value(most_populous).unwrap(),
            "smallest": serde_json::to_value(smallest).unwrap(),
            "sweep": serde_json::to_value(sweep).unwrap(),
            "ocean": serde_json::to_value(ocean).unwrap(),
        }));
    }

    fn spacing(cells: u32) -> NonZeroU32 {
        NonZeroU32::new(cells).expect("a fixture spacing is not zero")
    }

    #[test]
    fn the_most_populous_document_holds_its_shape() {
        // Over a search rather than an assembled result, so the document is the shape one really takes —
        // and over a search whose bound bit, which `blocks_pruned` is the witness to: a scan wearing the
        // name would publish zero there and pass a snapshot taken of it.
        //
        // The centre is in the south-east, because the fixture's cells grow with row and column. That it
        // is not (0, 0) is the assertion that matters: a fixture whose maximum sat on the north-west cell
        // would pass with `Candidate::better`'s tie-break wired backwards.
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let (rows, cols) = table.whole();
        let total = table.population(rows, cols);

        let found =
            search::most_populous(&table, RadiusKm::from(3000u32), Some(spacing(4)), &mut ())
                .expect("a whole-globe fixture has kernels");
        assert!(found.stats.blocks_pruned > 0, "{:?}", found.stats);
        assert_ne!((found.centre.row.get(), found.centre.col.get()), (0, 0));

        insta::assert_json_snapshot!(Envelope::new(
            MostPopulousReport::new(&found, &grid, total)
                .expect("search centre belongs to the grid")
        ));
    }

    fn share(value: f64) -> Share {
        Share::new(value).expect("a fixture share is a proportion")
    }

    #[test]
    fn the_smallest_document_holds_its_shape() {
        // Over the real search over radius, with no ledger, so the bracket in the document is one a
        // search proved rather than a pair someone wrote down.
        //
        // The two populations are what make it a bracket: the answer's reaches the target and
        // `short_below`'s does not. An off-by-one in the bisection's reporting — publishing the radius
        // two below, or the one above — fails here rather than in a renderer.
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let found = smallest::smallest(&table, share(0.25), Some(spacing(4)), &mut (), &mut ())
            .expect("a whole-globe fixture and a no-op ledger cannot fail");
        let below = found.short_below.expect("the answer is not 0 km");
        assert!(below.1 < found.target.persons, "{found:?}");
        assert!(found.centre.population >= found.target.persons, "{found:?}");
        assert_eq!(below.0, found.radius_km - 1);

        insta::assert_json_snapshot!(Envelope::new(
            SmallestReport::new(&found, &grid).expect("search centre belongs to the grid")
        ));
    }

    #[test]
    fn an_unseparated_answer_publishes_the_span_and_a_separated_one_omits_it() {
        // The other document the field has, over the fixture whose answer the arithmetic cannot separate:
        // four cells of a hundred and nobody else, so every circle holding all four holds the same 400
        // people and every reaching probe's margin against a target of everyone is zero. The span the
        // snapshot pins is 1572 to 2048 km over seven probed radii, which is what a change to how it is
        // accumulated moves; the answer above, over the same document type, has no `ambiguity` key at all.
        let grid = fixture_grid();
        let payload = payload_over(&grid, |row, col| {
            if (8..=9).contains(&row) && (15..=16).contains(&col) {
                100.0
            } else {
                0.0
            }
        });
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let found = smallest::smallest(&table, share(1.0), Some(spacing(4)), &mut (), &mut ())
            .expect("a whole-globe fixture and a no-op ledger cannot fail");
        assert_eq!(found.radius_km, 1572);
        assert!(found.ambiguity.is_some(), "{found:?}");

        insta::assert_json_snapshot!(Envelope::new(
            SmallestReport::new(&found, &grid).expect("search centre belongs to the grid")
        ));
    }

    /// A ledger in a map, so the `radii` a document publishes is a real count and not a figure written
    /// down beside one. `smallest.rs` drives its own tests through the same seam.
    #[derive(Debug, Default)]
    struct Recorded {
        entries: BTreeMap<u32, Candidate>,
    }

    impl RadiusLedger for Recorded {
        type Error = Infallible;

        fn get(&self, km: u32) -> Option<Candidate> {
            self.entries.get(&km).copied()
        }

        fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
            self.entries.insert(km, found);
            Ok(())
        }
    }

    /// Where a run would put its ledger, which is `smallest-for-share`'s default. Fabricated because a
    /// map has no path, and it is the shape of the block that a snapshot pins rather than the location.
    fn ledger_path() -> &'static Path {
        Path::new("out/radii.json")
    }

    #[test]
    fn the_smallest_circle_document_holds_its_shape() {
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let mut ledger = Recorded::default();
        let found = smallest::smallest(&table, share(0.25), Some(spacing(4)), &mut ledger, &mut ())
            .expect("a whole-globe fixture and a map cannot fail");
        // The count is the run's own, which is what makes the block worth publishing at all.
        assert_eq!(ledger.entries.len() as u64, found.stats.radii_evaluated);

        insta::assert_json_snapshot!(Envelope::new(SmallestDocument::new(
            LedgerReport::new(ledger_path(), ledger.entries.len()),
            SmallestReport::new(&found, &grid).expect("search centre belongs to the grid"),
        )));
    }

    #[test]
    fn the_sweep_document_holds_its_shape_and_ascends_by_share() {
        // Ten, twenty and thirty percent through `Share::new` on the exact fractions a percent walk
        // produces, so the published shares are `0.1`, `0.2` and `0.3` and not the residue accumulating a
        // step of a tenth would leave. That is the whole reason the flags are percent.
        //
        // Handed to the constructor in descending order, because what is being pinned is that the type
        // orders them: passing them already sorted would let a constructor that does nothing pass.
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let mut ledger = Recorded::default();
        let mut records: Vec<SmallestReport> = [0.3, 0.2, 0.1]
            .into_iter()
            .map(|value| {
                let found = smallest::smallest(
                    &table,
                    share(value),
                    Some(spacing(4)),
                    &mut ledger,
                    &mut (),
                )
                .expect("a whole-globe fixture and a map cannot fail");
                SmallestReport::new(&found, &grid).expect("search centre belongs to the grid")
            })
            .collect();
        assert_eq!(records.len(), 3);

        let document = SweepDocument::new(
            LedgerReport::new(ledger_path(), ledger.entries.len()),
            SweepShares::new(10, 30, 10),
            std::mem::take(&mut records),
        );
        let shares: Vec<f64> = document
            .records
            .iter()
            .map(|record| record.target.share)
            .collect();
        assert_eq!(shares, vec![0.1, 0.2, 0.3]);

        insta::assert_json_snapshot!(Envelope::new(document));
    }

    #[test]
    fn a_document_wrapping_a_circle_carries_exactly_one_ledger_block() {
        // One run opens one, so a ledger block per record would write the same path once per share. The
        // count is over the serialised text, which is where a field moved into `SmallestReport` would
        // show up as three keys in a sweep of three.
        let grid = fixture_grid();
        let payload = fixture_payload();
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let found = smallest::smallest(&table, share(0.2), Some(spacing(4)), &mut (), &mut ())
            .expect("a whole-globe fixture and a no-op ledger cannot fail");
        let record = SmallestReport::new(&found, &grid).expect("search centre belongs to the grid");
        let block = || LedgerReport::new(ledger_path(), 0);

        let one = serde_json::to_string(&Envelope::new(SmallestDocument::new(
            block(),
            record.clone(),
        )))
        .unwrap();
        let swept = serde_json::to_string(&Envelope::new(SweepDocument::new(
            block(),
            SweepShares::new(20, 20, 10),
            vec![record.clone(), record.clone(), record],
        )))
        .unwrap();

        assert_eq!(one.matches(r#""ledger":"#).count(), 1, "{one}");
        assert_eq!(swept.matches(r#""ledger":"#).count(), 1, "{swept}");
    }

    #[test]
    fn a_circle_over_a_table_holding_nobody_publishes_a_share_of_zero() {
        // Every cell the sentinel, so the build sanitises the lot to zero and the total is nothing. The
        // quotient would be a `NaN`, which serialises as `null` and reaches a renderer as a chart labelled
        // with it — so the text is asserted rather than the value.
        let grid = fixture_grid();
        let payload = payload_over(&grid, |_, _| NODATA);
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let requested = LatLon {
            lat: 48.0,
            lon: 11.0,
        };
        let (cell, radius, population, total) = circle_at(&table, requested, 1200.0);
        assert_eq!((population, total), (0.0, 0.0));

        let report = CircleReport::new(requested, cell, &grid, radius, population, total)
            .expect("circle indices belong to the grid");
        assert_eq!(report.share_of_total, 0.0);
        let json = serde_json::to_string(&Envelope::new(report)).unwrap();
        assert!(!json.contains("null") && !json.contains("NaN"), "{json}");
    }

    #[test]
    fn the_grid_document_holds_its_shape() {
        let grid = Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a 1 degree whole-globe grid is valid");
        insta::assert_json_snapshot!(Envelope::new(GridSummary::from(&grid)));
    }

    #[test]
    fn the_country_list_document_holds_its_shape() {
        let countries = CountryListDocument::new(vec![
            (
                FeatureId::new("AA").expect("valid country ID"),
                "Alpha".to_owned(),
            ),
            (
                FeatureId::new("BB").expect("valid country ID"),
                "Beta".to_owned(),
            ),
        ]);

        insta::assert_json_snapshot!(Envelope::new(countries));
    }
}

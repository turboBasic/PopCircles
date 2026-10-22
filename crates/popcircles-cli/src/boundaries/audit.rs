use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use popcircles::domain::{Domain, MEMBERSHIP_RULE_VERSION};
use popcircles::grid::{Grid, Row};
use popcircles::raster::geotiff::GeoTiffSource;
use popcircles::raster::{PixelType, RasterSource, RasterSpec};
use popcircles::table::Decimation;
use serde::Serialize;

use super::cache::{Cache, Identity};
use super::{BoundaryFeature, load};

const BOUNDARY_KEY: &str = "gisco-countries-2024-1m";
const POPULATION_KEY: &str = "population-count-2020-30arcsec";
const INVALID_SOURCE_IDS: [&str; 9] = ["CR", "DE", "ET", "GN", "UA", "IN", "LR", "MN", "NI"];

#[derive(Debug, Default, Serialize)]
struct FeatureAudit {
    feature_id: String,
    member_cells: u64,
    zero_after_sanitization_cells: Option<u64>,
    positive_population: Option<f64>,
}

#[derive(Debug, Default, Serialize)]
struct OverlapAudit {
    feature_ids: Vec<String>,
    member_cells: u64,
    positive_cells: Option<u64>,
    positive_population: Option<f64>,
}

#[derive(Debug, Serialize)]
struct ResolutionAudit {
    decimation: u32,
    grid: (u32, u32),
    features: Vec<FeatureAudit>,
    overlap_cases: Vec<OverlapAudit>,
    diagnostics: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PopulationAudit {
    world_positive_population: f64,
    assigned_positive_population: f64,
    unassigned_positive_population: f64,
    assigned_zero_after_sanitization_cells: u64,
    raster_cells: u64,
    raster_tallies: (u64, u64, u64, u64),
}

#[derive(Debug, Serialize)]
struct Audit {
    boundary_key: &'static str,
    boundary_path: PathBuf,
    boundary_sha256: String,
    boundary_features: usize,
    source_invalid_ids: [&'static str; 9],
    population_key: &'static str,
    population_path: PathBuf,
    population_sha256: String,
    membership_rule_version: u32,
    k1_population: PopulationAudit,
    k1_masks: ResolutionAudit,
    k10_masks: ResolutionAudit,
}

#[derive(Debug, Default)]
struct Counts {
    cells: u64,
    positive_cells: u64,
    zero_cells: u64,
    population: f64,
}

#[test]
#[ignore = "reads registered GISCO and population data; writes tmpg/gh-13/audit.json"]
#[allow(clippy::expect_used, clippy::similar_names, clippy::too_many_lines)] // The paired k=1/k=10 results are intentionally named in parallel.
fn gisco_registered_masks_audit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry_path = root.join(crate::registry::REGISTRY_PATH);
    let registry = crate::registry::Registry::load(&registry_path).expect("registry reads");
    let boundary = registry
        .boundary(BOUNDARY_KEY)
        .expect("GISCO is a registered boundary vector");
    let boundary_sha256 = boundary
        .sha256
        .as_deref()
        .expect("registered boundary has a source digest")
        .to_owned();
    let features = load(
        &root.join(&boundary.path),
        boundary
            .source_id_field
            .as_deref()
            .expect("registered boundary has an identity field"),
    )
    .expect("registered GISCO GeoJSON decodes");
    assert_eq!(features.len(), 263);
    let ids: BTreeSet<_> = features.iter().map(|feature| feature.id.as_str()).collect();
    assert_eq!(ids.len(), 263);
    assert!(INVALID_SOURCE_IDS.iter().all(|id| ids.contains(id)));

    let population = registry
        .raster(POPULATION_KEY)
        .expect("registered population raster resolves");
    let population_sha256 = population
        .sha256
        .as_deref()
        .expect("registered population raster has a source digest")
        .to_owned();
    let population_path = root.join(&population.raster);
    let grid = Grid::new(
        population.grid.width,
        population.grid.height,
        popcircles::geodesy::LatLon {
            lat: population.grid.origin_lat,
            lon: population.grid.origin_lon,
        },
        population.grid.lon_step,
        population.grid.lat_step,
    )
    .expect("registered population grid is valid");
    let spec = RasterSpec {
        grid,
        epsg: population.epsg,
        pixel: PixelType::Float32,
        nodata: population.nodata,
    };

    let output_dir = root.join("tmpg/gh-13");
    let cache_dir = output_dir.join(format!("masks-{}", std::process::id()));
    let (k1_domains, k1_diagnostics) =
        rasterize_all(&features, grid, 1, &boundary_sha256, &cache_dir);
    let decimation = Decimation::new(grid, 10).expect("factor 10 divides the population grid");
    let (k10_domains, k10_diagnostics) = rasterize_all(
        &features,
        *decimation.grid(),
        10,
        &boundary_sha256,
        &cache_dir,
    );
    assert_domain_identities(&k1_domains, &ids);
    assert_domain_identities(&k10_domains, &ids);
    let (k1_features, k1_overlaps, population_audit) =
        population_audit(&k1_domains, &population_path, &spec);
    let (k10_features, k10_overlaps) = geometry_audit(&k10_domains, *decimation.grid());

    let audit = Audit {
        boundary_key: BOUNDARY_KEY,
        boundary_path: boundary.path.clone(),
        boundary_sha256,
        boundary_features: features.len(),
        source_invalid_ids: INVALID_SOURCE_IDS,
        population_key: POPULATION_KEY,
        population_path: population.raster,
        population_sha256,
        membership_rule_version: MEMBERSHIP_RULE_VERSION,
        k1_population: population_audit,
        k1_masks: ResolutionAudit {
            decimation: 1,
            grid: (grid.width(), grid.height()),
            features: k1_features,
            overlap_cases: k1_overlaps,
            diagnostics: k1_diagnostics,
        },
        k10_masks: ResolutionAudit {
            decimation: 10,
            grid: (decimation.grid().width(), decimation.grid().height()),
            features: k10_features,
            overlap_cases: k10_overlaps,
            diagnostics: k10_diagnostics,
        },
    };
    fs::create_dir_all(&output_dir).expect("audit output directory creates");
    let document = serde_json::to_vec_pretty(&audit).expect("audit serializes");
    fs::write(output_dir.join("audit.json"), &document).expect("audit document writes");
    fs::write(
        output_dir.join("command.txt"),
        "# If either registered input is absent, fetch it with `mise run data:get`.\nmkdir -p tmpg/gh-13/tmp\nTMPDIR=\"$PWD/tmpg/gh-13/tmp\" cargo test --release -p popcircles-cli gisco_registered_masks_audit -- --ignored --nocapture\n",
    )
    .expect("reproduction command writes");
    eprintln!(
        "GISCO mask audit wrote {} bytes to {}",
        document.len(),
        output_dir.join("audit.json").display()
    );
}

fn assert_domain_identities(domains: &[Domain], source_ids: &BTreeSet<&str>) {
    let domain_ids: BTreeSet<_> = domains
        .iter()
        .map(|domain| domain.feature_id().as_str())
        .collect();
    assert_eq!(domain_ids.len(), domains.len());
    assert_eq!(domain_ids, *source_ids);
}

#[allow(clippy::expect_used)] // Fail the audit with the feature/cache that cannot be represented.
fn rasterize_all(
    features: &[BoundaryFeature],
    grid: Grid,
    decimation: u32,
    source_sha256: &str,
    cache_dir: &Path,
) -> (Vec<Domain>, Vec<String>) {
    let mut domains = Vec::with_capacity(features.len());
    let mut diagnostics = Vec::new();
    for feature in features {
        let (domain, noted) = Domain::rasterize(feature.id.clone(), &feature.geometry, grid)
            .unwrap_or_else(|error| {
                panic!(
                    "feature {} rasterization failed: {error}",
                    feature.id.as_str()
                )
            });
        let identity = Identity::new(
            source_sha256,
            feature.id.clone(),
            grid,
            decimation,
            MEMBERSHIP_RULE_VERSION,
        )
        .expect("registered cache identity is valid");
        let base = cache_dir
            .join(format!("k{decimation}"))
            .join(feature.id.as_str());
        Cache::new(base)
            .publish(&domain, &identity)
            .unwrap_or_else(|error| {
                panic!(
                    "feature {} cache publication failed: {error}",
                    feature.id.as_str()
                )
            });
        diagnostics.extend(noted.iter().map(ToString::to_string));
        domains.push(domain);
    }
    (domains, diagnostics)
}

fn geometry_audit(domains: &[Domain], grid: Grid) -> (Vec<FeatureAudit>, Vec<OverlapAudit>) {
    let mut features = make_features(domains, false);
    let mut overlaps = BTreeMap::<Vec<usize>, Counts>::new();
    for row in grid.rows() {
        let events = row_events(row, domains);
        sweep(&events, grid.width(), |columns, members| {
            let count = u64::from(columns.end - columns.start);
            for &member in members {
                features[member].member_cells += count;
            }
            if members.len() > 1 {
                overlaps.entry(members.to_vec()).or_default().cells += count;
            }
        });
    }
    let cases = overlap_cases(overlaps, domains, false);
    (features, cases)
}

#[allow(clippy::expect_used)] // A missing registered raster or failed row is an audit failure.
fn population_audit(
    domains: &[Domain],
    raster_path: &Path,
    spec: &RasterSpec,
) -> (Vec<FeatureAudit>, Vec<OverlapAudit>, PopulationAudit) {
    let mut source =
        GeoTiffSource::open(raster_path, spec).expect("registered population raster opens");
    let mut features = make_features(domains, true);
    let mut overlaps = BTreeMap::<Vec<usize>, Counts>::new();
    let mut world_positive = 0.0;
    let mut assigned_positive = 0.0;
    let mut assigned_zero = 0u64;
    while let Some(result) = source.next_row() {
        let row = result.expect("population row decodes");
        world_positive += row
            .values
            .iter()
            .map(|&value| f64::from(value))
            .sum::<f64>();
        let events = row_events(row.row, domains);
        sweep_with_values(&events, row.values, |columns, members, values| {
            if members.is_empty() {
                return;
            }
            let start = columns.start as usize;
            let end = columns.end as usize;
            let span = &values[start..end];
            let zero = u64::try_from(span.iter().filter(|&&value| value == 0.0).count())
                .unwrap_or(u64::MAX);
            let positive: Vec<f64> = span
                .iter()
                .filter(|&&value| value > 0.0)
                .map(|&value| f64::from(value))
                .collect();
            let positive_cells = u64::try_from(positive.len()).unwrap_or(u64::MAX);
            let sum = positive.iter().sum::<f64>();
            assigned_zero += zero;
            assigned_positive += sum;
            for &member in members {
                features[member].member_cells += u64::from(columns.end - columns.start);
                features[member].zero_after_sanitization_cells = Some(
                    features[member]
                        .zero_after_sanitization_cells
                        .unwrap_or_default()
                        + zero,
                );
                features[member].positive_population =
                    Some(features[member].positive_population.unwrap_or_default() + sum);
            }
            if members.len() > 1 {
                let tally = overlaps.entry(members.to_vec()).or_default();
                tally.cells += u64::from(columns.end - columns.start);
                tally.zero_cells += zero;
                tally.positive_cells += positive_cells;
                tally.population += sum;
            }
        });
    }
    let tallies = source.finish();
    let audit = PopulationAudit {
        world_positive_population: world_positive,
        assigned_positive_population: assigned_positive,
        unassigned_positive_population: world_positive - assigned_positive,
        assigned_zero_after_sanitization_cells: assigned_zero,
        raster_cells: tallies.total(),
        raster_tallies: (
            tallies.nodata,
            tallies.unexpected_negative,
            tallies.zero,
            tallies.populated,
        ),
    };
    (features, overlap_cases(overlaps, domains, true), audit)
}

fn make_features(domains: &[Domain], with_population: bool) -> Vec<FeatureAudit> {
    domains
        .iter()
        .map(|domain| FeatureAudit {
            feature_id: domain.feature_id().as_str().to_owned(),
            zero_after_sanitization_cells: with_population.then_some(0),
            positive_population: with_population.then_some(0.0),
            ..FeatureAudit::default()
        })
        .collect()
}

fn row_events(row: Row, domains: &[Domain]) -> Vec<(u32, usize, bool)> {
    let mut events = Vec::new();
    for (feature, domain) in domains.iter().enumerate() {
        if let Some(intervals) = domain.row_intervals(row) {
            for interval in intervals {
                events.push((interval.start(), feature, true));
                events.push((interval.end(), feature, false));
            }
        }
    }
    events.sort_unstable();
    events
}

fn sweep<F>(events: &[(u32, usize, bool)], width: u32, mut visit: F)
where
    F: FnMut(std::ops::Range<u32>, &[usize]),
{
    sweep_events(events, width, |columns, members| visit(columns, members));
}

fn sweep_with_values<F>(events: &[(u32, usize, bool)], values: &[f32], mut visit: F)
where
    F: FnMut(std::ops::Range<u32>, &[usize], &[f32]),
{
    sweep_events(
        events,
        u32::try_from(values.len()).unwrap_or(u32::MAX),
        |columns, members| visit(columns, members, values),
    );
}

fn sweep_events<F>(events: &[(u32, usize, bool)], width: u32, mut visit: F)
where
    F: FnMut(std::ops::Range<u32>, &[usize]),
{
    let mut active = BTreeSet::new();
    let mut cursor = 0;
    let mut index = 0;
    while index < events.len() {
        let column = events[index].0;
        if cursor < column {
            let members: Vec<_> = active.iter().copied().collect();
            visit(cursor..column, &members);
        }
        while index < events.len() && events[index].0 == column {
            let (_, feature, entering) = events[index];
            if entering {
                active.insert(feature);
            } else {
                active.remove(&feature);
            }
            index += 1;
        }
        cursor = column;
    }
    if cursor < width {
        let members: Vec<_> = active.iter().copied().collect();
        visit(cursor..width, &members);
    }
}

fn overlap_cases(
    overlaps: BTreeMap<Vec<usize>, Counts>,
    domains: &[Domain],
    with_population: bool,
) -> Vec<OverlapAudit> {
    overlaps
        .into_iter()
        .map(|(members, tally)| OverlapAudit {
            feature_ids: members
                .into_iter()
                .map(|member| domains[member].feature_id().as_str().to_owned())
                .collect(),
            member_cells: tally.cells,
            positive_cells: with_population.then_some(tally.positive_cells),
            positive_population: with_population.then_some(tally.population),
        })
        .collect()
}

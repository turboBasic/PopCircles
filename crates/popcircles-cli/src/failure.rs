// The one place a failure becomes an exit code. Every command returns `Failure`, so no arm of `run`
// decides for itself what stdout and stderr are for, and every library error reaching the binary is
// classified here rather than at the call site that met it.
//
// The nine `exit_code_for_*` functions stay private: `Failure`'s constructors are the only callers, and
// a command that needs a class asks for it by naming the error it holds.

use popcircles::grid::GridError;
use popcircles::kernel::KernelError;
use popcircles::raster::RasterError;
use popcircles::search::SearchError;
use popcircles::smallest::SmallestError;
use popcircles::smallest::cache::LedgerError;
use popcircles::table::cache::CacheError;
use popcircles::table::{BuildError, TableError};

use crate::registry::RegistryError;
use crate::settlements::SettlementsError;

/// Bad input, and data that is not there; `application.md`'s "interrupted" class has no caller yet and
/// is not coded here ahead of one.
pub(crate) const EXIT_BAD_INPUT: u8 = 2;

pub(crate) const EXIT_MISSING_DATA: u8 = 3;

/// A failure and the exit-code class it falls in, so every command reports through one path and no
/// arm of `run` decides for itself what stdout and stderr are for.
#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) code: u8,
    pub(crate) message: String,
}

impl Failure {
    pub(crate) fn grid(error: &GridError) -> Self {
        Self::new(exit_code_for_grid_error(error), error)
    }

    pub(crate) fn table(error: &TableError) -> Self {
        Self::new(exit_code_for_table_error(error), error)
    }

    pub(crate) fn raster(error: &RasterError) -> Self {
        Self::new(exit_code_for_raster_error(error), error)
    }

    pub(crate) fn cache(error: &CacheError) -> Self {
        Self::new(exit_code_for_cache_error(error), error)
    }

    pub(crate) fn build(error: &BuildError<CacheError>) -> Self {
        Self::new(exit_code_for_build_error(error), error)
    }

    /// By value rather than by reference, unlike its neighbours: a `KernelError` is one f64 wide, and
    /// clippy's copy threshold is what settles that rather than a preference.
    pub(crate) fn kernel(error: KernelError) -> Self {
        Self::new(exit_code_for_kernel_error(error), &error)
    }

    /// By value for [`Self::kernel`]'s reason: a `SearchError` wraps one and is no wider.
    pub(crate) fn search(error: SearchError) -> Self {
        Self::new(exit_code_for_search_error(error), &error)
    }

    /// Bad input this crate has diagnosed itself, where there is no library error to carry the sentence:
    /// a coordinate off the grid, a window off it, a sweep that runs backwards.
    pub(crate) fn bad_input(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_BAD_INPUT,
            message: message.into(),
        }
    }

    pub(crate) fn registry(error: &RegistryError) -> Self {
        Self::new(exit_code_for_registry_error(error), error)
    }

    pub(crate) fn settlements(error: &SettlementsError) -> Self {
        Self::new(exit_code_for_settlements_error(error), error)
    }

    pub(crate) fn ledger(error: &LedgerError) -> Self {
        Self::new(exit_code_for_ledger_error(error), error)
    }

    pub(crate) fn smallest(error: &SmallestError<LedgerError>) -> Self {
        Self::new(exit_code_for_smallest_error(error), error)
    }

    /// The whole source chain, because a `thiserror` message is one link and the sentence naming the
    /// file or the syscall is usually the one beneath it.
    pub(crate) fn new(code: u8, error: &dyn std::error::Error) -> Self {
        let mut message = error.to_string();
        let mut source = error.source();
        while let Some(error) = source {
            message.push_str(": ");
            message.push_str(&error.to_string());
            source = error.source();
        }
        Self { code, message }
    }
}

pub(crate) const EXIT_FAILURE: u8 = 1;

/// Bad input is the only class a `GridError` has. A variant it gains later has no arm here to fall
/// into, so this crate fails to build until one is added — the property every exhaustive match below
/// exists to hold.
const fn exit_code_for_grid_error(error: &GridError) -> u8 {
    match error {
        GridError::EmptyDimension { .. }
        | GridError::NonFiniteOrigin { .. }
        | GridError::OriginLatOutOfRange { .. }
        | GridError::NonFiniteStep { .. }
        | GridError::ZeroStep { .. }
        | GridError::LatStepNotSouthward { .. }
        | GridError::RunsPastSouthPole { .. }
        | GridError::RunsPastAFullTurn { .. } => EXIT_BAD_INPUT,
    }
}

/// The factor is a flag, so a factor that does not divide the grid is bad input. A payload that is not
/// the grid's padded product is not: nothing the caller said produced it.
const fn exit_code_for_table_error(error: &TableError) -> u8 {
    match error {
        TableError::Decimation { .. } | TableError::DecimatedGrid(_) => EXIT_BAD_INPUT,
        TableError::PayloadLength { .. } => EXIT_FAILURE,
    }
}

/// Every disagreement between the file and the declaration is bad input, because the caller chose the
/// declaration: the registry row `--dataset` names is what the file is being held to, and a file that no
/// longer answers to its own row is a dataset picked wrongly rather than a broken program. Bytes that are
/// not there to read are missing data, which is the class that names `mise run data:get`.
fn exit_code_for_raster_error(error: &RasterError) -> u8 {
    match error {
        RasterError::Dimensions { .. }
        | RasterError::Origin { .. }
        | RasterError::Step { .. }
        | RasterError::Tiled
        | RasterError::SamplesPerPixel { .. }
        | RasterError::PixelFormat { .. }
        | RasterError::RasterType { .. }
        | RasterError::ModelType { .. }
        | RasterError::Epsg { .. }
        | RasterError::ModelTransformation
        | RasterError::NodataMismatch { .. }
        | RasterError::NodataNotANumber { .. }
        | RasterError::NodataMissing { .. }
        | RasterError::MissingTag { .. }
        | RasterError::MissingGeoKey { .. } => EXIT_BAD_INPUT,

        RasterError::Absent { .. } => EXIT_MISSING_DATA,
        RasterError::Io(_) | RasterError::Decode(_) => EXIT_FAILURE,
    }
}

/// A cache that is absent and a cache of some other table share a class, because a caller's answer to
/// both is to build. A cache that is there and broken is neither the caller's doing nor a rebuild away
/// from being trusted, so it is a plain failure and says which file. A held temporary joins it, against
/// the pull of the other class: nothing is missing, and building again is what it refuses.
fn exit_code_for_cache_error(error: &CacheError) -> u8 {
    match error {
        CacheError::Absent { .. }
        | CacheError::FormatVersion { .. }
        | CacheError::ByteOrderMismatch { .. }
        | CacheError::NotThisTable(_) => EXIT_MISSING_DATA,

        CacheError::HeaderRead { .. }
        | CacheError::HeaderWrite { .. }
        | CacheError::HeaderSyntax { .. }
        | CacheError::PayloadRead { .. }
        | CacheError::PayloadWrite { .. }
        | CacheError::PayloadTruncated { .. }
        | CacheError::PayloadTrailing { .. }
        | CacheError::PayloadTemporaryHeld { .. }
        | CacheError::PayloadAlignment => EXIT_FAILURE,
    }
}

fn exit_code_for_build_error(error: &BuildError<CacheError>) -> u8 {
    match error {
        BuildError::Raster(error) => exit_code_for_raster_error(error),
        BuildError::Sink(error) => exit_code_for_cache_error(error),
    }
}

/// A grid whose columns do not close has no kernels at all, and the six grid numbers are flags, so this is
/// bad input rather than a failure.
const fn exit_code_for_kernel_error(error: KernelError) -> u8 {
    match error {
        KernelError::ColumnsDoNotClose { .. } => EXIT_BAD_INPUT,
    }
}

/// One variant, and it is the kernel's, so a search inherits that classification rather than restating it.
const fn exit_code_for_search_error(error: SearchError) -> u8 {
    match error {
        SearchError::Kernel(error) => exit_code_for_kernel_error(error),
    }
}

/// A ledger's grounds split the way [`exit_code_for_cache_error`] splits a cache's, and for that reason: a
/// document describing another table, or one this build reads a different version of, is answered by
/// starting afresh, so it shares the class that names a rebuild. A document that is there and does not
/// hold together — unreadable, not this JSON, or recording two maxima for one radius — is neither the
/// caller's doing nor a rebuild away from being trusted.
///
/// Absent has no arm because it is not an error: opening a
/// [`popcircles::smallest::cache::Ledger`] answers a first run with an empty one.
fn exit_code_for_ledger_error(error: &LedgerError) -> u8 {
    match error {
        LedgerError::FormatVersion { .. } | LedgerError::NotThisTable(_) => EXIT_MISSING_DATA,

        LedgerError::Read { .. }
        | LedgerError::Write { .. }
        | LedgerError::Syntax { .. }
        | LedgerError::CentreOffGrid { .. }
        | LedgerError::DuplicateRadius { .. }
        | LedgerError::TemporaryHeld { .. } => EXIT_FAILURE,
    }
}

/// A name no table can be built from is what the caller typed, so it is bad input. A registry that is not at
/// the path this run resolved is missing data — nothing fetches it, because it is committed, so the message
/// says to run from the repository root instead. One that is there and does not hold together is neither the
/// caller's doing nor a fetch away from being right.
fn exit_code_for_registry_error(error: &RegistryError) -> u8 {
    match error {
        RegistryError::Unknown { .. } | RegistryError::UnknownSettlement { .. } => EXIT_BAD_INPUT,
        RegistryError::Read { .. } => EXIT_MISSING_DATA,
        RegistryError::Syntax { .. }
        | RegistryError::KeyIsNotTheStem { .. }
        | RegistryError::InvalidBoundaryMetadata { .. } => EXIT_FAILURE,
    }
}

/// An absent file is missing data; malformed `GeoJSON` or place properties are bad input.
const fn exit_code_for_settlements_error(error: &SettlementsError) -> u8 {
    match error {
        SettlementsError::Read { .. } => EXIT_MISSING_DATA,
        SettlementsError::Format(_)
        | SettlementsError::Feature { .. }
        | SettlementsError::Empty => EXIT_BAD_INPUT,
    }
}

/// Two arms onto the two layers beneath, so the search over radius invents no class of its own.
fn exit_code_for_smallest_error(error: &SmallestError<LedgerError>) -> u8 {
    match error {
        SmallestError::Search(error) => exit_code_for_search_error(*error),
        SmallestError::Ledger(error) => exit_code_for_ledger_error(error),
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests.
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::path::PathBuf;

    use popcircles::geodesy::LatLon;
    use popcircles::table::cache::Mismatch;

    use super::*;

    #[test]
    fn every_grid_error_is_bad_input() {
        let errors = [
            GridError::EmptyDimension {
                width: 0,
                height: 0,
            },
            GridError::NonFiniteOrigin {
                origin: LatLon {
                    lat: f64::NAN,
                    lon: 0.0,
                },
            },
            GridError::OriginLatOutOfRange { origin_lat: 91.0 },
            GridError::NonFiniteStep {
                lon_step: f64::NAN,
                lat_step: -1.0,
            },
            GridError::ZeroStep {
                lon_step: 0.0,
                lat_step: -1.0,
            },
            GridError::LatStepNotSouthward { lat_step: 1.0 },
            GridError::RunsPastSouthPole { south_edge: -91.0 },
            GridError::RunsPastAFullTurn { lon_span: 361.0 },
        ];
        for error in errors {
            assert_eq!(exit_code_for_grid_error(&error), EXIT_BAD_INPUT);
        }
    }

    // One witness per class rather than a table of every variant: unlike `GridError`, these enums do not
    // map uniformly, and what the exhaustive match holds is that a new variant has to be classified
    // before this crate builds. What the tests hold is which class each one is.
    #[test]
    fn a_raster_the_file_contradicts_is_bad_input_and_bytes_that_are_absent_are_missing_data() {
        assert_eq!(
            exit_code_for_raster_error(&RasterError::Tiled),
            EXIT_BAD_INPUT
        );
        assert_eq!(
            exit_code_for_raster_error(&RasterError::Absent {
                path: std::path::PathBuf::from("data/population/absent.tif")
            }),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_raster_error(&RasterError::Io(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied
            ))),
            EXIT_FAILURE
        );
    }

    #[test]
    fn a_cache_of_another_table_is_missing_data_and_a_broken_one_is_a_failure() {
        assert_eq!(
            exit_code_for_cache_error(&CacheError::Absent {
                path: PathBuf::from("out/table.header.json")
            }),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_cache_error(&CacheError::NotThisTable(Mismatch::Digest {
                wanted: 1,
                found: 2
            })),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_cache_error(&CacheError::PayloadAlignment),
            EXIT_FAILURE
        );
        // Through a build, where the sink's failure is the cache's and keeps its class.
        assert_eq!(
            exit_code_for_build_error(&BuildError::Sink(CacheError::PayloadTruncated {
                expected: 160,
                found: 40
            })),
            EXIT_FAILURE
        );
    }

    #[test]
    fn a_grid_whose_columns_do_not_close_is_bad_input() {
        assert_eq!(
            exit_code_for_kernel_error(KernelError::ColumnsDoNotClose { lon_span: 90.0 }),
            EXIT_BAD_INPUT
        );
        // A search's only failure is the kernel's, and it keeps the kernel's class.
        assert_eq!(
            exit_code_for_search_error(SearchError::Kernel(KernelError::ColumnsDoNotClose {
                lon_span: 90.0
            })),
            EXIT_BAD_INPUT
        );
    }

    #[test]
    fn a_stale_ledger_is_missing_data_and_a_broken_one_is_a_failure() {
        // The split `exit_code_for_cache_error` makes, for its reason: a ledger of another table is
        // answered by starting afresh, and one that does not hold together is not.
        assert_eq!(
            exit_code_for_ledger_error(&LedgerError::NotThisTable(Mismatch::Digest {
                wanted: 1,
                found: 2
            })),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_ledger_error(&LedgerError::Syntax {
                path: PathBuf::from("out/radii.json"),
                source: serde_json::from_str::<u32>("nonsense").expect_err("that is not a number"),
            }),
            EXIT_FAILURE
        );
        // Through the search over radius, where a ledger's failure keeps its own class.
        assert_eq!(
            exit_code_for_smallest_error(&SmallestError::Ledger(LedgerError::NotThisTable(
                Mismatch::DecimationFactor {
                    wanted: 10,
                    found: 1
                }
            ))),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_smallest_error(&SmallestError::Search(SearchError::Kernel(
                KernelError::ColumnsDoNotClose { lon_span: 90.0 }
            ))),
            EXIT_BAD_INPUT
        );
    }

    #[test]
    fn settlement_read_failures_are_missing_data_and_invalid_geojson_is_bad_input() {
        assert_eq!(
            Failure::settlements(&SettlementsError::Read {
                path: PathBuf::from("data/settlements/places.geojson"),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            })
            .code,
            EXIT_MISSING_DATA
        );
        assert_eq!(
            Failure::settlements(&SettlementsError::Empty).code,
            EXIT_BAD_INPUT
        );
        assert_eq!(
            Failure::settlements(&SettlementsError::Format(
                serde_json::from_str::<u32>("not json").expect_err("invalid JSON"),
            ))
            .code,
            EXIT_BAD_INPUT
        );
    }
}

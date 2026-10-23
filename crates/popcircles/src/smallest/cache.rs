// The I/O half of the search over radius: one JSON document holding what every probe found, so an
// interrupted run resumes instead of paying for its radii twice. The same split as the table's, at a tenth of
// the size — `smallest.rs` stays a computation whose tests need no filesystem, and every path, every
// `serde` derive and every write in this work is here.
//
// One document rather than the header-and-payload pair `table/cache.rs` publishes. That shape exists so a
// 7.5 GB payload can be mapped with its cells aligned; a ledger holds a few dozen rows, is read once at
// startup, and is something a person debugging a resumed run wants to be able to `cat`. What it keeps from
// that record is the two things that make a cache safe rather than fast: the identity it is checked
// against, and publication by rename so a reader never sees a half-written document.
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{CountryRadiusLedger, RadiusLedger};
use crate::grid::Grid;
use crate::search::Candidate;
use crate::table::CountryTableIdentity;
use crate::table::cache::{Attestation, Identity, Mismatch};

/// Bumped when a change to the document's fields is one an older reader would misread rather than refuse.
///
/// Additive growth bumps it too, unlike a wire format's version: `serde` ignores keys it does not know, so
/// a build reading a later document would accept it and resume from radii it has half understood. That is
/// `FU-06`'s reasoning for the table's header, and a ledger is the same kind of file.
///
/// Its own constant and not the table's: the two documents share an attestation
/// and are separately versioned, which is what the `version-bumps` hook asks for both of when the shared
/// shape moves.
pub const FORMAT_VERSION: u32 = 3;

/// One suffix, so an interrupted publication leaves one name and no more. Created exclusively, which
/// makes it this ledger's lock as well — [`Ledger::publish`].
const TEMPORARY_SUFFIX: &str = ".tmp";

/// Every way a ledger can fail to be the one a caller asked for.
///
/// One variant per ground, [`CacheError`](crate::table::cache::CacheError)'s reasoning: a caller that
/// starts afresh on a digest mismatch and reports a broken file as an error has to be able to tell the two
/// apart, and a message reading only "stale ledger" sends its reader to a hex editor. The grounds for not
/// being this table are [`Mismatch`]'s, shared with the header rather than spelled again.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("the ledger at {} could not be read", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the ledger at {} could not be written", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the ledger at {} is not the JSON document this format is", path.display())]
    Syntax {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    // The table's [`PayloadTemporaryHeld`](crate::table::cache::CacheError::PayloadTemporaryHeld), for the
    // same ground and with the same action: one mechanism for both documents, which is what stops two
    // answers to one question.
    #[error(
        "a ledger temporary is already at {}: either a run is publishing this ledger now, or one was \
         interrupted and left it — remove it to resume",
        path.display()
    )]
    TemporaryHeld { path: PathBuf },

    #[error("this build reads format version {expected}; the ledger declares {found}")]
    FormatVersion { expected: u32, found: u32 },

    // The noun is the wrapper's and the ground is the attestation's, which is the half of the shared enum
    // that had to be got right: a refusal here names the ledger where the header's names the header, and
    // the two documents cannot drift into one message that reads for only one of them.
    #[error("the ledger does not describe the table wanted: {0}")]
    NotThisTable(Mismatch),

    #[error("the ledger does not describe the country wanted: {0}")]
    NotThisCountry(CountryMismatch),

    #[error("the ledger describes a {found} search, but a {wanted} search was requested")]
    IdentityKind {
        wanted: &'static str,
        found: &'static str,
    },

    #[error(
        "the ledger records a maximum at ({row}, {col}), which is not a cell of a {width} x {height} grid"
    )]
    CentreOffGrid {
        row: u32,
        col: u32,
        width: u32,
        height: u32,
    },

    #[error("the ledger records {km} km twice, with maxima of {first} and {second}")]
    DuplicateRadius { km: u32, first: f64, second: f64 },
}

/// One probe, as published.
///
/// The centre is a pair of indices rather than a [`Candidate`]: a [`Row`](crate::grid::Row) is only ever
/// minted by the grid that contains it, so the indices are checked back into cells at open time and no
/// later reader has to wonder whether they are on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Probe {
    km: u32,
    population: f64,
    row: u32,
    col: u32,
}

/// The one field a document of any version carries, so the version can be compared before the rest is
/// parsed at all.
///
/// The table header's probe and this one are separate structs over the same property — serde ignoring keys
/// a struct does not name — because a test of one says nothing about the other.
#[derive(Debug, Clone, Copy, Deserialize)]
struct DocumentVersion {
    format_version: u32,
}

/// The identity a ledger records, either for a world table or a country prefix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum LedgerIdentity {
    Table(Attestation),
    Country(CountryAttestation),
}

impl LedgerIdentity {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Table(_) => "world-table",
            Self::Country(_) => "country",
        }
    }
}

/// The grid fields that `CountryTableIdentity` compares as part of its complete computation identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GridIdentity {
    width: u32,
    height: u32,
    origin_lat: f64,
    origin_lon: f64,
    lon_step: f64,
    lat_step: f64,
}

impl From<&crate::grid::Grid> for GridIdentity {
    fn from(grid: &crate::grid::Grid) -> Self {
        let origin = grid.origin();
        Self {
            width: grid.width(),
            height: grid.height(),
            origin_lat: origin.lat,
            origin_lon: origin.lon,
            lon_step: grid.lon_step(),
            lat_step: grid.lat_step(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CountryAttestation {
    feature_id: String,
    source_digest: u64,
    boundary_source_digest: String,
    ownership_digest: u64,
    source_grid: GridIdentity,
    effective_grid: GridIdentity,
    decimation: u32,
    ownership_rule_version: u32,
    aggregation_order_version: u32,
}

impl CountryAttestation {
    fn new(identity: &CountryTableIdentity) -> Self {
        Self {
            feature_id: identity.feature_id().as_str().to_owned(),
            source_digest: identity.source_digest(),
            boundary_source_digest: identity.boundary_source_digest().to_owned(),
            ownership_digest: identity.ownership_digest(),
            source_grid: identity.source_grid().into(),
            effective_grid: identity.effective_grid().into(),
            decimation: identity.decimation(),
            ownership_rule_version: identity.ownership_rule_version(),
            aggregation_order_version: identity.aggregation_order_version(),
        }
    }

    fn check(&self, wanted: &Self) -> Result<(), CountryMismatch> {
        let mismatch = if self.feature_id != wanted.feature_id {
            Some(CountryMismatch::FeatureId)
        } else if self.source_digest != wanted.source_digest {
            Some(CountryMismatch::SourceDigest)
        } else if self.boundary_source_digest != wanted.boundary_source_digest {
            Some(CountryMismatch::BoundarySourceDigest)
        } else if self.ownership_digest != wanted.ownership_digest {
            Some(CountryMismatch::OwnershipDigest)
        } else if self.source_grid != wanted.source_grid {
            Some(CountryMismatch::SourceGrid)
        } else if self.effective_grid != wanted.effective_grid {
            Some(CountryMismatch::EffectiveGrid)
        } else if self.decimation != wanted.decimation {
            Some(CountryMismatch::Decimation)
        } else if self.ownership_rule_version != wanted.ownership_rule_version {
            Some(CountryMismatch::OwnershipRuleVersion)
        } else if self.aggregation_order_version != wanted.aggregation_order_version {
            Some(CountryMismatch::AggregationOrderVersion)
        } else {
            None
        };
        mismatch.map_or(Ok(()), Err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CountryMismatch {
    #[error("the feature identifier differs")]
    FeatureId,
    #[error("the decoded population digest differs")]
    SourceDigest,
    #[error("the boundary source digest differs")]
    BoundarySourceDigest,
    #[error("the cell ownership digest differs")]
    OwnershipDigest,
    #[error("the source grid differs")]
    SourceGrid,
    #[error("the effective grid differs")]
    EffectiveGrid,
    #[error("the decimation factor differs")]
    Decimation,
    #[error("the ownership rule version differs")]
    OwnershipRuleVersion,
    #[error("the aggregation order version differs")]
    AggregationOrderVersion,
}

/// The document. Its explicit identity kind prevents a world maximum from being resumed for a country.
/// The share and spacing are omitted because each radius records the maximum for the identity alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
    format_version: u32,
    identity: LedgerIdentity,
    radii: Vec<Probe>,
}

impl Document {
    fn new(identity: LedgerIdentity) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            identity,
            radii: Vec::new(),
        }
    }

    /// The attestation's comparison and nothing else. The version is not here — [`Ledger::open_or_empty`]
    /// reads it out of the document before this struct is parsed, because a document of another version
    /// need not parse into this shape at all.
    fn check(&self, wanted: &LedgerIdentity) -> Result<(), LedgerError> {
        match (&self.identity, wanted) {
            (LedgerIdentity::Table(found), LedgerIdentity::Table(wanted)) => found
                .check_attestation(wanted)
                .map_err(LedgerError::NotThisTable),
            (LedgerIdentity::Country(found), LedgerIdentity::Country(wanted)) => {
                found.check(wanted).map_err(LedgerError::NotThisCountry)
            }
            (found, wanted) => Err(LedgerError::IdentityKind {
                wanted: wanted.kind(),
                found: found.kind(),
            }),
        }
    }
}

/// What every probe of a search over radius found, kept in a file between runs.
///
/// The entries are held resident and the file is rewritten whole on every `put`. Both follow from the size:
/// a search settles a few dozen radii, so the map is kilobytes and the rewrite is one small write against a
/// probe that costs a search over the globe.
#[derive(Debug)]
pub struct Ledger {
    path: PathBuf,
    temporary: PathBuf,
    identity: LedgerIdentity,
    grid: Grid,
    entries: BTreeMap<u32, Candidate>,
}

impl Ledger {
    /// The ledger at `path` for the table `wanted` describes, or an empty one when nothing is there.
    ///
    /// Absent is not an error — a first run has nothing to resume, and making a caller distinguish "no file
    /// yet" from "wrong file" at every call site is how one of the two ends up handled wrongly. Every other
    /// disagreement is refused by ground: see [`LedgerError`].
    ///
    /// # Errors
    /// [`LedgerError::Read`] when the file is there and cannot be read, [`LedgerError::Syntax`] when it is
    /// not this document, and one variant per ground when it describes another table.
    pub fn open_or_empty(path: impl AsRef<Path>, wanted: &Identity) -> Result<Self, LedgerError> {
        Self::open(
            path,
            LedgerIdentity::Table(Attestation::new(wanted)),
            *wanted.decimation.grid(),
        )
    }

    fn open(
        path: impl AsRef<Path>,
        identity: LedgerIdentity,
        grid: Grid,
    ) -> Result<Self, LedgerError> {
        let path = path.as_ref().to_path_buf();
        let mut temporary = path.clone().into_os_string();
        temporary.push(TEMPORARY_SUFFIX);

        let mut ledger = Self {
            temporary: PathBuf::from(temporary),
            path,
            identity,
            grid,
            entries: BTreeMap::new(),
        };

        let bytes = match fs::read(&ledger.path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(ledger),
            Err(source) => {
                return Err(LedgerError::Read {
                    path: ledger.path,
                    source,
                });
            }
        };

        // The version out of the document before the document: a ledger of
        // another version is not required to parse into this build's `Document`, so without this probe a
        // bumped format reports as "not the JSON document this format is" and the constant is decoration.
        let probed: DocumentVersion =
            serde_json::from_slice(&bytes).map_err(|source| LedgerError::Syntax {
                path: ledger.path.clone(),
                source,
            })?;
        if probed.format_version != FORMAT_VERSION {
            return Err(LedgerError::FormatVersion {
                expected: FORMAT_VERSION,
                found: probed.format_version,
            });
        }

        let document: Document =
            serde_json::from_slice(&bytes).map_err(|source| LedgerError::Syntax {
                path: ledger.path.clone(),
                source,
            })?;
        document.check(&ledger.identity)?;

        for probe in document.radii {
            let cell = ledger.grid.row(probe.row).zip(ledger.grid.col(probe.col));
            let (row, col) = cell.ok_or(LedgerError::CentreOffGrid {
                row: probe.row,
                col: probe.col,
                width: ledger.grid.width(),
                height: ledger.grid.height(),
            })?;
            let found = Candidate {
                row,
                col,
                population: probe.population,
            };
            // A radius has one maximum. Two rows for it are two answers to the same question, and a reader
            // that took the last would resume from whichever the writer happened to append second.
            if let Some(first) = ledger.entries.insert(probe.km, found) {
                return Err(LedgerError::DuplicateRadius {
                    km: probe.km,
                    first: first.population,
                    second: probe.population,
                });
            }
        }
        Ok(ledger)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many radii the file holds, which is what a resumed run reuses.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Writes the whole document to a temporary, syncs it, and renames it into place.
    ///
    /// ADR 0005's publication by rename, and the reason is the same: a rename replaces a directory entry
    /// rather than an inode, so a reader either sees the document that was there before or the one this
    /// call finished, never the half-written middle. The temporary is created exclusively, so it is also
    /// this ledger's lock: two runs publishing to one path cannot interleave into a document that parses
    /// and resumes from radii neither of them measured. What that costs is an interrupted run's leftover
    /// refusing the next `put` — [`LedgerError::TemporaryHeld`].
    fn publish(&self) -> Result<(), LedgerError> {
        let mut document = Document::new(self.identity.clone());
        document.radii = self
            .entries
            .iter()
            .map(|(&km, found)| Probe {
                km,
                population: found.population,
                row: found.row.get(),
                col: found.col.get(),
            })
            .collect();

        let write = |path: &Path, source| LedgerError::Write {
            path: path.to_path_buf(),
            source,
        };
        let bytes = serde_json::to_vec(&document)
            .map_err(|source| write(&self.path, io::Error::other(source)))?;

        let mut file = File::create_new(&self.temporary).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                LedgerError::TemporaryHeld {
                    path: self.temporary.clone(),
                }
            } else {
                write(&self.temporary, source)
            }
        })?;
        file.write_all(&bytes)
            .map_err(|source| write(&self.temporary, source))?;
        file.sync_all()
            .map_err(|source| write(&self.temporary, source))?;
        drop(file);
        fs::rename(&self.temporary, &self.path).map_err(|source| write(&self.path, source))
    }
}

/// A resumable ledger whose entries are valid only for one complete country table identity.
#[derive(Debug)]
pub struct CountryLedger {
    inner: Ledger,
}

impl CountryLedger {
    /// The ledger at `path` for this country computation, or an empty ledger when the path is absent.
    ///
    /// # Errors
    /// Returns the same read, format, syntax and identity errors as [`Ledger::open_or_empty`].
    pub fn open_or_empty(
        path: impl AsRef<Path>,
        wanted: &CountryTableIdentity,
    ) -> Result<Self, LedgerError> {
        Ok(Self {
            inner: Ledger::open(
                path,
                LedgerIdentity::Country(CountryAttestation::new(wanted)),
                *wanted.effective_grid(),
            )?,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl RadiusLedger for CountryLedger {
    type Error = LedgerError;

    fn get(&self, km: u32) -> Option<Candidate> {
        self.inner.get(km)
    }

    fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
        self.inner.put(km, found)
    }
}

impl CountryRadiusLedger for CountryLedger {}

impl RadiusLedger for Ledger {
    type Error = LedgerError;

    fn get(&self, km: u32) -> Option<Candidate> {
        self.entries.get(&km).copied()
    }

    /// The entry is added and the document republished before this returns, which is what
    /// [`RadiusLedger::put`] promises and what makes an interrupted run resumable: a probe that has been
    /// paid for is on disk before the next one starts.
    fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
        self.entries.insert(km, found);
        self.publish()
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is here because the round trip's claim is
// bit-identity, which a tolerance would not check.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::convert::Infallible;
    use std::num::NonZeroU32;

    use tempfile::TempDir;

    use super::super::{Share, smallest};
    use super::*;
    use crate::domain::{FeatureId, Geometry, Polygon};
    use crate::geodesy::LatLon;
    use crate::ownership::CellOwnership;
    use crate::raster::Synthetic;
    use crate::table::{CountryTable, Decimation, Table, build, build_country};

    fn grid(width: u32, height: u32) -> Grid {
        Grid::new(
            width,
            height,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            360.0 / f64::from(width),
            -180.0 / f64::from(height),
        )
        .expect("a whole-globe grid of any shape is valid")
    }

    const DIGEST: u64 = 0x3a5d_5e3b_082f_2fb7;

    fn identity(width: u32, height: u32) -> Identity {
        Identity {
            digest: DIGEST,
            decimation: Decimation::none(grid(width, height)),
        }
    }

    fn country_table(feature_name: &str, boundary_digest: &str) -> CountryTable {
        let grid = grid(8, 4);
        let feature = FeatureId::new(feature_name).expect("fixture feature ID is valid");
        let geometry = Geometry::Polygon(Polygon {
            rings: vec![vec![
                LatLon {
                    lat: 45.0,
                    lon: -179.0,
                },
                LatLon {
                    lat: 45.0,
                    lon: -136.0,
                },
                LatLon {
                    lat: 89.0,
                    lon: -136.0,
                },
                LatLon {
                    lat: 89.0,
                    lon: -179.0,
                },
                LatLon {
                    lat: 45.0,
                    lon: -179.0,
                },
            ]],
        });
        let (ownership, diagnostics) =
            CellOwnership::rasterize_features(&[(feature.clone(), geometry)], grid)
                .expect("fixture country geometry is valid");
        assert_eq!(diagnostics, []);
        build_country(
            Synthetic::new(grid, -9999.0, vec![vec![1.0; 8]; 4])
                .expect("synthetic rows match the grid"),
            &ownership,
            &feature,
            boundary_digest,
        )
        .expect("fixture contains at least one owned cell")
    }

    fn country_ledger_in(directory: &TempDir, identity: &CountryTableIdentity) -> CountryLedger {
        CountryLedger::open_or_empty(directory.path().join("radii.json"), identity).unwrap()
    }

    fn country_candidate(identity: &CountryTableIdentity, population: f64) -> Candidate {
        let grid = identity.effective_grid();
        Candidate {
            row: grid.row(0).expect("the country grid has a row"),
            col: grid.col(0).expect("the country grid has a column"),
            population,
        }
    }

    fn candidate(identity: &Identity, row: u32, col: u32, population: f64) -> Candidate {
        let grid = identity.decimation.grid();
        Candidate {
            row: grid.row(row).expect("a row of the fixture"),
            col: grid.col(col).expect("a column of the fixture"),
            population,
        }
    }

    fn ledger_in(directory: &TempDir, identity: &Identity) -> Ledger {
        Ledger::open_or_empty(directory.path().join("radii.json"), identity).unwrap()
    }

    /// Three probes with distinct populations, so a row read back at the wrong offset shows up as a wrong
    /// maximum rather than as a coincidence.
    fn fill(ledger: &mut Ledger, identity: &Identity) {
        for (km, row, col, population) in [
            (0u32, 0u32, 0u32, 1.5f64),
            (1571, 2, 3, 300.25),
            (1572, 2, 3, 400.5),
        ] {
            ledger
                .put(km, candidate(identity, row, col, population))
                .unwrap();
        }
    }

    #[test]
    fn a_ledger_where_nothing_exists_is_empty_and_its_first_put_creates_the_file() {
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let mut ledger = ledger_in(&directory, &identity);

        // Absent rather than an error, because a first run has nothing to resume.
        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);
        assert!(!ledger.path().exists());

        ledger.put(500, candidate(&identity, 1, 2, 42.0)).unwrap();
        assert!(ledger.path().exists());
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn the_document_leads_with_its_format_version() {
        // On the text rather than on a parsed value, for `table/cache.rs`'s reason: what needs pinning is
        // that a reader streaming the document meets the version before anything it might not understand.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let mut ledger = ledger_in(&directory, &identity);
        ledger.put(500, candidate(&identity, 1, 2, 42.0)).unwrap();

        let document = fs::read_to_string(ledger.path()).unwrap();
        assert!(
            document.starts_with(r#"{"format_version":3,"#),
            "{document}"
        );
    }

    #[test]
    fn every_probe_survives_the_round_trip() {
        let directory = TempDir::new().unwrap();
        let identity = identity(8, 6);
        let mut written = ledger_in(&directory, &identity);
        fill(&mut written, &identity);

        let reopened = ledger_in(&directory, &identity);
        assert_eq!(reopened.len(), 3);
        for km in [0u32, 1571, 1572] {
            let before = written.get(km).expect("a radius that was put");
            let after = reopened.get(km).expect("a radius that was written");
            // Bit-identical, not close: these are bytes this host wrote and bytes this host read, so
            // anything but equality is a figure that moved.
            assert_eq!(after.population.to_bits(), before.population.to_bits());
            assert_eq!(after.row, before.row);
            assert_eq!(after.col, before.col);
        }
        // And a radius nobody probed is still absent after a reopen.
        assert_eq!(reopened.get(1570), None);
    }

    /// The document as text, so a test can plant exactly the disagreement it means.
    fn write_document(ledger: &Ledger, document: &Document) {
        fs::write(ledger.path(), serde_json::to_vec(document).unwrap()).unwrap();
    }

    fn document_of(identity: &Identity, radii: Vec<Probe>) -> Document {
        let mut document = Document::new(LedgerIdentity::Table(Attestation::new(identity)));
        document.radii = radii;
        document
    }

    fn table_attestation_mut(document: &mut Document) -> &mut Attestation {
        match &mut document.identity {
            LedgerIdentity::Table(attestation) => attestation,
            LedgerIdentity::Country(_) => panic!("test document has a world-table identity"),
        }
    }

    fn reopen(ledger: &Ledger, identity: &Identity) -> Result<Ledger, LedgerError> {
        Ledger::open_or_empty(ledger.path(), identity)
    }

    #[test]
    fn a_format_version_from_another_release_is_refused() {
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let mut document = document_of(&identity, Vec::new());
        document.format_version = FORMAT_VERSION + 1;
        // The digest goes too, and the refusal still names the version: a document of another version is
        // not one whose other fields this build knows how to compare.
        table_attestation_mut(&mut document).digest ^= 1;
        write_document(&ledger, &document);

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::FormatVersion { expected, found })
                if expected == FORMAT_VERSION && found == FORMAT_VERSION + 1
        ));
    }

    #[test]
    fn the_previous_world_document_shape_is_refused_after_the_identity_change() {
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);
        fs::write(
            ledger.path(),
            format!(
                r#"{{"format_version":2,"digest":{DIGEST},"width":4,"height":3,"decimation":1,"origin_lat":90.0,"origin_lon":-180.0,"lon_step":90.0,"lat_step":-60.0,"radii":[]}}"#
            ),
        )
        .unwrap();

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::FormatVersion {
                expected: 3,
                found: 2
            })
        ));
    }

    #[test]
    fn every_country_identity_dimension_refuses_a_resumed_ledger() {
        for (field, expected) in [
            ("feature_id", CountryMismatch::FeatureId),
            ("source_digest", CountryMismatch::SourceDigest),
            (
                "boundary_source_digest",
                CountryMismatch::BoundarySourceDigest,
            ),
            ("ownership_digest", CountryMismatch::OwnershipDigest),
            ("source_grid", CountryMismatch::SourceGrid),
            ("effective_grid", CountryMismatch::EffectiveGrid),
            ("decimation", CountryMismatch::Decimation),
            (
                "ownership_rule_version",
                CountryMismatch::OwnershipRuleVersion,
            ),
            (
                "aggregation_order_version",
                CountryMismatch::AggregationOrderVersion,
            ),
        ] {
            let table = country_table("country", "boundary-v1");
            let identity = table.identity();
            let directory = TempDir::new().unwrap();
            let mut ledger = country_ledger_in(&directory, identity);
            ledger.put(500, country_candidate(identity, 42.0)).unwrap();

            let mut document: serde_json::Value =
                serde_json::from_slice(&fs::read(ledger.path()).unwrap()).unwrap();
            let country = &mut document["identity"]["value"];
            match field {
                "feature_id" => country[field] = serde_json::json!("another-country"),
                "source_digest" | "ownership_digest" => {
                    let value = country[field].as_u64().unwrap();
                    country[field] = serde_json::json!(value.wrapping_add(1));
                }
                "boundary_source_digest" => {
                    country[field] = serde_json::json!("boundary-v2");
                }
                "source_grid" | "effective_grid" => {
                    let grid = &mut country[field];
                    let width = grid["width"].as_u64().unwrap();
                    grid["width"] = serde_json::json!(width + 1);
                }
                "decimation" | "ownership_rule_version" | "aggregation_order_version" => {
                    let value = country[field].as_u64().unwrap();
                    country[field] = serde_json::json!(value + 1);
                }
                _ => unreachable!("every identity field is listed above"),
            }
            fs::write(ledger.path(), serde_json::to_vec(&document).unwrap()).unwrap();

            assert!(
                matches!(
                    CountryLedger::open_or_empty(ledger.path(), identity),
                    Err(LedgerError::NotThisCountry(found)) if found == expected
                ),
                "changed field {field} was accepted"
            );
        }
    }

    #[test]
    fn a_world_ledger_cannot_be_opened_as_a_country_ledger() {
        let table = country_table("country", "boundary-v1");
        let directory = TempDir::new().unwrap();
        let world_identity = identity(8, 4);
        let mut world = ledger_in(&directory, &world_identity);
        world
            .put(500, candidate(&world_identity, 0, 0, 1.0))
            .unwrap();

        assert!(matches!(
            CountryLedger::open_or_empty(world.path(), table.identity()),
            Err(LedgerError::IdentityKind {
                wanted: "country",
                found: "world-table"
            })
        ));
    }

    #[test]
    fn a_digest_from_other_cells_is_refused() {
        // Issue #7's own requirement, and it reuses the table cache's mechanism rather than inventing a second
        // notion of which raster a file belongs to.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let mut document = document_of(&identity, Vec::new());
        table_attestation_mut(&mut document).digest ^= 1;
        write_document(&ledger, &document);

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::NotThisTable(Mismatch::Digest { wanted, found }))
                if wanted == DIGEST && found == DIGEST ^ 1
        ));
    }

    #[test]
    fn a_width_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let mut document = document_of(&identity, Vec::new());
        table_attestation_mut(&mut document).width += 1;
        write_document(&ledger, &document);

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::NotThisTable(Mismatch::Width {
                wanted: 4,
                found: 5
            }))
        ));
    }

    #[test]
    fn a_height_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let mut document = document_of(&identity, Vec::new());
        table_attestation_mut(&mut document).height += 1;
        write_document(&ledger, &document);

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::NotThisTable(Mismatch::Height {
                wanted: 3,
                found: 4
            }))
        ));
    }

    #[test]
    fn a_decimation_factor_that_is_not_the_tables_is_refused() {
        // The dimensions agree and the digest agrees — a decimated table carries the source's digest, by
        // ADR 0005 — so this is the only field that can tell the two tables apart.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let mut document = document_of(&identity, Vec::new());
        table_attestation_mut(&mut document).decimation = 2;
        write_document(&ledger, &document);

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::NotThisTable(Mismatch::DecimationFactor {
                wanted: 1,
                found: 2
            }))
        ));
    }

    /// The same shape whose columns start half a turn away: the dimensions and the factor agree, so
    /// nothing but the geometry can tell the two tables apart.
    fn half_turn_identity() -> Identity {
        let shifted = Grid::new(
            4,
            3,
            LatLon {
                lat: 90.0,
                lon: 0.0,
            },
            90.0,
            -60.0,
        )
        .expect("a 4 x 3 whole-globe grid starting at the prime meridian is valid");
        Identity {
            digest: DIGEST,
            decimation: Decimation::none(shifted),
        }
    }

    #[test]
    fn a_ledger_filled_over_another_grid_is_refused() {
        // The consequence ADR 0005 puts above a wrong sum: before this the dimensions matched, so every
        // probe's row and column minted cleanly onto the new grid, `CentreOffGrid` never fired, and the
        // resumed run published a centre whose population was measured half a turn away.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let mut ledger = ledger_in(&directory, &identity);
        fill(&mut ledger, &identity);

        assert!(matches!(
            reopen(&ledger, &half_turn_identity()),
            Err(LedgerError::NotThisTable(Mismatch::OriginLon { wanted, found }))
                if wanted == 0.0 && found == -180.0
        ));
    }

    #[test]
    fn a_v1_document_is_refused_for_its_version_and_not_for_its_syntax() {
        // A direct parse would report `Syntax`; the version probe should report `FormatVersion` first.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let v1 = format!(
            r#"{{"format_version":1,"digest":{DIGEST},"width":4,"height":3,"decimation":1,"radii":[]}}"#
        );
        fs::write(ledger.path(), v1).unwrap();

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::FormatVersion {
                expected: 3,
                found: 1
            })
        ));
    }

    #[test]
    fn the_version_is_read_out_of_a_document_carrying_more() {
        // The ledger's own probe, and its own test: the two probes are separate structs, so
        // `deny_unknown_fields` on this one would refuse every real ledger while the header's test still
        // passed.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let mut ledger = ledger_in(&directory, &identity);
        ledger.put(500, candidate(&identity, 1, 2, 42.0)).unwrap();

        let bytes = fs::read(ledger.path()).unwrap();
        let probed: DocumentVersion = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(probed.format_version, FORMAT_VERSION);
        assert!(bytes.len() > br#"{"format_version":3}"#.len());
    }

    #[test]
    fn a_refusal_names_the_ledger_and_not_the_header() {
        // The half of the shared enum a wrapper could quietly drop: the ground is the header's, and the
        // noun has to be this document's.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);
        write_document(&ledger, &document_of(&identity, Vec::new()));

        let message = reopen(&ledger, &half_turn_identity())
            .expect_err("a ledger over another grid is refused")
            .to_string();
        assert!(message.contains("ledger"), "{message}");
        assert!(!message.contains("header"), "{message}");
        assert!(message.contains("origin longitude is 0"), "{message}");
        assert!(message.contains("found -180"), "{message}");
    }

    #[test]
    fn a_document_that_is_not_json_is_refused() {
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);
        fs::write(ledger.path(), b"{\"format_version\": ").unwrap();

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::Syntax { .. })
        ));
    }

    #[test]
    fn a_centre_that_is_not_a_cell_of_the_grid_is_refused() {
        // The one thing a row can be wrong about that the header cannot catch: the dimensions agree, so a
        // row naming column 9 of a four-column table is a file this build must refuse rather than resume
        // from — and refuse at open, not at whichever query first looked.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let document = document_of(
            &identity,
            vec![Probe {
                km: 500,
                population: 12.0,
                row: 1,
                col: 9,
            }],
        );
        write_document(&ledger, &document);

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::CentreOffGrid {
                row: 1,
                col: 9,
                width: 4,
                height: 3
            })
        ));
    }

    #[test]
    fn the_same_radius_recorded_twice_is_refused() {
        // A radius has one maximum. Two rows for it are two answers to the same question, and a reader
        // that took the last would resume from whichever the writer happened to append second.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let ledger = ledger_in(&directory, &identity);

        let document = document_of(
            &identity,
            vec![
                Probe {
                    km: 500,
                    population: 12.0,
                    row: 1,
                    col: 2,
                },
                Probe {
                    km: 500,
                    population: 13.0,
                    row: 1,
                    col: 2,
                },
            ],
        );
        write_document(&ledger, &document);

        assert!(matches!(
            reopen(&ledger, &identity),
            Err(LedgerError::DuplicateRadius {
                km: 500,
                first,
                second
            }) if first == 12.0 && second == 13.0
        ));
    }

    #[test]
    fn an_interrupted_publication_leaves_the_document_before_it_and_refuses_the_next_put() {
        // The rename's whole point, from the other side: a temporary a crashed run left behind is never
        // read, and the document a reader finds is the last one a `put` finished. What the temporary being
        // exclusive adds is that the next `put` refuses rather than writing over it — two runs publishing
        // to one path would otherwise interleave into a document that parses and resumes from radii
        // neither of them measured.
        let directory = TempDir::new().unwrap();
        let identity = identity(8, 6);
        let mut ledger = ledger_in(&directory, &identity);
        fill(&mut ledger, &identity);
        let published = fs::read(ledger.path()).unwrap();

        let orphan = ledger.temporary.clone();
        fs::write(&orphan, b"a document a crashed run was part way through").unwrap();
        assert_eq!(fs::read(ledger.path()).unwrap(), published);
        assert_eq!(reopen(&ledger, &identity).unwrap().len(), 3);

        // The next put refuses, naming the file — and the ledger on disk is untouched, so nothing measured
        // is lost by the refusal: removing the orphan resumes.
        assert!(matches!(
            ledger.put(2000, candidate(&identity, 3, 4, 900.0)),
            Err(LedgerError::TemporaryHeld { ref path }) if *path == orphan
        ));
        assert_eq!(fs::read(ledger.path()).unwrap(), published);

        fs::remove_file(&orphan).unwrap();
        ledger.put(2000, candidate(&identity, 3, 4, 900.0)).unwrap();
        assert!(!orphan.exists());
        assert_eq!(reopen(&ledger, &identity).unwrap().len(), 4);
    }

    /// The registry raster's sentinel, so a fixture reaches the table by the path a real raster takes.
    const NODATA: f32 = -3.402_823e38;

    /// The whole-globe fixture the module's own tests use, with cells distinct so a maximum read back at
    /// the wrong radius shows up as a wrong figure rather than as a coincidence.
    fn payload_over(grid: &Grid) -> Vec<f64> {
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| f32::from(u16::try_from(row * grid.width() + col + 1).unwrap()))
                    .collect()
            })
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

    /// A real ledger that stops recording after `budget` probes: the first ones reach the file, and then it
    /// refuses. A process killed mid-run leaves exactly what this leaves.
    #[derive(Debug)]
    struct StopsAfter<'a> {
        inner: &'a mut Ledger,
        budget: usize,
    }

    impl RadiusLedger for StopsAfter<'_> {
        type Error = LedgerError;

        fn get(&self, km: u32) -> Option<Candidate> {
            self.inner.get(km)
        }

        fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
            if self.budget == 0 {
                return Err(LedgerError::Write {
                    path: self.inner.path().to_path_buf(),
                    source: io::Error::other("the run was interrupted"),
                });
            }
            self.budget -= 1;
            self.inner.put(km, found)
        }
    }

    #[test]
    fn an_interrupted_run_resumes_from_the_file_and_answers_what_an_uninterrupted_one_does() {
        // Issue #7's second box, end to end: through a real document in a real directory, with the entry
        // count read back off the disk rather than from the decorator's own tally.
        let fixture = grid(36, 18);
        let payload = payload_over(&fixture);
        let table = Table::new(fixture, &payload).expect("the build emits the padded product");
        let identity = Identity {
            digest: DIGEST,
            decimation: Decimation::none(fixture),
        };
        let share = Share::new(0.25).expect("a fixture share is a proportion");
        let spacing = NonZeroU32::new(4).expect("a fixture spacing is not zero");

        let directory = TempDir::new().unwrap();
        let path = directory.path().join("radii.json");

        // What the answer is when nothing is interrupted, for the resumed run to be held to.
        let uninterrupted = smallest(&table, share, Some(spacing), &mut (), &mut ())
            .expect("a whole-globe fixture and a no-op ledger cannot fail");

        // The run that dies on its fourth probe, having banked three.
        let mut interrupted = Ledger::open_or_empty(&path, &identity).unwrap();
        let mut stopping = StopsAfter {
            inner: &mut interrupted,
            budget: 3,
        };
        assert!(matches!(
            smallest(&table, share, Some(spacing), &mut stopping, &mut ()),
            Err(crate::smallest::SmallestError::Ledger(
                LedgerError::Write { .. }
            ))
        ));

        // The next process, which knows nothing but the path: three radii are on disk.
        let mut resumed_ledger = Ledger::open_or_empty(&path, &identity).unwrap();
        assert_eq!(resumed_ledger.len(), 3);

        let resumed = smallest(&table, share, Some(spacing), &mut resumed_ledger, &mut ())
            .expect("the ledger is this table's and the directory is writable");
        assert_eq!(resumed.stats.radii_reused, 3);
        assert_eq!(
            resumed.stats.radii_settled(),
            uninterrupted.stats.radii_settled()
        );
        assert_eq!(resumed.radius_km, uninterrupted.radius_km);
        assert_eq!(
            resumed.centre.population.to_bits(),
            uninterrupted.centre.population.to_bits()
        );
        assert_eq!(resumed.centre.row, uninterrupted.centre.row);
        assert_eq!(resumed.centre.col, uninterrupted.centre.col);
        assert_eq!(resumed.short_below, uninterrupted.short_below);

        // Every radius the finished run settled is on disk, so a third run would search nothing at all.
        assert_eq!(
            Ledger::open_or_empty(&path, &identity).unwrap().len() as u64,
            uninterrupted.stats.radii_settled()
        );
    }
}

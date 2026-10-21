// The raster seam: the vocabulary every consumer matches on, and no decoder. Which crate does the
// decoding is the sibling module's business and appears in no type here, so replacing it — ADR 0003
// names the conditions — is not a breaking change to anything downstream has matched on.

pub mod geotiff;

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::geodesy::LatLon;
use crate::grid::{Grid, Row};

/// What the caller declares a raster must be, and what a reader hands back on success: the grid a
/// consumer sees is this one, never one assembled from the file's own tags. A geotransform arrives as
/// a decimal that need not round-trip to the rational it means — the registry raster's step is
/// `1/120 + 5.4e-16` — and every later stage wants the rational.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterSpec {
    pub grid: Grid,
    pub epsg: u16,
    pub pixel: PixelType,
    /// The sentinel the file must declare. Compared bit for bit rather than by value: the registry
    /// raster's sits two ulps off `-f32::MAX`, so a reader reaching for that constant matches nothing,
    /// counts no nodata, and reports a world population it has no reason to doubt.
    pub nodata: f32,
}

/// The one storage layout this reader accepts. An enum rather than a constant so that a second layout
/// arrives as a variant every `match` has to answer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelType {
    Float32,
}

impl fmt::Display for PixelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Float32 => f.write_str("Float32 (IEEE floating point, 32 bits)"),
        }
    }
}

/// Where every cell of a drained raster went. The four classes are disjoint and exhaustive, so they
/// sum to the grid's cell count, and that sum is the claim a caller can check its own traversal
/// against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellTallies {
    /// Cells matching the declared sentinel bit for bit.
    pub nodata: u64,
    /// Negative cells that did not match it. Zero population all the same, counted apart because
    /// nothing about the dataset says they should exist — the count is what turns that from an
    /// assumption into an observation.
    pub unexpected_negative: u64,
    pub zero: u64,
    pub populated: u64,
}

impl CellTallies {
    #[must_use]
    pub const fn total(self) -> u64 {
        self.nodata + self.unexpected_negative + self.zero + self.populated
    }
}

/// Every way a raster can fail to be the one that was declared.
///
/// A message names both sides — what was declared, or what this reader requires, and what the file
/// actually says. A rejection reading only "grid mismatch" sends its reader back to the file with a
/// hex editor, which is the failure this enum's shape is against.
#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error(
        "declared a {} x {} raster; the file is {} x {}",
        declared.0, declared.1, found.0, found.1
    )]
    Dimensions {
        declared: (u32, u32),
        found: (u32, u32),
    },

    #[error(
        "declared origin (lat {}, lon {}); the file's tiepoint is (lat {}, lon {}), outside the \
         tolerance a rounded geotransform decimal is allowed",
        declared.lat, declared.lon, found.lat, found.lon
    )]
    Origin { declared: LatLon, found: LatLon },

    #[error(
        "declared step (lon {}, lat {}); the file's pixel scale is (lon {}, lat {}), outside the \
         tolerance a rounded geotransform decimal is allowed",
        declared.0, declared.1, found.0, found.1
    )]
    Step {
        declared: (f64, f64),
        found: (f64, f64),
    },

    #[error(
        "this reader reads strips; the file is tiled (tags TileWidth 322 and TileLength 323 present)"
    )]
    Tiled,

    #[error("declared {declared} sample per pixel; the file says {found}")]
    SamplesPerPixel { declared: u16, found: u16 },

    #[error(
        "declared {declared}; the file says SampleFormat {found_sample_format} at {found_bits} bits \
         per sample"
    )]
    PixelFormat {
        declared: PixelType,
        found_sample_format: u16,
        found_bits: u16,
    },

    #[error(
        "this reader requires GTRasterType 1 (RasterPixelIsArea), which is what puts a tiepoint on \
         the outer corner of the first cell; the file says {found}"
    )]
    RasterType { found: u16 },

    #[error("this reader requires GTModelType 2 (geographic); the file says {found}")]
    ModelType { found: u16 },

    #[error("declared EPSG {declared}; the file's GeographicType key says {found}")]
    Epsg { declared: u16, found: u16 },

    #[error(
        "this reader reads a pixel scale and a tiepoint; the file carries ModelTransformation (tag \
         34264), which can express a rotation neither of those can"
    )]
    ModelTransformation,

    #[error(
        "declared nodata {declared:e} (0x{:08x}); the file declares {found:e} (0x{:08x})",
        declared.to_bits(), found.to_bits()
    )]
    NodataMismatch { declared: f32, found: f32 },

    #[error("declared nodata {declared:e}; the file's GDAL_NODATA tag reads {found:?}")]
    NodataNotANumber { declared: f32, found: String },

    #[error("declared nodata {declared:e}; the file carries no GDAL_NODATA tag (42113)")]
    NodataMissing { declared: f32 },

    #[error("the file carries no {name} tag ({tag}), so this reader cannot read its geotransform")]
    MissingTag { name: &'static str, tag: u16 },

    #[error("the file's GeoKeyDirectory carries no {name} key ({key})")]
    MissingGeoKey { name: &'static str, key: u16 },

    // Separate from `Io` although a `NotFound` is what produces it: an absent raster is the one I/O
    // failure with a next step, and naming the path is what makes the message actionable when the
    // caller passed a path it did not spell.
    #[error("no raster at {path}; fetch it with `mise run data:get`")]
    Absent { path: PathBuf },

    // Apart from Decode, and deliberately: a file that is not there and a file whose bytes are wrong
    // are different findings, and #8's exit-code classes will want to separate them. Folding both
    // into Decode would make "the decoder said no" mean "something went wrong".
    #[error("the raster could not be read")]
    Io(#[source] std::io::Error),

    // The source is boxed and opaque, so no consumer matches on the decoder's own error type and
    // ADR 0003's fallback stays the two lines of Cargo.toml it claims to be.
    #[error("the raster could not be decoded")]
    Decode(#[source] Box<dyn Error + Send + Sync>),
}

/// One row of a raster, borrowed from whatever buffer the source decoded it into.
///
/// The index is a [`Row`] rather than an integer because the accessors on [`Grid`] take nothing else:
/// handing out a `u32` would mean every consumer re-mints it, or does not, and carries a raw index the
/// grid has already stopped accepting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterRow<'a> {
    pub row: Row,
    pub values: &'a [f32],
}

/// A raster read one row at a time, in row order, with nodata already turned into zero.
///
/// The borrow in [`RasterSource::next_row`] ends when the row is dropped, so a caller cannot hold two
/// input rows at once. That costs a prefix-sum pass nothing — it carries the previous *output* row in
/// its own accumulator, not the previous input row — and it is what lets a source hand out a slice of
/// the strip it just decoded instead of a copy.
pub trait RasterSource {
    /// The **declared** grid, never one assembled from a file's own tags.
    fn grid(&self) -> Grid;

    /// The next row, or `None` when the raster is drained. The grid is the mint and the bound at once:
    /// this returns `None` exactly when `grid().row(i)` does, so the stream needs no length of its own.
    fn next_row(&mut self) -> Option<Result<RasterRow<'_>, RasterError>>;

    /// The tallies, once the stream is exhausted.
    ///
    /// It consumes the source rather than reading through a `&self` getter, because a count taken
    /// halfway through a read is not wrong for any reason a caller could state. Consuming makes the
    /// half-formed read unrepresentable instead of documented. `where Self: Sized` keeps the streaming
    /// half of this trait usable through `dyn RasterSource`, which is what a consumer holding several
    /// kinds of source wants.
    fn finish(self) -> CellTallies
    where
        Self: Sized;
}

/// The one place a sentinel becomes a zero, and the only thing in this crate that knows a nodata value
/// from a population count. `application.md` "Nodata" is why it is one place: a second copy is how a
/// later stage ends up having to tell them apart again.
///
/// The sentinel is matched on bits, not with `==`. A `GDAL_NODATA` of `nan` is a legal tag that `==`
/// would never match, and the registry raster's sentinel sits two ulps from `-f32::MAX`, so a
/// comparison that is nearly right is a world population of zero.
pub fn sanitise_row(values: &mut [f32], nodata: f32, tallies: &mut CellTallies) {
    let sentinel = nodata.to_bits();
    for value in values {
        if value.to_bits() == sentinel {
            tallies.nodata += 1;
            *value = 0.0;
        } else if *value < 0.0 {
            tallies.unexpected_negative += 1;
            *value = 0.0;
        } else if *value > 0.0 {
            tallies.populated += 1;
        } else {
            // Zero, and everything else that is not a count: a NaN which is not the sentinel compares
            // false both ways and lands here. Writing zero rather than leaving it is what keeps "no
            // later stage has to know a sentinel from a count" true of NaN too.
            tallies.zero += 1;
            *value = 0.0;
        }
    }
}

/// A raster held in memory, for a caller that wants a raster without a file — every consumer of this
/// trait is tested against one.
///
/// Public and unconditional rather than `#[cfg(test)]`: the tests that want it live in `tests/` and in
/// other crates, where a test-only item is invisible.
#[derive(Debug, Clone)]
pub struct Synthetic {
    grid: Grid,
    nodata: f32,
    remaining: VecDeque<Vec<f32>>,
    next: u32,
    current: Vec<f32>,
    tallies: CellTallies,
}

impl Synthetic {
    /// `rows` are raw values, sentinels included, one inner vector per grid row: a fixture states what
    /// the file would hold and gets the same conversion a decoded strip gets.
    ///
    /// # Errors
    /// [`RasterError::Dimensions`] when the rows are not the shape the grid declares.
    pub fn new(grid: Grid, nodata: f32, rows: Vec<Vec<f32>>) -> Result<Self, RasterError> {
        let widths: Vec<usize> = rows.iter().map(Vec::len).collect();
        let found_width = widths.first().copied().unwrap_or(0);
        let ragged = widths.iter().any(|width| *width != found_width);
        let declared = (grid.width(), grid.height());

        // usize -> u32 saturates rather than wrapping, so an oversized fixture is reported as too
        // large instead of as some smaller shape that happens to fit.
        let found = (
            u32::try_from(found_width).unwrap_or(u32::MAX),
            u32::try_from(rows.len()).unwrap_or(u32::MAX),
        );
        if ragged || found != declared {
            return Err(RasterError::Dimensions { declared, found });
        }

        Ok(Self {
            grid,
            nodata,
            remaining: rows.into(),
            next: 0,
            current: Vec::with_capacity(found_width),
            tallies: CellTallies::default(),
        })
    }
}

impl RasterSource for Synthetic {
    fn grid(&self) -> Grid {
        self.grid
    }

    fn next_row(&mut self) -> Option<Result<RasterRow<'_>, RasterError>> {
        let row = self.grid.row(self.next)?;
        let raw = self.remaining.pop_front()?;
        self.next += 1;

        self.current = raw;
        sanitise_row(&mut self.current, self.nodata, &mut self.tallies);
        Some(Ok(RasterRow {
            row,
            values: &self.current,
        }))
    }

    fn finish(self) -> CellTallies {
        self.tallies
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp is here because the sanitiser's
// contract is exact: a cell is zero or it is the value the file held, never within a tolerance of one.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn spec_grid() -> Grid {
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0 / 120.0,
            -1.0 / 120.0,
        )
        .expect("the registry row is a valid grid")
    }

    // The sentinel the registry raster declares, which is not -f32::MAX: two ulps short of it, and
    // the bit pattern is what the assertions below pin.
    const GPW_NODATA: f32 = -3.402_823e38;

    #[test]
    fn the_spec_carries_the_declared_grid_verbatim() {
        let spec = RasterSpec {
            grid: spec_grid(),
            epsg: 4326,
            pixel: PixelType::Float32,
            nodata: GPW_NODATA,
        };
        assert_eq!(spec.grid, spec_grid());
        assert_eq!(spec.nodata.to_bits(), 0xff7f_fffd);
    }

    #[test]
    fn the_tallies_sum_to_the_cells_they_classify() {
        let tallies = CellTallies {
            nodata: 710_450_072,
            unexpected_negative: 0,
            zero: 40_311_312,
            populated: 182_358_616,
        };
        assert_eq!(tallies.total(), 933_120_000);
        assert_eq!(CellTallies::default().total(), 0);
    }

    type Rejection = (RasterError, Vec<&'static str>);

    // The three that compare a number the file carries against one the caller declared.
    fn geometry_rejections() -> Vec<Rejection> {
        vec![
            (
                RasterError::Dimensions {
                    declared: (43200, 21600),
                    found: (43200, 21601),
                },
                vec!["43200 x 21600", "43200 x 21601"],
            ),
            (
                RasterError::Origin {
                    declared: LatLon {
                        lat: 90.0,
                        lon: -180.0,
                    },
                    found: LatLon {
                        lat: 89.5,
                        lon: -180.0,
                    },
                },
                vec!["lat 90", "lat 89.5"],
            ),
            (
                RasterError::Step {
                    declared: (1.0 / 120.0, -1.0 / 120.0),
                    found: (-1.0 / 120.0, -1.0 / 120.0),
                },
                vec!["lon 0.008333333333333333", "lon -0.008333333333333333"],
            ),
        ]
    }

    // What the layout tags and the GeoKeys say.
    fn layout_rejections() -> Vec<Rejection> {
        vec![
            (RasterError::Tiled, vec!["strips", "tiled", "322"]),
            (
                RasterError::SamplesPerPixel {
                    declared: 1,
                    found: 3,
                },
                vec!["declared 1", "says 3"],
            ),
            (
                RasterError::PixelFormat {
                    declared: PixelType::Float32,
                    found_sample_format: 1,
                    found_bits: 32,
                },
                vec!["Float32", "SampleFormat 1", "32 bits"],
            ),
            (
                RasterError::RasterType { found: 2 },
                vec!["GTRasterType 1", "says 2"],
            ),
            (
                RasterError::ModelType { found: 1 },
                vec!["GTModelType 2", "says 1"],
            ),
            (
                RasterError::Epsg {
                    declared: 4326,
                    found: 3857,
                },
                vec!["EPSG 4326", "says 3857"],
            ),
            (
                RasterError::ModelTransformation,
                vec!["pixel scale", "34264"],
            ),
        ]
    }

    // The sentinel, the absences, and the three failures that are not a disagreement about the file's
    // contents at all.
    fn nodata_and_absence_rejections() -> Vec<Rejection> {
        vec![
            (
                RasterError::NodataMismatch {
                    declared: GPW_NODATA,
                    found: -9999.0,
                },
                vec!["0xff7ffffd", "-9.999e3", "0xc61c3c00"],
            ),
            (
                RasterError::NodataNotANumber {
                    declared: GPW_NODATA,
                    found: "not-a-number".to_owned(),
                },
                vec!["-3.402823e38", "not-a-number"],
            ),
            (
                RasterError::NodataMissing {
                    declared: GPW_NODATA,
                },
                vec!["-3.402823e38", "42113"],
            ),
            (
                RasterError::MissingTag {
                    name: "ModelPixelScale",
                    tag: 33550,
                },
                vec!["ModelPixelScale", "33550"],
            ),
            (
                RasterError::MissingGeoKey {
                    name: "GeographicType",
                    key: 2048,
                },
                vec!["GeographicType", "2048"],
            ),
            (
                RasterError::Absent {
                    path: PathBuf::from("data/population/absent.tif"),
                },
                vec!["data/population/absent.tif", "mise run data:get"],
            ),
            (
                RasterError::Io(std::io::Error::from(std::io::ErrorKind::NotFound)),
                vec!["read"],
            ),
            (
                RasterError::Decode(Box::new(std::io::Error::from(
                    std::io::ErrorKind::UnexpectedEof,
                ))),
                vec!["decoded"],
            ),
        ]
    }

    // The substrings each message owes a reader: what was declared or required, and what the file
    // said. A table rather than a test each is what makes the two-sided property a property of the
    // enum instead of a habit of whoever wrote the last variant.
    #[test]
    fn every_rejection_names_both_sides() {
        let cases = geometry_rejections()
            .into_iter()
            .chain(layout_rejections())
            .chain(nodata_and_absence_rejections());

        for (error, wanted) in cases {
            let message = error.to_string();
            for fragment in wanted {
                assert!(
                    message.contains(fragment),
                    "{error:?} says {message:?}, which does not name {fragment:?}"
                );
            }
        }
    }

    // Four cells, one of each class: the declared sentinel, a negative nothing explains, a zero and a
    // count.
    fn one_of_each() -> Synthetic {
        let grid = Grid::new(
            4,
            1,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            90.0,
            -90.0,
        )
        .expect("a four-cell band is a valid grid");
        Synthetic::new(grid, GPW_NODATA, vec![vec![GPW_NODATA, -1.0, 0.0, 12.5]])
            .expect("the fixture is the shape the grid declares")
    }

    #[test]
    fn a_sentinel_and_an_unexplained_negative_both_come_out_as_zero() {
        let mut raster = one_of_each();
        let row = raster.next_row().expect("the first row").expect("no error");
        assert_eq!(row.values, [0.0, 0.0, 0.0, 12.5]);
        assert_eq!(row.row, raster.grid().row(0).unwrap());

        assert!(raster.next_row().is_none());
        // The counts are only a fact about the raster once the last row is read, which is why they are
        // reachable through `finish` and nowhere else.
        let tallies = raster.finish();
        assert_eq!(
            tallies,
            CellTallies {
                nodata: 1,
                unexpected_negative: 1,
                zero: 1,
                populated: 1,
            }
        );
        assert_eq!(tallies.total(), 4);
    }

    #[test]
    fn a_drained_raster_hands_out_its_grids_rows_in_order_and_then_nothing() {
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
        .expect("a one-degree whole-globe grid is valid");
        let rows = vec![vec![1.0; 360]; 180];
        let mut raster =
            Synthetic::new(grid, GPW_NODATA, rows).expect("the fixture matches the grid");

        let mut seen = Vec::new();
        while let Some(row) = raster.next_row() {
            seen.push(row.expect("no error").row);
        }
        assert_eq!(seen, grid.rows().collect::<Vec<_>>());
        // Stays drained: a caller polling a second time gets None rather than the first row again.
        assert!(raster.next_row().is_none());
        assert!(raster.next_row().is_none());
        assert_eq!(raster.finish().populated, 360 * 180);
    }

    #[test]
    fn the_streaming_half_works_through_a_trait_object() {
        // What `where Self: Sized` on finish buys: a consumer holding several kinds of source reads
        // rows through one pointer, and pays only by not being able to end the stream through it.
        let mut raster = one_of_each();
        let source: &mut dyn RasterSource = &mut raster;
        assert_eq!(source.grid().width(), 4);
        let row = source.next_row().expect("the first row").expect("no error");
        assert_eq!(row.values, [0.0, 0.0, 0.0, 12.5]);
    }

    #[test]
    fn a_fixture_that_is_not_the_grids_shape_is_rejected() {
        let grid = one_of_each().grid();
        for rows in [
            vec![vec![1.0, 2.0, 3.0]],
            vec![vec![1.0; 4], vec![1.0; 4]],
            vec![vec![1.0, 2.0, 3.0, 4.0], vec![1.0, 2.0]],
            vec![],
        ] {
            assert!(matches!(
                Synthetic::new(grid, GPW_NODATA, rows),
                Err(RasterError::Dimensions { .. })
            ));
        }
    }

    // A cell is a sentinel, a negative, a zero or a count, and the strategy has to produce all four —
    // a generator of arbitrary f32 would hit the sentinel's exact bits about never.
    fn a_cell() -> impl Strategy<Value = f32> {
        prop_oneof![
            1 => Just(GPW_NODATA),
            1 => -1e6f32..0.0,
            1 => Just(0.0f32),
            3 => 0.0f32..1e6,
        ]
    }

    proptest! {
        #[test]
        fn no_cell_leaves_the_seam_negative_and_the_counts_survive(
            width in 1u32..8,
            height in 1u32..8,
            seed in prop::collection::vec(a_cell(), 1..64),
        ) {
            let grid = Grid::new(
                width,
                height,
                LatLon { lat: 90.0, lon: -180.0 },
                360.0 / f64::from(width),
                -180.0 / f64::from(height),
            )?;
            let cells: Vec<f32> = (0..width * height)
                .map(|index| seed[index as usize % seed.len()])
                .collect();
            let rows: Vec<Vec<f32>> = cells
                .chunks(width as usize)
                .map(<[f32]>::to_vec)
                .collect();

            let mut raster = Synthetic::new(grid, GPW_NODATA, rows)?;
            let mut out = Vec::new();
            while let Some(row) = raster.next_row() {
                out.extend_from_slice(row?.values);
            }

            let expected: f32 = cells
                .iter()
                .filter(|value| **value > 0.0)
                .sum();
            prop_assert!(out.iter().all(|value| *value >= 0.0));
            prop_assert_eq!(out.iter().sum::<f32>(), expected);
            let tallies = raster.finish();
            prop_assert_eq!(tallies.total(), u64::from(width) * u64::from(height));
        }
    }

    #[test]
    fn the_error_crosses_a_thread_and_a_boxed_source_survives() {
        // Send + Sync is what lets a parallel search return one, and the source chain is what makes
        // the opaque Decode variant usable rather than merely opaque.
        fn assert_error<E: Error + Send + Sync + 'static>(error: E) -> String {
            error.source().map_or_else(String::new, ToString::to_string)
        }
        let inner = std::io::Error::from(std::io::ErrorKind::UnexpectedEof);
        let wanted = inner.to_string();
        assert_eq!(assert_error(RasterError::Decode(Box::new(inner))), wanted);
        assert_eq!(assert_error(RasterError::Tiled), "");
    }
}

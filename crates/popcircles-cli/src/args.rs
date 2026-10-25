// Every flag group the parser flattens, and the value parsers those flags name. They sit together
// because that is what a command declaration reaches for: `Command`'s variants flatten these structs
// and cite these parsers, and nothing else in the crate constructs either.
//
// A conversion here calls a library constructor and computes nothing — `GridValues::grid` hands six
// numbers to `Grid::new` and `WindowArgs::window` hands four to `Window`. Arithmetic on a coordinate
// belongs below this crate, so a method here that grew any would be a defect rather than a helper.

use std::num::NonZeroU32;
use std::path::PathBuf;

use clap::Args;
use log::LevelFilter;
use popcircles::geodesy::{LatLon, RadiusKm};
use popcircles::grid::{Grid, GridError};
use popcircles::smallest::Share;
use popcircles::table::Window;

// `//` rather than `///`, deliberately: clap publishes a flattened struct's description
// when the command declares none, so this reasoning was the first thing `--help` showed. The words are a
// maintainer's and the level's own help text is the field's below.
//
// How much a run says about what it is doing, and the only control over it. There
// is no boolean pair beside it: two flags standing in for a threshold is the shape `FU-04` names, and its
// condition is a sweep over this directory, so spelling those two flags out here would fire it.
//
// `global` sits on the argument rather than on the `#[command(flatten)]` above, because the attribute is
// the argument's: that is what lets every subcommand take the flag after its own name without declaring
// it.
#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct LogArgs {
    /// How much to report on stderr: `error`, `warn`, `info` or `debug`. It does not govern the progress
    /// meter, which answers how far a run has got rather than what happened.
    #[arg(long, global = true, default_value = "info", value_name = "LEVEL", value_parser = parse_log_level)]
    pub(crate) log_level: LevelFilter,
}

/// The four names box 5 of issue #8 gives, and no others.
///
/// Its own parser rather than `LevelFilter`'s `FromStr`, which also accepts `trace` and `off`: a level is
/// accepted here as a threshold a reader asks for, and `trace` adds none that `debug` does not already
/// give — the granularity below `debug`'s is the candidate block and the kernel, which are half a million
/// and sixteen thousand in a measured run and logged at no level.
pub(crate) fn parse_log_level(value: &str) -> Result<LevelFilter, String> {
    match value {
        "error" => Ok(LevelFilter::Error),
        "warn" => Ok(LevelFilter::Warn),
        "info" => Ok(LevelFilter::Info),
        "debug" => Ok(LevelFilter::Debug),
        other => Err(format!(
            "`{other}` is not a level; the levels are error, warn, info and debug"
        )),
    }
}

/// The range of shares a sweep walks, in whole percent.
///
/// Integers rather than fractions, and the walk is over them: a step of a tenth accumulated in f64 reaches
/// `0.30000000000000004` by its third share and publishes it. Dividing each integer by a hundred instead
/// gives the f64 a caller typing the fraction would have got, with no accumulation anywhere.
#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct SweepArgs {
    /// The first share to answer, in whole percent.
    #[arg(long)]
    pub(crate) from: u32,
    /// The last share to answer, in whole percent. A share the step would carry past it is not answered.
    #[arg(long)]
    pub(crate) to: u32,
    /// How much to raise the share by between records, in whole percent.
    #[arg(long)]
    pub(crate) step: u32,
}

/// Where the radii a run settles are kept, so an interrupted run resumes instead of paying for them
/// twice.
///
/// On by default and with no way to turn it off: a ledger describing another table is refused rather than
/// resumed from, so there is nothing an opt-out would protect against.
#[derive(Args, Debug, Clone)]
pub(crate) struct LedgerArgs {
    /// The JSON document every probe's maximum is recorded in. Under `out/` beside the cache, which is
    /// gitignored.
    #[arg(long, default_value = "out/radii.json")]
    pub(crate) ledger: PathBuf,
}

#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct SearchArgs {
    /// Override the initial block side in cells, derived from each radius and the grid when omitted.
    /// Changes runtime only; the answer is the same at every spacing.
    #[arg(long)]
    pub(crate) spacing: Option<NonZeroU32>,
}

/// A table that has already been built, named the way opening one needs: the grid it was declared over,
/// where the cache sits, and an optional digest expectation.
///
/// Flattened into every command that reads a cached table, so those flags have one spelling and one help
/// string rather than a copy per command.
#[derive(Args, Debug, Clone)]
pub(crate) struct CachedTableArgs {
    #[command(flatten)]
    pub(crate) grid: DatasetGridArgs,
    #[command(flatten)]
    pub(crate) table: TableArgs,
    /// Require this decoded-cells digest; otherwise use the cache header's digest.
    #[arg(long, value_parser = parse_digest)]
    pub(crate) digest: Option<u64>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CountryTableArgs {
    /// Configured population and boundary sources to query.
    #[arg(long)]
    pub(crate) combination: Option<String>,
    /// Prepared resolution; defaults to the combination's configured default.
    #[arg(long)]
    pub(crate) resolution: Option<u32>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct DatasetGridArgs {
    /// Population raster in data/registry.toml supplying defaults for omitted grid flags.
    #[arg(long)]
    pub(crate) dataset: Option<String>,
    #[command(flatten)]
    pub(crate) grid: GridArgs,
}

impl DatasetGridArgs {
    pub(crate) fn grid(&self) -> Result<Grid, crate::failure::Failure> {
        let defaults = self
            .dataset
            .as_deref()
            .map(|name| {
                crate::registry::Registry::load(std::path::Path::new(
                    crate::registry::REGISTRY_PATH,
                ))
                .and_then(|registry| registry.raster(name))
                .map(|row| row.grid)
                .map_err(|error| crate::failure::Failure::registry(&error))
            })
            .transpose()?;
        self.grid
            .resolve(defaults)?
            .grid()
            .map_err(|error| crate::failure::Failure::grid(&error))
    }
}

#[derive(Args, Debug, Clone, Copy, Default)]
pub(crate) struct GridArgs {
    #[arg(long)]
    pub(crate) width: Option<u32>,
    #[arg(long)]
    pub(crate) height: Option<u32>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) origin_lat: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) origin_lon: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) lon_step: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) lat_step: Option<f64>,
}

impl GridArgs {
    pub(crate) fn resolve(
        self,
        defaults: Option<GridValues>,
    ) -> Result<GridValues, crate::failure::Failure> {
        let fields = [
            ("--width", self.width.is_some()),
            ("--height", self.height.is_some()),
            ("--origin-lat", self.origin_lat.is_some()),
            ("--origin-lon", self.origin_lon.is_some()),
            ("--lon-step", self.lon_step.is_some()),
            ("--lat-step", self.lat_step.is_some()),
        ];
        let missing: Vec<_> = fields
            .into_iter()
            .filter_map(|(name, present)| (!present).then_some(name))
            .collect();
        if defaults.is_none() && !missing.is_empty() {
            return Err(crate::failure::Failure::bad_input(format!(
                "without --dataset the missing grid flags are required: {}",
                missing.join(", ")
            )));
        }
        // All fields exist above; zero is only a fallback for construction after that check.
        let defaults = defaults.unwrap_or_default();
        Ok(GridValues {
            width: self.width.unwrap_or(defaults.width),
            height: self.height.unwrap_or(defaults.height),
            origin_lat: self.origin_lat.unwrap_or(defaults.origin_lat),
            origin_lon: self.origin_lon.unwrap_or(defaults.origin_lon),
            lon_step: self.lon_step.unwrap_or(defaults.lon_step),
            lat_step: self.lat_step.unwrap_or(defaults.lat_step),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct GridValues {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) origin_lat: f64,
    pub(crate) origin_lon: f64,
    pub(crate) lon_step: f64,
    pub(crate) lat_step: f64,
}

impl GridValues {
    pub(crate) fn grid(self) -> Result<Grid, GridError> {
        Grid::new(
            self.width,
            self.height,
            LatLon {
                lat: self.origin_lat,
                lon: self.origin_lon,
            },
            self.lon_step,
            self.lat_step,
        )
    }
}

#[derive(Args, Debug, Clone)]
pub(crate) struct TableArgs {
    /// Both cache files are this path plus a suffix. Under `out/` by default, which is gitignored —
    /// a generated table is never committed.
    #[arg(long, default_value = "out/table")]
    pub(crate) cache: PathBuf,
    /// Fold every k by k block of cells into one table cell. Must divide both grid dimensions.
    #[arg(long, default_value_t = 1)]
    pub(crate) decimate: u32,
}

/// The rectangle a query covers. All four or none: without them the query is the table's whole extent,
/// which is not a window a pair of coordinates can express — a full turn and one column reduce alike.
#[derive(Args, Debug, Clone, Copy)]
#[group(multiple = true, requires_all = ["north", "south", "west", "east"])]
pub(crate) struct WindowArgs {
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) north: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) south: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) west: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) east: Option<f64>,
}

impl WindowArgs {
    pub(crate) fn window(self) -> Option<Window> {
        Some(Window {
            north: self.north?,
            south: self.south?,
            west: self.west?,
            east: self.east?,
        })
    }
}

/// Accepts what a build printed, `0x` and all, so a digest expectation needs no transcription.
pub(crate) fn parse_digest(value: &str) -> Result<u64, String> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    u64::from_str_radix(digits, 16)
        .map_err(|error| format!("`{value}` is not a 64-bit hexadecimal digest: {error}"))
}

/// A radius through [`RadiusKm::new`], so a negative or non-finite one is a usage error the parser
/// reports.
///
/// Exit 2 is what clap gives a usage error, which is already `EXIT_BAD_INPUT`, so `RadiusError` needs no
/// arm of its own in the classifiers below.
pub(crate) fn parse_radius(value: &str) -> Result<RadiusKm, String> {
    let km: f64 = value
        .parse()
        .map_err(|error| format!("`{value}` is not a number of kilometres: {error}"))?;
    RadiusKm::new(km).map_err(|error| error.to_string())
}

/// A share through [`Share::from_percent`], so the conversion from percent to fraction is the domain's and
/// this crate divides nothing.
pub(crate) fn parse_share(value: &str) -> Result<Share, String> {
    let percent: u32 = value
        .parse()
        .map_err(|error| format!("`{value}` is not a whole percent: {error}"))?;
    Share::from_percent(percent).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grid_override_wins_independently_including_zero() {
        let defaults = GridValues {
            width: 36,
            height: 18,
            origin_lat: 90.0,
            origin_lon: -180.0,
            lon_step: 10.0,
            lat_step: -10.0,
        };
        for (flags, expected) in [
            (
                GridArgs {
                    width: Some(0),
                    ..GridArgs::default()
                },
                GridValues {
                    width: 0,
                    ..defaults
                },
            ),
            (
                GridArgs {
                    height: Some(0),
                    ..GridArgs::default()
                },
                GridValues {
                    height: 0,
                    ..defaults
                },
            ),
            (
                GridArgs {
                    origin_lat: Some(0.0),
                    ..GridArgs::default()
                },
                GridValues {
                    origin_lat: 0.0,
                    ..defaults
                },
            ),
            (
                GridArgs {
                    origin_lon: Some(0.0),
                    ..GridArgs::default()
                },
                GridValues {
                    origin_lon: 0.0,
                    ..defaults
                },
            ),
            (
                GridArgs {
                    lon_step: Some(0.0),
                    ..GridArgs::default()
                },
                GridValues {
                    lon_step: 0.0,
                    ..defaults
                },
            ),
            (
                GridArgs {
                    lat_step: Some(0.0),
                    ..GridArgs::default()
                },
                GridValues {
                    lat_step: 0.0,
                    ..defaults
                },
            ),
            (GridArgs::default(), defaults),
        ] {
            assert_eq!(
                flags.resolve(Some(defaults)).map_err(|error| error.message),
                Ok(expected)
            );
        }
    }

    #[test]
    fn a_digest_is_read_back_in_the_spelling_a_build_prints() {
        // `report.rs` prints `{:#018x}`, so this is the round trip between the two documents.
        assert_eq!(
            parse_digest("0x3a5d5e3b082f2fb7"),
            Ok(0x3a5d_5e3b_082f_2fb7)
        );
        assert_eq!(parse_digest("3a5d5e3b082f2fb7"), Ok(0x3a5d_5e3b_082f_2fb7));
        assert!(parse_digest("0x not a digest").is_err());
        // The prefix alone is not a digest either, and neither is one 17 digits long.
        assert!(parse_digest("0x").is_err());
        assert!(parse_digest("0x1f17aa802a6890f0c").is_err());
    }

    #[test]
    fn a_share_outside_a_proportion_is_refused_by_the_parser() {
        // `Share::from_percent` holds the two grounds, so this crate performs no division and no range
        // check of its own — and the message a caller sees is the domain's.
        assert!(parse_share("0").is_err());
        assert!(parse_share("101").is_err());
        assert!(parse_share("-1").is_err());
        assert!(parse_share("12.5").is_err());
    }

    #[test]
    fn a_share_in_whole_percent_is_the_fraction_a_document_publishes() {
        // Exactly, which is the whole reason the flag is percent rather than a fraction: no accumulated
        // residue reaches a published share.
        assert_eq!(parse_share("50").map(Share::get), Ok(0.5));
        assert_eq!(parse_share("100").map(Share::get), Ok(1.0));
        assert_eq!(parse_share("10").map(Share::get), Ok(0.1));
    }

    #[test]
    fn the_four_levels_parse_and_a_fifth_does_not() {
        assert_eq!(parse_log_level("error"), Ok(LevelFilter::Error));
        assert_eq!(parse_log_level("warn"), Ok(LevelFilter::Warn));
        assert_eq!(parse_log_level("info"), Ok(LevelFilter::Info));
        assert_eq!(parse_log_level("debug"), Ok(LevelFilter::Debug));

        // `LevelFilter`'s own `FromStr` takes both of these, which is why the parser is this crate's.
        assert!(parse_log_level("trace").is_err());
        assert!(parse_log_level("off").is_err());
        assert!(parse_log_level("nonsense").is_err());
    }
}

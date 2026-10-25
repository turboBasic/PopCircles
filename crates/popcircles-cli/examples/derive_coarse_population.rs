use std::fs::{self, File};
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;

use anyhow::{Context, bail};
use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::{PixelType, RasterSource, RasterSpec, geotiff::GeoTiffSource};
use sha2::{Digest, Sha256};
use tiff::encoder::{Compression, TiffEncoder, colortype::Gray32Float};
use tiff::tags::Tag;

const FACTOR: u32 = 20;
const SOURCE_KEY: &str = "population-count-2020-30arcsec";
const SOURCE_SHA256: &str = "956993aa500774aed548c8e1af1a3a68fc164577be82ca799d4ae8568d445e9d";
const NODATA: f32 = f32::from_bits(0xff7f_fffd);
const SOURCE_WIDTH: u32 = 43_200;
const SOURCE_HEIGHT: u32 = 21_600;
const SOURCE_STEP: f64 = 0.008_333_333_333_333_333;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let Some(first) = args.next() else {
        bail!(
            "usage: cargo run -p popcircles-cli --example derive_coarse_population -- <source.tif> <output.tif> | --self-check"
        );
    };
    if first == "--self-check" {
        self_check()?;
        println!("coarse raster self-check passed");
        return Ok(());
    }
    let Some(output) = args.next() else {
        bail!(
            "usage: cargo run -p popcircles-cli --example derive_coarse_population -- <source.tif> <output.tif>"
        );
    };
    if args.next().is_some() {
        bail!("expected exactly a source and output path");
    }
    derive(Path::new(&first), Path::new(&output))
}

fn derive(source_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    let source_spec = RasterSpec {
        grid: Grid::new(
            SOURCE_WIDTH,
            SOURCE_HEIGHT,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            SOURCE_STEP,
            -SOURCE_STEP,
        )?,
        epsg: 4326,
        pixel: PixelType::Float32,
        nodata: NODATA,
    };
    let digest = digest(source_path)?;
    if digest != SOURCE_SHA256 {
        bail!("{SOURCE_KEY} source digest mismatch: expected {SOURCE_SHA256}, found {digest}");
    }
    let mut source = GeoTiffSource::open(source_path, &source_spec)
        .with_context(|| format!("opening registered source {SOURCE_KEY}"))?;
    let grid = source.grid();
    let width = grid.width().div_ceil(FACTOR);
    let height = grid.height().div_ceil(FACTOR);
    let mut sums = vec![0.0; usize::try_from(width)? * usize::try_from(height)?];
    let mut source_total = Sum::default();
    while let Some(row) = source.next_row() {
        let row = row?;
        accumulate_row(
            &mut sums,
            width,
            row.row.get(),
            FACTOR,
            row.values,
            &mut source_total,
        )?;
    }
    let tallies = source.finish();
    let (values, rounding_bound) = finish_sums(sums)?;
    let coarse_total = compensated_sum(values.iter().map(|value| f64::from(*value)));
    let operations = f64::from(FACTOR * FACTOR);
    let unit_rounding_error = f64::EPSILON / 2.0;
    let block_sum_bound = (operations * unit_rounding_error
        / (1.0 - operations * unit_rounding_error))
        * source_total.value();
    let tolerance = rounding_bound + block_sum_bound + f64::EPSILON * source_total.value() * 4.0;
    if (coarse_total - source_total.value()).abs() > tolerance {
        bail!(
            "coarse total {coarse_total:.9} differs from source total {:.9} by more than the Float32 rounding bound {tolerance:.9}",
            source_total.value()
        );
    }
    let output_grid = Grid::new(
        width,
        height,
        grid.origin(),
        grid.lon_step() * f64::from(FACTOR),
        grid.lat_step() * f64::from(FACTOR),
    )?;
    let output_spec = RasterSpec {
        grid: output_grid,
        ..source_spec
    };
    let part = output_path.with_extension("tif.part");
    let write_result = (|| -> anyhow::Result<()> {
        let mut file = File::create(&part).context("creating temporary output")?;
        write_geotiff(&mut file, &output_spec, &values)?;
        file.sync_all().context("syncing temporary output")?;
        fs::rename(&part, output_path).context("publishing coarse raster")?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&part);
    }
    write_result?;
    println!("source={SOURCE_KEY} sha256={digest}");
    println!(
        "source_grid={}x{} coarse_grid={}x{} factor={FACTOR}",
        grid.width(),
        grid.height(),
        width,
        height
    );
    println!(
        "source_total={:.9} coarse_total={coarse_total:.9} tolerance={tolerance:.9}",
        source_total.value()
    );
    println!(
        "source_nodata={} output={}",
        tallies.nodata,
        output_path.display()
    );
    Ok(())
}

fn finish_sums(sums: Vec<f64>) -> anyhow::Result<(Vec<f32>, f64)> {
    // Float32 is the registered reader's storage type; track its per-cell rounding for the total bound.
    let mut rounding = Sum::default();
    let values = sums
        .into_iter()
        .map(|sum| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the raster format stores each checked aggregate as Float32"
            )]
            let value = sum as f32;
            rounding.add((f64::from(value) - sum).abs());
            value
        })
        .collect::<Vec<_>>();
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        bail!("coarse values must be finite and nonnegative");
    }
    Ok((values, rounding.value()))
}

fn write_geotiff<W: Write + Seek>(
    writer: &mut W,
    spec: &RasterSpec,
    values: &[f32],
) -> anyhow::Result<()> {
    let grid = spec.grid;
    let mut tiff = TiffEncoder::new(writer)?.with_compression(Compression::Lzw);
    let mut image = tiff.new_image::<Gray32Float>(grid.width(), grid.height())?;
    image.encoder().write_tag(
        Tag::ModelPixelScaleTag,
        &[grid.lon_step(), -grid.lat_step(), 0.0][..],
    )?;
    image.encoder().write_tag(
        Tag::ModelTiepointTag,
        &[0.0, 0.0, 0.0, grid.origin().lon, grid.origin().lat, 0.0][..],
    )?;
    image.encoder().write_tag(
        Tag::GeoKeyDirectoryTag,
        &[
            1_u16, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, spec.epsg,
        ][..],
    )?;
    let nodata = format!("{:e}", spec.nodata);
    image
        .encoder()
        .write_tag(Tag::Unknown(42113), nodata.as_str())?;
    image.write_data(values)?;
    Ok(())
}

fn digest(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 20];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn self_check() -> anyhow::Result<()> {
    let source = [1.0, 2.0, 3.0, NODATA, 0.0, 5.0, 7.0, 8.0, 9.0];
    let (sums, source_total) = aggregate(&source, 3, 3, 2)?;
    let (values, rounding_bound) = finish_sums(sums)?;
    let total_bound = rounding_bound + 4.0 * f64::EPSILON * source_total;
    if source_total.to_bits() != 35.0_f64.to_bits()
        || (compensated_sum(values.iter().map(|value| f64::from(*value))) - source_total).abs()
            > total_bound
    {
        bail!("synthetic total exceeded its rounding bound {total_bound:.9}");
    }
    if values != [3.0, 8.0, 15.0, 9.0] {
        bail!("unexpected coarse values: {values:?}");
    }
    let grid = Grid::new(
        2,
        2,
        LatLon {
            lat: 90.0,
            lon: -180.0,
        },
        1.0,
        -1.0,
    )?;
    let spec = RasterSpec {
        grid,
        epsg: 4326,
        pixel: PixelType::Float32,
        nodata: NODATA,
    };
    let mut first = Cursor::new(Vec::new());
    let mut second = Cursor::new(Vec::new());
    write_geotiff(&mut first, &spec, &values)?;
    write_geotiff(&mut second, &spec, &values)?;
    let bytes = first.into_inner();
    if bytes != second.into_inner() {
        bail!("identical inputs did not produce identical GeoTIFF bytes");
    }
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(&bytes)?;
    let mut decoded = GeoTiffSource::open(file.path(), &spec)?;
    let mut decoded_values = Vec::new();
    while let Some(row) = decoded.next_row() {
        decoded_values.push(row?.values.to_vec());
    }
    if decoded_values != [vec![3.0, 8.0], vec![15.0, 9.0]] {
        bail!("registered reader returned unexpected values: {decoded_values:?}");
    }
    Ok(())
}

fn aggregate(
    values: &[f32],
    width: u32,
    height: u32,
    factor: u32,
) -> anyhow::Result<(Vec<f64>, f64)> {
    let coarse_width = width.div_ceil(factor);
    let coarse_height = height.div_ceil(factor);
    let mut sums = vec![0.0; usize::try_from(coarse_width)? * usize::try_from(coarse_height)?];
    let mut total = Sum::default();
    for y in 0..height {
        let start = usize::try_from(y)? * usize::try_from(width)?;
        let end = start + usize::try_from(width)?;
        accumulate_row(
            &mut sums,
            coarse_width,
            y,
            factor,
            &values[start..end],
            &mut total,
        )?;
    }
    Ok((sums, total.value()))
}

fn accumulate_row(
    sums: &mut [f64],
    coarse_width: u32,
    row: u32,
    factor: u32,
    values: &[f32],
    total: &mut Sum,
) -> anyhow::Result<()> {
    let coarse_y = usize::try_from(row / factor)? * usize::try_from(coarse_width)?;
    let factor = usize::try_from(factor)?;
    for (x, value) in values.iter().copied().enumerate() {
        let value = if value.is_finite() && value >= 0.0 {
            value
        } else {
            0.0
        };
        total.add(f64::from(value));
        sums[coarse_y + x / factor] += f64::from(value);
    }
    Ok(())
}

#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}

impl Sum {
    fn add(&mut self, value: f64) {
        let next = self.value + value;
        self.correction += if self.value.abs() >= value.abs() {
            (self.value - next) + value
        } else {
            (value - next) + self.value
        };
        self.value = next;
    }

    fn value(&self) -> f64 {
        self.value + self.correction
    }
}

fn compensated_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = Sum::default();
    for value in values {
        sum.add(value);
    }
    sum.value()
}

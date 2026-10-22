// A generated GeoTIFF keeps the combined-workflow tests independent of fetched raster bytes.
use std::error::Error;
use std::path::PathBuf;

#[derive(Debug)]
struct Tag {
    id: u16,
    kind: u16,
    count: u32,
    bytes: Vec<u8>,
}

fn short(id: u16, value: u16) -> Tag {
    Tag {
        id,
        kind: 3,
        count: 1,
        bytes: value.to_le_bytes().to_vec(),
    }
}

fn long(id: u16, value: u32) -> Tag {
    Tag {
        id,
        kind: 4,
        count: 1,
        bytes: value.to_le_bytes().to_vec(),
    }
}

fn doubles(id: u16, values: &[f64]) -> Result<Tag, Box<dyn Error>> {
    Ok(Tag {
        id,
        kind: 12,
        count: u32::try_from(values.len())?,
        bytes: values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
    })
}

fn raster() -> Result<Vec<u8>, Box<dyn Error>> {
    let pixels: Vec<u8> = [-1.0_f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let keys: [u16; 16] = [1, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326];
    let mut tags = vec![
        long(256, 4),
        long(257, 2),
        short(258, 32),
        short(259, 1),
        short(262, 1),
        long(273, 0),
        short(277, 1),
        long(278, 2),
        long(279, u32::try_from(pixels.len())?),
        short(284, 1),
        short(339, 3),
        doubles(33550, &[90.0, 90.0, 0.0])?,
        doubles(33922, &[0.0, 0.0, 0.0, -180.0, 90.0, 0.0])?,
        Tag {
            id: 34735,
            kind: 3,
            count: 16,
            bytes: keys.into_iter().flat_map(u16::to_le_bytes).collect(),
        },
        Tag {
            id: 42113,
            kind: 2,
            count: 3,
            bytes: b"-1\0".to_vec(),
        },
    ];
    tags.sort_by_key(|tag| tag.id);
    let end = 8 + 2 + tags.len() * 12 + 4;
    let mut extra = Vec::new();
    let mut directory = Vec::new();
    let mut strip_position = 0;
    for tag in tags {
        directory.extend_from_slice(&tag.id.to_le_bytes());
        directory.extend_from_slice(&tag.kind.to_le_bytes());
        directory.extend_from_slice(&tag.count.to_le_bytes());
        if tag.id == 273 {
            strip_position = directory.len();
        }
        if tag.bytes.len() <= 4 {
            let mut inline = tag.bytes;
            inline.resize(4, 0);
            directory.extend(inline);
        } else {
            if extra.len() % 2 != 0 {
                extra.push(0);
            }
            directory.extend_from_slice(&u32::try_from(end + extra.len())?.to_le_bytes());
            extra.extend(tag.bytes);
        }
    }
    directory[strip_position..strip_position + 4]
        .copy_from_slice(&u32::try_from(end + extra.len())?.to_le_bytes());
    let mut bytes = b"II\x2a\x00\x08\x00\x00\x00".to_vec();
    bytes.extend_from_slice(&u16::try_from(directory.len() / 12)?.to_le_bytes());
    bytes.extend(directory);
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend(extra);
    bytes.extend(pixels);
    Ok(bytes)
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide the fixture directory")?,
    );
    std::fs::create_dir_all(root.join("data"))?;
    std::fs::create_dir_all(root.join("data/boundaries"))?;
    std::fs::create_dir_all(root.join("data/settlements"))?;
    std::fs::write(root.join("data/tiny.tif"), raster()?)?;
    std::fs::write(
        root.join("data/boundaries/gisco-countries-2024-1m.geojson"),
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"CNTR_ID":"XX","NAME_ENGL":"Fixture"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[90,0],[90,90],[0,90],[0,0]]]}}]}"#,
    )?;
    std::fs::write(
        root.join("data/settlements/populated-places-1to10m.geojson"),
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"NAME":"Fixture","ADM0NAME":"Fixture"},"geometry":{"type":"Point","coordinates":[0,0]}}]}"#,
    )?;
    std::fs::write(
        root.join("data/registry.toml"),
        r#"
[datasets.tiny]
kind = "population-raster"
path = "data/tiny.tif"
bytes = 1
sha256 = "0"
source_url = "https://example.invalid/tiny"
licence = "CC0"
licence_url = "https://creativecommons.org/publicdomain/zero/1.0/"
attribution = "Generated workflow fixture"
width = 4
height = 2
origin_lat = 90.0
origin_lon = -180.0
lat_step = -90.0
lon_step = 90.0
epsg = 4326
nodata = -1.0

[datasets.populated-places-1to10m]
kind = "settlement-vector"
path = "data/settlements/populated-places-1to10m.geojson"
bytes = 0
sha256 = "fixture"
source_url = "https://example.invalid/fixture"
licence = "public domain"
licence_url = "https://example.invalid/licence"
attribution = ""

[datasets.gisco-countries-2024-1m]
kind = "boundary-vector"
path = "data/boundaries/gisco-countries-2024-1m.geojson"
bytes = 1
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
source_url = "https://example.invalid/fixture"
licence = "CC0"
licence_url = "https://creativecommons.org/publicdomain/zero/1.0/"
attribution = "Fixture country boundary"
format = "GeoJSON"
crs = "EPSG:4326"
axis_order = "longitude-latitude"
feature_count = 1
revision = "fixture"
source_id_field = "CNTR_ID"
identity_policy = "fixture"
"#,
    )?;
    Ok(())
}

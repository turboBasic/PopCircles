# Data

This folder contains the source data used by the project. The files are described for the program in
[`registry.toml`][registry] and explained here. Results made from the source data, such as lookup
tables and maps, are generated when needed and are not stored here.

```text
data/
  population/   population rasters
  terrain/      elevation rasters used only to present maps
  boundaries/   coastlines and country borders
  settlements/  populated places
  context/      water and transport vectors used on maps
```

Each type of data has its own folder, a row in [`registry.toml`][registry], and an entry below.
Filenames use lowercase words separated by hyphens. If there are several resolutions, the filename
includes the resolution. **These filenames are project labels, not the publishers' original names.** The
information below identifies each source file. Rename a downloaded file to match its heading before
using it.

## Getting it

Large data files are kept in a download rather than in the source code repository. They are not included
when you first clone the project. Fetch them with:

```sh
mise run setup
mise run data:get   # fetch every registered dataset not already here, and verify it
```

`data:get` does not require an account. It reads [`registry.toml`][registry] and checks each
download against its recorded SHA-256 checksum before saving it. It skips files that are already
present and correct. If a download fails the check, it is not saved. After a successful download, the
command prints any credit required by the data's licence.

A small dataset is included in the repository, so it is already available after cloning. The coastline
below is one; maps can use it without downloading anything. The registry says whether each dataset is
included or must be fetched.
Obtaining an independent copy from the publisher, to check a republished asset against its source, is
[`CONTRIBUTING.md`][contributing-data].

## etopo-2022-60s

| Property | Value |
| --- | --- |
| Grid | 21 600 × 10 800 (60 arc-second) |
| Extent | whole globe, origin (−180°, 90°) |
| CRS | EPSG:9518 (WGS 84) |
| Pixel type | Float32, single band |
| Nodata | −99 999 |
| Size | 465 969 062 bytes |
| SHA-256 | `9d27d4b8ea8e76977e2988bca667d7c8fa68b927355feffcddd6b4875a7fd08e` |

The [NOAA NCEI ETOPO 2022 60 arc-second GeoTIFF][etopo-file] supplies map terrain outside
Switzerland. The checksum and size identify the exact publisher file. ETOPO is CC0-1.0 and is free
for private, academic and commercial use. NCEI asks users to cite the source; maps carry the
attribution from the registry. The grid is intentionally coarse compared with the Swiss national
grid, but it provides consistent global coverage for the other seven reference maps. See the
[product page][etopo] for documentation and citation details.

The terrain renderer uses this registered raster only for map presentation. It does not read
population data or affect published results.

## etopo-2022-15s-n45e000

| Property | Value |
| --- | --- |
| Grid | 3 600 × 3 600 (15 arc-second) |
| Extent | 0–15°E, 30–45°N |
| CRS | EPSG:9518 (WGS 84) |
| Pixel type | Float32, single band |
| Nodata | −99 999 |
| Size | 25 195 718 bytes |
| SHA-256 | `86f7ba59e904807fee7e035ada1ad8e761fd89d658574e91efa9e0e727f8d5d0` |

The [NOAA NCEI ETOPO 2022 15 arc-second tile][etopo-15s-file] is an available regional terrain
source. It is the same CC0-1.0 product as the global 60 arc-second raster. The Andorra map uses the
Copernicus GLO-90 tile below instead.

## copdem-glo-90-n42-e001

| Property | Value |
| --- | --- |
| Grid | 1 200 × 1 200 (30 arc-second; about 90 m) |
| Extent | 1–2°E, 42–43°N |
| CRS | EPSG:4326 (WGS 84) |
| Pixel type | Float32, single band, Cloud Optimized GeoTIFF |
| Nodata | Not set in the source |
| Size | 5 295 321 bytes |
| SHA-256 | `f3d7ce54cdc6ac7f00297a54b2e3c28814a069360d1ae2ecd9d76358c76cdf43` |

The [Copernicus DEM GLO-90 tile][copdem-glo-90-file] provides relief for Andorra's production map.
The public tile is a Cloud Optimized GeoTIFF derived from COP-DEM GLO-90-DGED. Its required
adapted-data attribution is recorded in the registry and printed on maps. GLO-90 is available under
its full, free and open licence.

## Additional Copernicus GLO-90 tiles

These checksum-pinned 1° Cloud Optimized GeoTIFF tiles provide high-resolution terrain overlays for selected country maps. Their grids vary by latitude to keep the nominal ground resolution near 90 m. The registry owns each tile’s fetch URL, licence and required attribution. The source is Copernicus DEM GLO-90-DGED, available under its full, free and open licence.

| Dataset key | Grid | Tile | SHA-256 | Size (bytes) |
| --- | --- | --- | --- | ---: |
| `copdem-glo-90-s15-w172` | 1200 × 1200 | 14–15°S, 171–172°W | `8404431ed145d677d025c7d22ca93301e67e6d09b6efa8af65ee2c70ca12cc0d` | 145,722 |
| `copdem-glo-90-s14-w173` | 1200 × 1200 | 13–14°S, 172–173°W | `a4c5dbc397acb2b7079c1097f6c7d436152670d440a4d71d281446a2b2923e69` | 903,173 |
| `copdem-glo-90-s14-w172` | 1200 × 1200 | 13–14°S, 171–172°W | `71a375027536cdd520289ec145901d641a7f71c72e5535e74a19fc5fa93c9060` | 497,520 |
| `copdem-glo-90-s13-e043` | 1200 × 1200 | 12–13°S, 43–44°E | `6e1c18f89f5b8c17213f1a34148b445f01112c5ce6f27d6be1cdff572b8ebc4e` | 175,830 |
| `copdem-glo-90-s13-e044` | 1200 × 1200 | 12–13°S, 44–45°E | `6a2a7e2f0166478e79002108e85e81dd2179f0ae0048b6598e138e2f711ce985` | 285,270 |
| `copdem-glo-90-s12-e043` | 1200 × 1200 | 11–12°S, 43–44°E | `ba9411d926d8ddf15d2c16c877d98375d0ea79e8386b33e4bb506d288ee2d201` | 565,356 |
| `copdem-glo-90-n01-e103` | 1200 × 1200 | 1–2°N, 103–104°E | `598a0c08b6182c06ec21c2061c66f9d9f1b7a132dd5999de75c13387d17950a3` | 3,740,051 |
| `copdem-glo-90-n01-e104` | 1200 × 1200 | 1–2°N, 104–105°E | `2ec9d30033de435ec37967925f1caa1c954853dba0a9d7d54bc368b47c01cc3c` | 1,494,297 |
| `copdem-glo-90-n04-e114` | 1200 × 1200 | 4–5°N, 114–115°E | `71c18fe4390ec180ae59f115e39a4ea5e540352386e29421af17f0456fff36f6` | 4,539,057 |
| `copdem-glo-90-n04-e115` | 1200 × 1200 | 4–5°N, 115–116°E | `cfb6a8eed51f81b2f8d0163297f4b41fe1652ef0831da63c3b3f84b619244847` | 5,487,694 |
| `copdem-glo-90-n05-e114` | 1200 × 1200 | 5–6°N, 114–115°E | `59efbc696cc8fa64e657854491b00bb951eb85c53b3cc6cb1e27da2fbf152705` | 57,202 |
| `copdem-glo-90-n05-e115` | 1200 × 1200 | 5–6°N, 115–116°E | `b8ed387bf0fa1bf5919657761b8536602e43b2b2850e2130269abee348023ba8` | 2,093,774 |
| `copdem-glo-90-n10-w063` | 1200 × 1200 | 10–11°N, 62–63°W | `1d3983d745cd5cf1ac03000f34711bfb34b3327c5e7e5d618399ad4e3ac87bd9` | 2,183,503 |
| `copdem-glo-90-n10-w062` | 1200 × 1200 | 10–11°N, 61–62°W | `cb1a7655346672124f58b00126b3d8712506e893186c3be5a005b52121a0184b` | 2,502,500 |
| `copdem-glo-90-n10-w061` | 1200 × 1200 | 10–11°N, 60–61°W | `44561924f0d37cf47f5522a7ebdef7603c9eb95d289c5a6ac548cabecd77a6d6` | 109,374 |
| `copdem-glo-90-n11-w062` | 1200 × 1200 | 11–12°N, 61–62°W | `b435ad03067ad8e456e96d6b49254c9ab257aede8ea6e1af8bc97f177b64c104` | 52,928 |
| `copdem-glo-90-n11-w061` | 1200 × 1200 | 11–12°N, 60–61°W | `43c464c409f4f10551a172c7e2aec348b393be632dbff78a23fa8b916120b9ad` | 222,852 |
| `copdem-glo-90-n12-w070` | 1200 × 1200 | 12–13°N, 69–70°W | `6e1a4f071ac1616983121d74f60deee7652af3e39af8a0dde38318eec59f3bc3` | 377,189 |
| `copdem-glo-90-n12-w069` | 1200 × 1200 | 12–13°N, 68–69°W | `9a6f3f62c282acb89db4858a05a240bd7b0eb05b7e43b3b3853020503bf6b041` | 335,224 |
| `copdem-glo-90-n12-w062` | 1200 × 1200 | 12–13°N, 61–62°W | `fe9f914e380b2aaa4bbe8b6439b8229783b9df00fddd321a946a13e8d9d6b488` | 298,460 |
| `copdem-glo-90-n13-w091` | 1200 × 1200 | 13–14°N, 90–91°W | `e57a0693db94b1960929ce25b938830e0e3c99e5ccb5b3e779b805c2d08fd78c` | 912,457 |
| `copdem-glo-90-n13-w090` | 1200 × 1200 | 13–14°N, 89–90°W | `ed6142f5fb40a17227b4ffeea50d03c0ff59e74f50446e50a0fd8a624fc4b177` | 2,772,869 |
| `copdem-glo-90-n13-w089` | 1200 × 1200 | 13–14°N, 88–89°W | `504ff606cf1dbadd0a09e277bc34778f7db11d6f2f060e83f738387d2955108a` | 4,519,629 |
| `copdem-glo-90-n13-w088` | 1200 × 1200 | 13–14°N, 87–88°W | `e669a6c1ff852f696c43ea341ca06c8cba0e99da955446b7c63cb5b1d5386df0` | 4,815,544 |
| `copdem-glo-90-n13-w062` | 1200 × 1200 | 13–14°N, 61–62°W | `e35ef1369dc8b76e9149a8668755a44725469c1d4c833cbcc9a96301537e8bb9` | 352,202 |
| `copdem-glo-90-n13-w061` | 1200 × 1200 | 13–14°N, 60–61°W | `4494946e4865ff158f5facbb37c0c673433da2c112be39a0847ddf67d588ce41` | 246,735 |
| `copdem-glo-90-n13-w060` | 1200 × 1200 | 13–14°N, 59–60°W | `db18d167495cd2b90b243aadce020205eec72bdb14a0e55ac6b78e1f4b11dd56` | 279,832 |
| `copdem-glo-90-n13-w017` | 1200 × 1200 | 13–14°N, 16–17°W | `5fc5776a29311b2b3e86b45c1e658a0e135492991362c21271c9df799c809db8` | 3,993,362 |
| `copdem-glo-90-n13-w016` | 1200 × 1200 | 13–14°N, 15–16°W | `888de7f3b3b2f265c888c84b927483c6fa6887e2c637c479c142c208ef81f08b` | 5,655,853 |
| `copdem-glo-90-n13-w015` | 1200 × 1200 | 13–14°N, 14–15°W | `a2e2a5c9bfbf2d22226bff557cd76840879e9eeae4bef7d082b79e27fe47992e` | 5,606,314 |
| `copdem-glo-90-n13-w014` | 1200 × 1200 | 13–14°N, 13–14°W | `812e80031eb9047e60d1889bf9ad9ad79edbdf550ae12dcf8a7f6a61b2e3eaec` | 5,569,813 |
| `copdem-glo-90-n14-w090` | 1200 × 1200 | 14–15°N, 89–90°W | `9b13eeadff1c743ed586320cd502123ea2583444b2d004acddc1d68f5119c7eb` | 5,398,917 |
| `copdem-glo-90-n14-w089` | 1200 × 1200 | 14–15°N, 88–89°W | `eb5a832abad6247ae526f1f5d0aa21c9c2d59a731ff101e185c1d6c936a9693a` | 5,468,696 |
| `copdem-glo-90-n14-w062` | 1200 × 1200 | 14–15°N, 61–62°W | `9add676ad1e7b62a635a36b3598224099d3d355af4b4f910e24fdc546e411815` | 398,005 |
| `copdem-glo-90-n14-w061` | 1200 × 1200 | 14–15°N, 60–61°W | `198637ac8f03c84f4eec5b0a601fd2946b17cf656cd52bc06d63dfddfa1345b4` | 402,808 |
| `copdem-glo-90-n15-w062` | 1200 × 1200 | 15–16°N, 61–62°W | `c87a78083a3a4b11979b7ddec7f9a1b5e00355745091151e375c1572dbfbf825` | 587,827 |
| `copdem-glo-90-n16-w063` | 1200 × 1200 | 16–17°N, 62–63°W | `bba317b6e30f71f2f77fa271107feea60c9afcf64b974394f2f290f9f5628451` | 114,303 |
| `copdem-glo-90-n16-w062` | 1200 × 1200 | 16–17°N, 61–62°W | `4e757a34065f1417cbe3a6c4e0fad91f73baf6763948d330533fa1238c0e3ee5` | 819,270 |
| `copdem-glo-90-n17-w068` | 1200 × 1200 | 17–18°N, 67–68°W | `7d66190ef42295b1dac39193751352e9beddbd7155d85c94eee031631ff544bb` | 110,725 |
| `copdem-glo-90-n17-w067` | 1200 × 1200 | 17–18°N, 66–67°W | `1427bca2fa2152e704de1476cc0a735f688790e96576a4208bec5c5f0dcba92d` | 297,894 |
| `copdem-glo-90-n17-w066` | 1200 × 1200 | 17–18°N, 65–66°W | `50b2c6cac03c88c72e56d7de85bb6018669758c6662a918cc5107bd0e7f14038` | 70,486 |
| `copdem-glo-90-n17-w065` | 1200 × 1200 | 17–18°N, 64–65°W | `a0dff9935fedde9a59d1d40e15b4bf1c6e72eb7a445496dac41c4c7830b9d46e` | 179,428 |
| `copdem-glo-90-n17-w063` | 1200 × 1200 | 17–18°N, 62–63°W | `0f1683ff3b7f2796b3916f608e4eb6c335afb8fe6283f0d5280f7d76e591d647` | 240,048 |
| `copdem-glo-90-n17-w062` | 1200 × 1200 | 17–18°N, 61–62°W | `836639acd5bf3e6816455745c2d6c5ebc14be236f5a338b3fe4aa72b5dad0ec2` | 310,622 |
| `copdem-glo-90-n18-w068` | 1200 × 1200 | 18–19°N, 67–68°W | `ad6da8dba33f11032d9a8bfe2453dffdfe965ad20901e0fcb2e1bf011800061c` | 690,948 |
| `copdem-glo-90-n18-w067` | 1200 × 1200 | 18–19°N, 66–67°W | `4f4d285e0458ce84f05b1ec88d6eb96ff485b86ee99e3d3a924f9b7dc91d9c4c` | 2,813,291 |
| `copdem-glo-90-n18-w066` | 1200 × 1200 | 18–19°N, 65–66°W | `994da5de165b6658368af372dfa8802c31b935a390ac1f04a52cce7fc9513028` | 934,100 |
| `copdem-glo-90-n18-w065` | 1200 × 1200 | 18–19°N, 64–65°W | `e627b2fdff7b7ae289595042f31b0d5988519d583b322708c05606b36956defe` | 258,957 |
| `copdem-glo-90-n19-w082` | 1200 × 1200 | 19–20°N, 81–82°W | `8962485d4970e129f9b6f6e8fac1eb22b3bb9a9343fb7c5a499ec33d79879b3d` | 174,414 |
| `copdem-glo-90-n19-w081` | 1200 × 1200 | 19–20°N, 80–81°W | `8f195dc044d6daaa0c8288dd50e1482026fcb2af7d937bac0dbe76de620c7d97` | 66,693 |
| `copdem-glo-90-n19-w080` | 1200 × 1200 | 19–20°N, 79–80°W | `01959ad0a064232e23b95537c0905642bafd924e3e5bc6cab114e0cfb1087016` | 79,933 |
| `copdem-glo-90-n22-e113` | 1200 × 1200 | 22–23°N, 113–114°E | `d0958bde0ff54d7eeab0f87d947865faef88d1d49d276a772bf22902e7861397` | 4,394,459 |
| `copdem-glo-90-n22-e114` | 1200 × 1200 | 22–23°N, 114–115°E | `6e8557cb632ea331fbbe0c2a72f5718e38112c3c6d36fbd881aab41ce8714eb8` | 3,013,342 |
| `copdem-glo-90-n25-e050` | 1200 × 1200 | 25–26°N, 50–51°E | `7c46412420780b34bc060fe8aeb2b770f2587235c7539e8080cf01b2ce914a70` | 3,173,812 |
| `copdem-glo-90-n26-e050` | 1200 × 1200 | 26–27°N, 50–51°E | `58049a5530daa24cb0ae26fcf69a9f0e3804714642e9642834049d603b97a6fb` | 974,312 |
| `copdem-glo-90-n26-e088` | 1200 × 1200 | 26–27°N, 88–89°E | `54425329d717b5dd69d7b84c2c768e1a1d7712963334d6a9a99777988f0af15d` | 5,344,007 |
| `copdem-glo-90-n26-e089` | 1200 × 1200 | 26–27°N, 89–90°E | `378ba8221ec9b26fa07ce73d1a84b27e6edd4a8ee66c2e8c89957fe42132b3f1` | 5,516,222 |
| `copdem-glo-90-n26-e090` | 1200 × 1200 | 26–27°N, 90–91°E | `698fae6adfe5cb436f39972d6fda0ef4e301290df9526c522819219b07b4e21c` | 5,374,294 |
| `copdem-glo-90-n26-e091` | 1200 × 1200 | 26–27°N, 91–92°E | `a08feb412bb3526749fe06928331176332538c72e82495adc456e1424fa62593` | 5,315,884 |
| `copdem-glo-90-n26-e092` | 1200 × 1200 | 26–27°N, 92–93°E | `658f7794cedfb1b06d53fd8f8fcd475d5a187e5973fbf5b29b9ab28a446b140a` | 5,136,212 |
| `copdem-glo-90-n27-e088` | 1200 × 1200 | 27–28°N, 88–89°E | `c277a2996a42b38aae483e82fe8c09307ffa8da90545472a30a41d3f34ba132b` | 5,179,650 |
| `copdem-glo-90-n27-e089` | 1200 × 1200 | 27–28°N, 89–90°E | `41f89a667e23e504d4b4edfed3b5ffa038ca45c676214ae2e199caa2ed444c61` | 5,062,900 |
| `copdem-glo-90-n27-e090` | 1200 × 1200 | 27–28°N, 90–91°E | `54740a8441dfd58c6ed644b125d52e9e3d27b52c5ac1497e542241158cf801ec` | 5,176,337 |
| `copdem-glo-90-n27-e091` | 1200 × 1200 | 27–28°N, 91–92°E | `17b65b6d3022e89c2d437049c10efaaecabad47d634c7d600acf69fc4c1bcb18` | 5,255,368 |
| `copdem-glo-90-n27-e092` | 1200 × 1200 | 27–28°N, 92–93°E | `5b62756dad40b7c258a9ae791b79b30b24c70430b142929fcdd054c9fddd2be4` | 5,296,289 |
| `copdem-glo-90-n28-e089` | 1200 × 1200 | 28–29°N, 89–90°E | `2882aca31d3c4fb28321d7cc4098889edafd2eeca042b2b0de5081dfb2d84f04` | 4,594,604 |
| `copdem-glo-90-n28-e090` | 1200 × 1200 | 28–29°N, 90–91°E | `8d7d85b58e8f29af55ff1e0dade9fa813742cb5f8af65cbb05edb8c1e092d644` | 4,393,250 |
| `copdem-glo-90-n28-e091` | 1200 × 1200 | 28–29°N, 91–92°E | `3292f1dd4f7c9833bfd50fbb8c5c03a18ceb1e0f16b1cce5da6abc0deb84ae01` | 4,649,900 |
| `copdem-glo-90-n34-e032` | 1200 × 1200 | 34–35°N, 32–33°E | `511519a48ea01eb38be74ed8fcc069f0640b4bddf205fc0e092cf116ecba7dcf` | 1,268,182 |
| `copdem-glo-90-n34-e033` | 1200 × 1200 | 34–35°N, 33–34°E | `923de0d443c504fedb7ae97deede93787d42c144a9f866403a9ed4b4adb4b7d2` | 1,085,945 |
| `copdem-glo-90-n34-e034` | 1200 × 1200 | 34–35°N, 34–35°E | `f3bc808b384e30414cb00f7f886568bcb939b9656d4a04a1d8919fda84d94a48` | 66,505 |
| `copdem-glo-90-n35-e014` | 1200 × 1200 | 35–36°N, 14–15°E | `26233d94356c5efb0baf91e654e3d2d4f6aae4ebee60dfca23fdc1f6e1802cbc` | 213,530 |
| `copdem-glo-90-n35-e032` | 1200 × 1200 | 35–36°N, 32–33°E | `342a51c6ce692e0f76e6e6ad70230a2b530b06ab70896267c7c37db177394f37` | 736,316 |
| `copdem-glo-90-n35-e033` | 1200 × 1200 | 35–36°N, 33–34°E | `4b9ceca19b521dd751a82a66daaeb925d6573aa8ef9b1a92ab772376c0e6ca89` | 2,002,028 |
| `copdem-glo-90-n35-e034` | 1200 × 1200 | 35–36°N, 34–35°E | `501ea5eb69655d543c671efd843e8b9662578e28ff60568bf0fb3aba892382db` | 410,530 |
| `copdem-glo-90-n36-e014` | 1200 × 1200 | 36–37°N, 14–15°E | `c2b9ef0a229e348387dd09785472e550e9248201a91db22fd14ed8fa54198bd6` | 940,523 |
| `copdem-glo-90-n47-e009` | 1200 × 1200 | 47–48°N, 9–10°E | `51f5f20d1a5c8351dd8f8a18e8ae376bf93253c300d95a6e0b54c9fe75640a4c` | 4,879,071 |
| `copdem-glo-90-n49-e005` | 1200 × 1200 | 49–50°N, 5–6°E | `24c547ea84cbe73fdb952cef3ef3d80502ff8c17a3b46f91d381bceea1393552` | 5,253,246 |
| `copdem-glo-90-n49-e006` | 1200 × 1200 | 49–50°N, 6–7°E | `a92c383e679ef4fc30956fcc47af3b2b616a647b9eaf8d6a1cbc74990b310bac` | 5,334,254 |
| `copdem-glo-90-n50-e005` | 800 × 1200 | 50–51°N, 5–6°E | `a6c18de320ba39b7d4c61aa224224fdab9879f16ba1fccd44a4cdd30841f6c9f` | 3,749,957 |
| `copdem-glo-90-n50-e006` | 800 × 1200 | 50–51°N, 6–7°E | `70f5eedcea6c6cee2c4379ba2a32a0068b5618f327f7dd58158277ecaf3562c7` | 3,703,764 |
| `copdem-glo-90-n54-w005` | 800 × 1200 | 54–55°N, 4–5°W | `0113446f69d9b080c3dc22058e74b8bbd7d0c052d9682a08c4bc449c752d8def` | 1,260,682 |
| `copdem-glo-90-n61-w008` | 600 × 1200 | 61–62°N, 7–8°W | `a1e1511142093fbabe42784e1a45c0b524048eadf33e5c621a5854527e8a4688` | 50,671 |
| `copdem-glo-90-n61-w007` | 600 × 1200 | 61–62°N, 6–7°W | `d5fbbc9c1a6e01425f44fbf162869d5b871a2aa48e565e447d697377cfc7b963` | 268,638 |
| `copdem-glo-90-n62-w008` | 600 × 1200 | 62–63°N, 7–8°W | `c1fc294473d2f600143340fde1e7c072e0da1fdb5c70ee9027d338620c4fa035` | 300,753 |
| `copdem-glo-90-n62-w007` | 600 × 1200 | 62–63°N, 6–7°W | `7ed2fb98cddc4619797fd16e26051d4bb9c6dfa6eaafad97870d3a012b72778f` | 441,640 |

| `copdem-glo-90-s55-e003` | 800 × 1200 | 54–55°S, 3–4°E | `802e567cdd3dfac52cfb56a5e5f788b8b78e2c07fadd10e33bab39b44dd1fd7c` | 80,890 |
| `copdem-glo-90-s54-e072` | 800 × 1200 | 53–54°S, 72–73°E | `b380fe04143d64425b244e9871f51b8488788692c3a2fd5479ca29b2a14380e4` | 52,610 |
| `copdem-glo-90-s54-e073` | 800 × 1200 | 53–54°S, 73–74°E | `811044293d928c640555b24a4fa1578b6d3698fa407e26ef15b15e9dd9d86458` | 243,202 |
| `copdem-glo-90-s53-w062` | 800 × 1200 | 52–53°S, 61–62°W | `cfdfc412d797773da50c1d7ed7762f195ab352e006590248db9c2c8c5b5584a5` | 67,126 |
| `copdem-glo-90-s53-w061` | 800 × 1200 | 52–53°S, 60–61°W | `f161583753f1ff88653eefeec61feee41be758c34df7a54bdb501c60bdcef03e` | 530,026 |
| `copdem-glo-90-s53-w060` | 800 × 1200 | 52–53°S, 59–60°W | `6ab1590ee002c9ad44a7481a4b8f9b4b9c84f3d35817cc21a0590051b4f044f5` | 690,200 |
| `copdem-glo-90-s53-w059` | 800 × 1200 | 52–53°S, 58–59°W | `efb2a63edd69f0293f463ac4064e6104282688a43edda35a0e90c30235d67386` | 243,158 |
| `copdem-glo-90-s53-e073` | 800 × 1200 | 52–53°S, 73–74°E | `dfe8b73f4b28c01fc47d7c008050c031481ad1779b209f6d9c7f28fbe92624e4` | 61,181 |
| `copdem-glo-90-s52-w062` | 800 × 1200 | 51–52°S, 61–62°W | `5f2ad7c2acf68c562c178081d374727ca61c130e6b202bc3187e559490829746` | 233,128 |
| `copdem-glo-90-s52-w061` | 800 × 1200 | 51–52°S, 60–61°W | `062b1ccfc66e545c707fd9a903b2fbd627346feada8585cd667666f7f88c8f7c` | 1,344,193 |
| `copdem-glo-90-s52-w060` | 800 × 1200 | 51–52°S, 59–60°W | `952057f9accf704b0ed6e2c60e240d1dceb22d2b5d9c60fea302e96f6b12fd37` | 1,894,610 |
| `copdem-glo-90-s52-w059` | 800 × 1200 | 51–52°S, 58–59°W | `0fac346878740ec9d8fe8ab25b021b06df9512d8f1d4a2ec87865ac12b2dc621` | 2,134,056 |
| `copdem-glo-90-s52-w058` | 800 × 1200 | 51–52°S, 57–58°W | `035e24ecdb482929147c5f61cc1e1704e076c5cf37f9d468d3c257760812bfc2` | 295,497 |
| `copdem-glo-90-s51-w062` | 800 × 1200 | 50–51°S, 61–62°W | `ac1d2798df34e548f60e84e0a41f31e9f4b757f4b79a6a00c1e4b214328bf432` | 50,137 |
| `copdem-glo-90-s31-e027` | 1200 × 1200 | 30–31°S, 27–28°E | `2b612b012823ae13feff576cf0bff7cbb6d01556c1cafb185cdcc2fc84ead24b` | 5,049,867 |
| `copdem-glo-90-s31-e028` | 1200 × 1200 | 30–31°S, 28–29°E | `7c980b51670afdb0144399197b0f558f6e56edcaa778eee7700974533c296b0c` | 5,056,645 |
| `copdem-glo-90-s31-e029` | 1200 × 1200 | 30–31°S, 29–30°E | `4be452915c0832a7867ba575a66451794c990e05dc4d1d4da29f3780be2ea226` | 5,061,593 |
| `copdem-glo-90-s30-e027` | 1200 × 1200 | 29–30°S, 27–28°E | `16af1dd4558017ddec53ada7bed83f6eb2cec813af6aad327da507b72041b4e6` | 4,892,798 |
| `copdem-glo-90-s30-e028` | 1200 × 1200 | 29–30°S, 28–29°E | `4156676dad67c6cb577f2ab8f165043b4a8bc508928ed3112257e5fc3a85dc2a` | 5,056,817 |
| `copdem-glo-90-s30-e029` | 1200 × 1200 | 29–30°S, 29–30°E | `97d139c2d3e3611755fd20e98a6029f12ec6909cc3434b12852d9eb1c6a0504b` | 5,030,797 |
| `copdem-glo-90-s30-e167` | 1200 × 1200 | 29–30°S, 167–168°E | `c4e2688029a201b06c19876daff900d923daa460f23983df8ee6ee9615ceeb30` | 77,641 |
| `copdem-glo-90-s29-e027` | 1200 × 1200 | 28–29°S, 27–28°E | `549a221a390e11a81df17a22f16a0736a7a98a767ac00947db8cc22c026c25d3` | 4,516,360 |
| `copdem-glo-90-s29-e028` | 1200 × 1200 | 28–29°S, 28–29°E | `c7fd7c3805a7f22e101bbbcf4765ed3198f252f38d22d391d6c830f5ccf03a95` | 4,873,044 |
| `copdem-glo-90-s29-e029` | 1200 × 1200 | 28–29°S, 29–30°E | `c6470dcb4a1a3edd880d2e0bee90fdd29b39e4d3e2a0d4eef0ff8d421d560a0a` | 4,804,683 |
| `copdem-glo-90-s29-e167` | 1200 × 1200 | 28–29°S, 167–168°E | `422fe83a8f19e60d30eddeee95878dfd44de829554957885c264ab9e92bf1ae3` | 50,358 |
| `copdem-glo-90-s28-e030` | 1200 × 1200 | 27–28°S, 30–31°E | `4df88bbd607e668d431784aa11037a66fe474867b3fe06df2a9f096628bc365a` | 4,768,424 |
| `copdem-glo-90-s28-e031` | 1200 × 1200 | 27–28°S, 31–32°E | `dcf5443b6e1e4027a6fb3825e1e52ff40c0396b3a70283f727a252acb2332588` | 5,262,886 |
| `copdem-glo-90-s27-e030` | 1200 × 1200 | 26–27°S, 30–31°E | `01cd5a3b65b91abeafe45ab1aff438541ddcfaebfbf98740d58930ae696ec3d0` | 4,761,792 |
| `copdem-glo-90-s27-e031` | 1200 × 1200 | 26–27°S, 31–32°E | `74e2b4314b571067bdd1982c7ca5035966c84618b6c44fca34cbdc0c4d46e9be` | 5,252,069 |
| `copdem-glo-90-s27-e032` | 1200 × 1200 | 26–27°S, 32–33°E | `e189b89bb9eb2a73c6aa72d33ecd9225a6e09e162e87b5e520c5ff7249ab0e50` | 4,904,592 |
| `copdem-glo-90-s26-e031` | 1200 × 1200 | 25–26°S, 31–32°E | `e692cdcf08cf651d53d5d3ec2b1cbf83e893c74668c7e3cffbc2557f8d4fb143` | 5,216,834 |
| `copdem-glo-90-s26-e032` | 1200 × 1200 | 25–26°S, 32–33°E | `da4efa12412331f416cff14a0b0bbd6a29e671438daa13d2336299e5540d2c4a` | 4,811,043 |
| `copdem-glo-90-s23-w177` | 1200 × 1200 | 22–23°S, 176–177°W | `9515955b5bb4624485fa1350129df8de41d3fc04d2d95b83fef6907bc137fdcf` | 51,943 |
| `copdem-glo-90-s22-w176` | 1200 × 1200 | 21–22°S, 175–176°W | `708e010075a9334a5e1f6c8a376d365b4464068efe73da7c97f79ed056bd8cda` | 216,402 |
| `copdem-glo-90-s22-w175` | 1200 × 1200 | 21–22°S, 174–175°W | `80f9b17603406f64e42773d04aebed216c2e88ffea5ac4945cece266d969c446` | 108,514 |
| `copdem-glo-90-s21-w176` | 1200 × 1200 | 20–21°S, 175–176°W | `47367944f7906a2318f5e2ca7b0a6de8fabcaae432cd7652f77fc16970a0cb42` | 51,705 |
| `copdem-glo-90-s21-w175` | 1200 × 1200 | 20–21°S, 174–175°W | `99a6cfa414e739160f43ec5cd3aa7da68dbe9b5476987f57af29fd3281f339c7` | 64,769 |
| `copdem-glo-90-s20-w176` | 1200 × 1200 | 19–20°S, 175–176°W | `14c74b731bf7fa24402fe59225ab59cbb5e44a6e2cc739e766b3462183b9c479` | 88,985 |
| `copdem-glo-90-s20-w175` | 1200 × 1200 | 19–20°S, 174–175°W | `985bddf9c54c3f2cadfa4648894ad26766b4573da0869d2d74e1e8df9d878123` | 96,198 |
| `copdem-glo-90-s20-w170` | 1200 × 1200 | 19–20°S, 169–170°W | `5c5c53dc87eb8e41ea8a1699d0c730fb69b344eb3607a2af5b7e1179d1665890` | 165,507 |
| `copdem-glo-90-s19-w175` | 1200 × 1200 | 18–19°S, 174–175°W | `2f5e5e4feb86ad6bbe72368a77078bac5680f4234511eaf2405304550e1412f1` | 118,295 |
| `copdem-glo-90-s19-w174` | 1200 × 1200 | 18–19°S, 173–174°W | `7e18972dd686eeea6117bc0c22f80157d96b06011a0f2f8c5d21056c82a254ae` | 111,729 |
| `copdem-glo-90-s19-w170` | 1200 × 1200 | 18–19°S, 169–170°W | `5baaa207fa223286073d25eec17acd6a0d0924c0a2d246c251da1dc0dcb883df` | 77,345 |
| `copdem-glo-90-s16-w176` | 1200 × 1200 | 15–16°S, 175–176°W | `45e550244c7140faab234ae11f822ef9495242157126fff36e89e965ae09e2c9` | 76,990 |
| `copdem-glo-90-s16-w174` | 1200 × 1200 | 15–16°S, 173–174°W | `f5bac6504f76c3ac2601b48d3ccd8b2ae3165dfbeeb2601dd62e4b5a429df5fc` | 66,183 |
| `copdem-glo-90-s15-w179` | 1200 × 1200 | 14–15°S, 178–179°W | `375dcbf0434178f4f1b31a642347ac735f39e1cbc7ad48916eb8111f17b70c89` | 92,505 |
| `copdem-glo-90-s15-w171` | 1200 × 1200 | 14–15°S, 170–171°W | `03c2d61255ed0f3a6ac1db8776ce0fd0aa1021ede62237b12b8f52b394555d07` | 139,033 |
| `copdem-glo-90-s15-w170` | 1200 × 1200 | 14–15°S, 169–170°W | `d6e72504a79fd7bda50c9800c38729a1d1985d2326345aabc12945cf648d1e9f` | 87,994 |
| `copdem-glo-90-s15-w169` | 1200 × 1200 | 14–15°S, 168–169°W | `da46993af26f03bdfec96d91e508073c2e56b4014f82ed097c9163ea55f36314` | 50,214 |
| `copdem-glo-90-s14-w177` | 1200 × 1200 | 13–14°S, 176–177°W | `1a153222a0b0d463cd95e85f616ca48923883c2aaf12dd8a09511a9cef44daf4` | 103,925 |
| `copdem-glo-90-s13-e096` | 1200 × 1200 | 12–13°S, 96–97°E | `02a070525b35948a62156a8d2a1ec3e87c7436f4121202f68b51ab6b72395268` | 66,759 |
| `copdem-glo-90-s12-w172` | 1200 × 1200 | 11–12°S, 171–172°W | `73cfa63bc726a5194e756db2d2390e6754e4b72bc6a090fe0f1727e24326fea1` | 52,598 |
| `copdem-glo-90-s12-e096` | 1200 × 1200 | 11–12°S, 96–97°E | `1c50b8162afed46d11f007ff351aea3a61f8f6a0ca3806cddb11a1b6acf3717b` | 49,894 |
| `copdem-glo-90-s11-e105` | 1200 × 1200 | 10–11°S, 105–106°E | `790e9c2e42692528a7d6bc0649bb7e09ad025697b0d3c1d7405e59a3c4b7fadb` | 127,618 |
| `copdem-glo-90-s11-e179` | 1200 × 1200 | 10–11°S, 179–180°E | `0a4c1433c4072c88bf47aeecafa32cc1fc2255353f2b511d58d193718389d0f6` | 50,489 |
| `copdem-glo-90-s10-w172` | 1200 × 1200 | 9–10°S, 171–172°W | `f6f0ac7ea8a9e6bf05c378a99a659212942d2979f7cf4a93ff2401a9fd5fc5a3` | 67,772 |
| `copdem-glo-90-s10-e124` | 1200 × 1200 | 9–10°S, 124–125°E | `7719817402ebdaffbc0f6c3eeb323d68433bc7d5fc0519c560dcd51f0a330c22` | 4,387,122 |
| `copdem-glo-90-s10-e125` | 1200 × 1200 | 9–10°S, 125–126°E | `dbc5f0ac61406984bdfa1c659be596334dda6542c66eda5445c329d8de40ad2b` | 1,594,931 |
| `copdem-glo-90-s10-e126` | 1200 × 1200 | 9–10°S, 126–127°E | `87a8e4095c54b56f58ff4b53ab8a1ed979ed04bb631453de8a0df71070926872` | 95,910 |
| `copdem-glo-90-s10-e179` | 1200 × 1200 | 9–10°S, 179–180°E | `32c54bea7bb6a97b7888d33a8c4b7596539360b90e0c977c2428f8ec504e8ff3` | 55,046 |
| `copdem-glo-90-s09-w173` | 1200 × 1200 | 8–9°S, 172–173°W | `918745159daf422f47688f9b0d0b8d32bf1f0982ef1c3a2edd9ed481d64a7b80` | 56,630 |
| `copdem-glo-90-s09-e124` | 1200 × 1200 | 8–9°S, 124–125°E | `efdfa65c06821114972d63360335e711252c6bcca56788ec7aedb001082d7da9` | 1,357,576 |
| `copdem-glo-90-s09-e125` | 1200 × 1200 | 8–9°S, 125–126°E | `476d3c5437a0b27873881e6c9d7d44dd9dc24c9a72c88055094c995892a45ad5` | 2,701,820 |
| `copdem-glo-90-s09-e126` | 1200 × 1200 | 8–9°S, 126–127°E | `36ac26ee824292c10c8aaa2c56a2fc3b916605bc7c4612bee3bb1f384ee14f9b` | 2,542,695 |
| `copdem-glo-90-s09-e127` | 1200 × 1200 | 8–9°S, 127–128°E | `d117fd488d5307dcf858a443382f560a93fde3e78aafe5ecdc3b64a8137aca1b` | 608,415 |
| `copdem-glo-90-s09-e178` | 1200 × 1200 | 8–9°S, 178–179°E | `6be06aba13df2712066bfbf9f7760dfd31360dcd38e1481f978660d77833c70c` | 54,617 |
| `copdem-glo-90-s09-e179` | 1200 × 1200 | 8–9°S, 179–180°E | `6a740c377aa5c531fd4e2a2dfe87f3332a4495cf273e460aae6708083fd0e23d` | 58,999 |
| `copdem-glo-90-s08-e072` | 1200 × 1200 | 7–8°S, 72–73°E | `0fefa561cdbd8c781df166ff38dc9792602e178b6ad11098321848e89e7cb004` | 83,862 |
| `copdem-glo-90-s08-e177` | 1200 × 1200 | 7–8°S, 177–178°E | `85c6fd60887801d55393fcb32145ff9c1055502b0c96160db739f432897a5ded` | 58,011 |
| `copdem-glo-90-s08-e178` | 1200 × 1200 | 7–8°S, 178–179°E | `df2a1159f4295f791dd79d351de532355a1bf70970c5ab6a4fa743b9d9b1d972` | 58,578 |
| `copdem-glo-90-s07-e071` | 1200 × 1200 | 6–7°S, 71–72°E | `ed1e8c8c655da03624f13bba41685295b041c8927f01be2d5f2b50163ad47246` | 60,448 |
| `copdem-glo-90-s07-e176` | 1200 × 1200 | 6–7°S, 176–177°E | `a2a1fdca775b8fd15972435d2c0aea0dc50304c06348afe6865282a1c217366f` | 53,064 |
| `copdem-glo-90-s07-e177` | 1200 × 1200 | 6–7°S, 177–178°E | `0926113eac015e8dc9ab97b01c42ea7141dbf08481b46375876fd7d2c5356e7e` | 52,062 |
| `copdem-glo-90-s06-e071` | 1200 × 1200 | 5–6°S, 71–72°E | `cdd626d16feeea02082c52e0a373293a580929f368db763b2a00dd23d63a979f` | 63,651 |
| `copdem-glo-90-s06-e072` | 1200 × 1200 | 5–6°S, 72–73°E | `6ec7af6f3da69d07f3988e7a485124ae1fc7221daa32331f04ef7e9f11c3b2c1` | 55,432 |
| `copdem-glo-90-s06-e176` | 1200 × 1200 | 5–6°S, 176–177°E | `68de2daddd646c0e0d6c179861af5b70ee5fcf1daf5adf43c2f54507ef858f96` | 54,903 |
| `copdem-glo-90-s05-e029` | 1200 × 1200 | 4–5°S, 29–30°E | `cef89828188cab023b698b20dd359cda72c5d5b4b207fd3250e050e43ab14889` | 2,933,939 |
| `copdem-glo-90-s05-e030` | 1200 × 1200 | 4–5°S, 30–31°E | `48cf444a7636781263df1a89eadc11220547c316399ccca718447d16169e4c03` | 4,492,649 |
| `copdem-glo-90-s04-e029` | 1200 × 1200 | 3–4°S, 29–30°E | `d125572e093e73e13723c7675ee15fbc915817d783f3e1f9cb60234aeee98507` | 4,487,922 |
| `copdem-glo-90-s04-e030` | 1200 × 1200 | 3–4°S, 30–31°E | `4c2ac21d7f656e2d9085a8e877ee091ac6471053930fe844de1ba35fdeb7075a` | 4,858,144 |
| `copdem-glo-90-s03-e028` | 1200 × 1200 | 2–3°S, 28–29°E | `009f34f9a42e87d3790d05d2f7606cd661cb1b31793fa67f62e89b448dc89078` | 5,143,607 |
| `copdem-glo-90-s03-e029` | 1200 × 1200 | 2–3°S, 29–30°E | `71d0585a65d5e897f9fb7d31731d654fe82a7c1e59769799bb74fc4fc04a3afc` | 4,946,103 |
| `copdem-glo-90-s03-e030` | 1200 × 1200 | 2–3°S, 30–31°E | `7b9c50a086040738454797bd52cada7af32c876924eb8d887ae523e0b3be2aa1` | 4,838,817 |
| `copdem-glo-90-s02-e029` | 1200 × 1200 | 1–2°S, 29–30°E | `0242799492f6a13be03a378a5511ac78596c5276eb3c740646a2fee20babd7d8` | 4,707,981 |
| `copdem-glo-90-s02-e030` | 1200 × 1200 | 1–2°S, 30–31°E | `11d296ea42578e815662df45d805bc65726c0f2b7f22fcf43ffa197b3674d4f8` | 4,909,584 |
| `copdem-glo-90-s01-e006` | 1200 × 1200 | 0–1°S, 6–7°E | `ee8f48ea0139ee5affbc6c957cb054f822b15d40afb690bbf234eb71e80c05b5` | 51,775 |
| `copdem-glo-90-s01-e073` | 1200 × 1200 | 0–1°S, 73–74°E | `deaeba40752c3345ae93f22aeb4040cd2447898d85581e04a158909280b99b14` | 72,407 |
| `copdem-glo-90-s01-e166` | 1200 × 1200 | 0–1°S, 166–167°E | `249df95fc5e921ae87b2aaf2a547ff451b8879c076c2a295ab5c508232fbb9fa` | 64,819 |
| `copdem-glo-90-n00-e006` | 1200 × 1200 | 0–1°N, 6–7°E | `0dc963aff9cb03c884fccaa8370264978e5b9196eebda6af589733aa5ae14e95` | 490,119 |
| `copdem-glo-90-n00-e072` | 1200 × 1200 | 0–1°N, 72–73°E | `a6ba5ed158092142058400d2d3da7004c88fed996f06c8040fed144d53a89b7c` | 54,690 |
| `copdem-glo-90-n00-e073` | 1200 × 1200 | 0–1°N, 73–74°E | `ceca7dbfb37b6cae16ff2288087524988074ed936b21b212caba534bb0d44d74` | 99,288 |
| `copdem-glo-90-n01-w059` | 1200 × 1200 | 1–2°N, 58–59°W | `06a30dee8a50148e3d3a5a7b373cae31fd6c42cb459cfbe628dd919ee61c9e3e` | 5,598,119 |
| `copdem-glo-90-n01-w058` | 1200 × 1200 | 1–2°N, 57–58°W | `2c0b7e948053f8b1f070eb4963d1f43754e15704871a61c5d2896d57c315c00c` | 5,618,674 |
| `copdem-glo-90-n01-w057` | 1200 × 1200 | 1–2°N, 56–57°W | `9ef4e959f12b1fa87139315583c953f58055f9f20f2c8d879c56f012d25051bb` | 5,499,439 |
| `copdem-glo-90-n01-w056` | 1200 × 1200 | 1–2°N, 55–56°W | `27eca6aba9f1047c2dde42cead9ccd4e5ff792b86db3b66e7ad1049f46025d7e` | 5,351,625 |
| `copdem-glo-90-n01-e007` | 1200 × 1200 | 1–2°N, 7–8°E | `9025f3b01d6eb9ced5276e8eeefdf918944047d99fddace19746253ff4e7a1a7` | 136,350 |
| `copdem-glo-90-n01-e010` | 1200 × 1200 | 1–2°N, 10–11°E | `ee07241f0eee553d29aca30500d11f2e56f088e8a187b691f357b4cf9a94c3cc` | 5,503,148 |
| `copdem-glo-90-n01-e011` | 1200 × 1200 | 1–2°N, 11–12°E | `7e1b7db60615315ba0077566014110b333596733ccada7d62c2a1aefd2dce58c` | 5,239,403 |
| `copdem-glo-90-n01-e073` | 1200 × 1200 | 1–2°N, 73–74°E | `23cab5eeae118c8f77f537cfc001a5f37e84dfb79f075466b4f45f416f51e67f` | 71,457 |
| `copdem-glo-90-n02-w059` | 1200 × 1200 | 2–3°N, 58–59°W | `b0d6cdfb7994a3c94575381737862580c6a0e37e4f7712ebd8e0090cd306e7db` | 5,549,842 |
| `copdem-glo-90-n02-w058` | 1200 × 1200 | 2–3°N, 57–58°W | `6a9307af19dcbec60f5487930a3c51cee290111cce37dd87060ef4182ddc6253` | 5,573,753 |
| `copdem-glo-90-n02-w057` | 1200 × 1200 | 2–3°N, 56–57°W | `029712514b6604daeacb4b2d25af569f32bdf9bb35694bfd99acd4fd16698fcf` | 5,556,279 |
| `copdem-glo-90-n02-w056` | 1200 × 1200 | 2–3°N, 55–56°W | `ca7ae97c52f91ce0da3ef63a7724da7fbc77058643b91fc69d72736d27c2dd94` | 5,583,876 |
| `copdem-glo-90-n02-w055` | 1200 × 1200 | 2–3°N, 54–55°W | `aa9b7e13ed97854a9615b2213eb1db1779b2b0b544b7e4e07c586c9c00d0f41c` | 5,671,632 |
| `copdem-glo-90-n02-e011` | 1200 × 1200 | 2–3°N, 11–12°E | `370a15b6b7f954553b16cf3852faddd47076114772f1e093863251ca76d95754` | 5,198,279 |
| `copdem-glo-90-n02-e072` | 1200 × 1200 | 2–3°N, 72–73°E | `b0840ca3fd6421db141f89dcb90debd812e882857129642c09dd66021bdf5c15` | 57,701 |
| `copdem-glo-90-n02-e073` | 1200 × 1200 | 2–3°N, 73–74°E | `9bf00a5d89d76d83c3dd61c093b9e15107bb1f6bda297cd1401e9b3591b88fe8` | 81,449 |
| `copdem-glo-90-n02-e131` | 1200 × 1200 | 2–3°N, 131–132°E | `2e16ca77ddcced06bb483879113605cbbd94388a9f3a487f589f0d3692873c45` | 49,993 |
| `copdem-glo-90-n03-w059` | 1200 × 1200 | 3–4°N, 58–59°W | `af8371bafe51e402e03ca919fcdabc8229033bcfe9969318a5be907b8c5c3b78` | 5,749,748 |
| `copdem-glo-90-n03-w058` | 1200 × 1200 | 3–4°N, 57–58°W | `37b043d21ab9c9403795f7c84492232f86974069b343aac8726f7405bcecd21c` | 5,644,308 |
| `copdem-glo-90-n03-w057` | 1200 × 1200 | 3–4°N, 56–57°W | `065d7afc56d24d30f71a7ccb5e6ee8be0d6b14f5b1917ce8a4c7f1cc9ad19bf9` | 5,643,744 |
| `copdem-glo-90-n03-w056` | 1200 × 1200 | 3–4°N, 55–56°W | `41b716af4175afaac33b07933f0b4aa600f848c893464f8724f2e3125b21844e` | 5,670,506 |
| `copdem-glo-90-n03-w055` | 1200 × 1200 | 3–4°N, 54–55°W | `b20d8000d05443666aa816f0a18d13c4a708dac84a16fa56cc83a3d554dd2f7d` | 5,771,020 |
| `copdem-glo-90-n03-w054` | 1200 × 1200 | 3–4°N, 53–54°W | `c972111a7bcbb8308728e3b4c83a5b91765b6042b2684e5b30d58e922cb60146` | 5,696,383 |
| `copdem-glo-90-n03-e072` | 1200 × 1200 | 3–4°N, 72–73°E | `4cf6d99b02e3221a467f5b117f78e25fc47d8882d6b320093eab999a569a7541` | 63,617 |
| `copdem-glo-90-n03-e073` | 1200 × 1200 | 3–4°N, 73–74°E | `ab856656e94def5d7246460509dd0c6b8b1200a7d95b17432bf74db7431cc3d5` | 58,197 |
| `copdem-glo-90-n03-e131` | 1200 × 1200 | 3–4°N, 131–132°E | `a143977d31d29d22718abc4bb46ae7069cf52f8065795249dd950ce1485c0074` | 50,996 |
| `copdem-glo-90-n04-w059` | 1200 × 1200 | 4–5°N, 58–59°W | `2e9b759943c547a5477cf7b9b695146b2726aebd266ebdddd97ed6010b814722` | 5,689,831 |
| `copdem-glo-90-n04-w058` | 1200 × 1200 | 4–5°N, 57–58°W | `423e88ce9f40ce8d93f8782543949edcca087b2242700fefb0ab5184b70debb5` | 5,756,975 |
| `copdem-glo-90-n04-w057` | 1200 × 1200 | 4–5°N, 56–57°W | `d048a916ccd1e7a85f9a0ffdf817518e4d99dadc12c2918aaf730ad468856e6c` | 5,810,543 |
| `copdem-glo-90-n04-w056` | 1200 × 1200 | 4–5°N, 55–56°W | `2ea3ea865167b972ea25170f8d8a764a52d25a48db35f3fdfb36003b5853887a` | 5,423,410 |
| `copdem-glo-90-n04-w055` | 1200 × 1200 | 4–5°N, 54–55°W | `59c6f352f0f51ae4874106254626d2b687a31c54a180e00a2a68a1bb7091a8e2` | 5,722,989 |
| `copdem-glo-90-n04-w010` | 1200 × 1200 | 4–5°N, 9–10°W | `85c9740dd2e53692aafff5bd42beb0c7dad7e10b000ffd3bf427a70dc140b10e` | 57,586 |
| `copdem-glo-90-n04-w009` | 1200 × 1200 | 4–5°N, 8–9°W | `741f7cfccecf4664cc2f643e9906d0d107db9b166e58c9831ce527a27a0b0432` | 1,713,112 |
| `copdem-glo-90-n04-w008` | 1200 × 1200 | 4–5°N, 7–8°W | `2a0cc56473920eef27911666d2b876550eb419ddc1bf861888caef26dec2ca3c` | 3,374,006 |
| `copdem-glo-90-n04-e034` | 1200 × 1200 | 4–5°N, 34–35°E | `a828c76dd7abdc66bfe03758516c00261fa6c2d2beb52cbdd23ebd0f16493561` | 4,347,805 |
| `copdem-glo-90-n04-e035` | 1200 × 1200 | 4–5°N, 35–36°E | `147ceba7b36d627b62820c2405352ed29439a5561240856d441be9575435a2cb` | 4,586,452 |
| `copdem-glo-90-n04-e072` | 1200 × 1200 | 4–5°N, 72–73°E | `9aab6f4915a18d79730e07b16fef84bb54ed87499ef2cce6f9ea5c740ff1759a` | 58,069 |
| `copdem-glo-90-n04-e073` | 1200 × 1200 | 4–5°N, 73–74°E | `db5f19489a74779652f80866a248fc96956481255057da7d9ff8f73c72ecfad1` | 75,862 |
| `copdem-glo-90-n04-e131` | 1200 × 1200 | 4–5°N, 131–132°E | `9938e7e525ef8a3b549b9a1e23591be8d2ad55b2113400ea0973b75edb416089` | 50,501 |
| `copdem-glo-90-n04-e132` | 1200 × 1200 | 4–5°N, 132–133°E | `a0c7809a5d75db18a0b135349a1285baa0552f169c35f5ea90c147d39d203ea7` | 51,444 |
| `copdem-glo-90-n05-w059` | 1200 × 1200 | 5–6°N, 58–59°W | `1ebd1fc8c37d5f0d154da38ef83b3cb818c471bca3b37f6f65911aa2a893ed2a` | 5,760,846 |
| `copdem-glo-90-n05-w058` | 1200 × 1200 | 5–6°N, 57–58°W | `de55af376cf8f0c8f75ff0bf6fffc7408362782f9d458167188dc47db0c08149` | 5,788,700 |
| `copdem-glo-90-n05-w057` | 1200 × 1200 | 5–6°N, 56–57°W | `f82e142ffc61a76708039fcf19ae82f8d1bf34b8cab76a9cf6891214d867c95f` | 5,519,412 |
| `copdem-glo-90-n05-w056` | 1200 × 1200 | 5–6°N, 55–56°W | `6af4a8c4d81b9cbe9e07e9489a1d9a82aeeea2316fa80e68a4bd9cf4e61eb87c` | 5,723,373 |
| `copdem-glo-90-n05-w055` | 1200 × 1200 | 5–6°N, 54–55°W | `ce0564d5c9bf82400b1aa2d30b99a3ff326684c0b8c6c76baf0b1fc896259466` | 5,567,652 |
| `copdem-glo-90-n05-w054` | 1200 × 1200 | 5–6°N, 53–54°W | `6da0a49ae669a7e461772df4f20ce5d9d153a62ca0e2173f0717cbd6ac5b0180` | 3,658,590 |
| `copdem-glo-90-n05-w011` | 1200 × 1200 | 5–6°N, 10–11°W | `5ad2a8e11f3a01ce970a4b0f1fe31686b1b6e0e941e1cacd4bb0540b0db168b0` | 163,749 |
| `copdem-glo-90-n05-w010` | 1200 × 1200 | 5–6°N, 9–10°W | `163f0bbac92da9b3a47dc2bd9c9968c57f0f2fde64aaf8e9b8e7f7678c852177` | 3,741,257 |
| `copdem-glo-90-n05-w009` | 1200 × 1200 | 5–6°N, 8–9°W | `940ca7ebae6a23d1667ebbcf66778ff41c3bcd73f7fd40aa33f97c23ca4e5b99` | 5,715,892 |
| `copdem-glo-90-n05-w008` | 1200 × 1200 | 5–6°N, 7–8°W | `325a75cbd0cb49ca5d776f91e563360c1381948d3648df0458e2edfe121e7c80` | 5,639,173 |
| `copdem-glo-90-n05-e035` | 1200 × 1200 | 5–6°N, 35–36°E | `7071199d269eee7e6d42ab022c612703fc2a6faf249ab71ebf28c06085e44c0b` | 4,804,704 |
| `copdem-glo-90-n05-e072` | 1200 × 1200 | 5–6°N, 72–73°E | `c8005ce6a77e9a81e29c5fc86195fc21b36e6120ad29a2eca34a752c148c9fd9` | 63,827 |
| `copdem-glo-90-n05-e073` | 1200 × 1200 | 5–6°N, 73–74°E | `bc0a26ba46662222a0453910da9b80cd464ee713dfe931c858a8ec7fe2255f43` | 107,524 |
| `copdem-glo-90-n05-e080` | 1200 × 1200 | 5–6°N, 80–81°E | `a5a5bed2aa313fda5a202f494ded7985745a6977c62fe3e61002f3320ae62986` | 185,109 |
| `copdem-glo-90-n05-e132` | 1200 × 1200 | 5–6°N, 132–133°E | `0b98d117a2a33ba3835b897354606fe89d03631c6cc61702c1a3b164d99a811a` | 52,024 |
| `copdem-glo-90-n06-w059` | 1200 × 1200 | 6–7°N, 58–59°W | `93c9c23258cd9d0cc9d54e3636c23af0d6cca53353d6c012bf1307a074937547` | 5,412,497 |
| `copdem-glo-90-n06-w058` | 1200 × 1200 | 6–7°N, 57–58°W | `7d3292656e8cec5816066c6b1d56afcc298360ed656264a46fe2a4571a40b9af` | 2,253,741 |
| `copdem-glo-90-n06-w057` | 1200 × 1200 | 6–7°N, 56–57°W | `8f0d545ba6e822bd078e9a342056a0994f9c65f96ab7cbaaf018d3ef5c588ba1` | 50,041 |
| `copdem-glo-90-n06-w056` | 1200 × 1200 | 6–7°N, 55–56°W | `55bed2b51400e60fdbac2d56e9d5bd2740e1de21892834520c05529fadaea17d` | 51,647 |
| `copdem-glo-90-n06-w012` | 1200 × 1200 | 6–7°N, 11–12°W | `63ef6820d1da707f9140c61b05b5359907fd9e3b0225a7ceebf0a6bb5508c2d3` | 972,983 |
| `copdem-glo-90-n06-w011` | 1200 × 1200 | 6–7°N, 10–11°W | `2f616e0d740875f55e954cc3a80f730fdfcfce372b1a4958e56566d72b6db78d` | 4,742,218 |
| `copdem-glo-90-n06-w010` | 1200 × 1200 | 6–7°N, 9–10°W | `c6d6de1f4b968cdf285c35c5bb144d1faf4cfa038dca480d2023805b4737b0de` | 5,601,925 |
| `copdem-glo-90-n06-w009` | 1200 × 1200 | 6–7°N, 8–9°W | `a5b5f9a25e51f67716a1823df49a258d4e1f3558ec3f001baa5c279dd36fcde1` | 5,465,887 |
| `copdem-glo-90-n06-w008` | 1200 × 1200 | 6–7°N, 7–8°W | `0eb1bd6776d4bb71d42aa7ee65d5bcff415faf901c61fdfb52c287cdda4028e3` | 5,301,818 |
| `copdem-glo-90-n06-e000` | 1200 × 1200 | 6–7°N, 0–1°E | `b941595741f5f500bb5c59f740625c83b72d0f4abb243febc3c9f0d67451a2b6` | 5,058,959 |
| `copdem-glo-90-n06-e001` | 1200 × 1200 | 6–7°N, 1–2°E | `339548780f44f19079060420f1d820b13798cb8c3c9619a5740522383d60f4dd` | 4,568,318 |
| `copdem-glo-90-n06-e002` | 1200 × 1200 | 6–7°N, 2–3°E | `7f6a69f76d13a985614df7491fdac60de67635121744d3f50eb0242403cdefa7` | 3,664,237 |
| `copdem-glo-90-n06-e072` | 1200 × 1200 | 6–7°N, 72–73°E | `b99a15cbe95dc268056d375794773dbb50f39ccd8afa4e2e585d6ae6b896828c` | 66,331 |
| `copdem-glo-90-n06-e073` | 1200 × 1200 | 6–7°N, 73–74°E | `4add913c8459635749e698d94d80cacd5f0b55aff10a06f60f976565c358da80` | 105,432 |
| `copdem-glo-90-n06-e079` | 1200 × 1200 | 6–7°N, 79–80°E | `8954ea562191bb7c39f62a18b3572135f4db60f575b2919c946b8de07a6bc966` | 408,677 |
| `copdem-glo-90-n06-e080` | 1200 × 1200 | 6–7°N, 80–81°E | `5eddd379c4b27554d154f99f3aaf329226da09658e6c5d0f33fc68097c248e95` | 5,647,341 |
| `copdem-glo-90-n06-e081` | 1200 × 1200 | 6–7°N, 81–82°E | `abb5527b053b40330c24ac2fc018f2249f3e4b74733c4a3230467383d4b00f31` | 3,327,384 |
| `copdem-glo-90-n06-e134` | 1200 × 1200 | 6–7°N, 134–135°E | `eac10eabdea1ca3a53f91faf8b1d7e11c79eae17d813738c2e6053e700b6829a` | 60,348 |
| `copdem-glo-90-n07-w014` | 1200 × 1200 | 7–8°N, 13–14°W | `2bfece4159fa0cc4d7e2db1507b5e23bee626c8c95373965c56411bda345ce45` | 58,596 |
| `copdem-glo-90-n07-w013` | 1200 × 1200 | 7–8°N, 12–13°W | `1d74835c1c0a196f0f8d06d0d684e920957be2d098de3479be4b5ea3255be7c5` | 3,130,408 |
| `copdem-glo-90-n07-w012` | 1200 × 1200 | 7–8°N, 11–12°W | `fd6c37e717bf608417e75ed129aeb02d59dc9bc245cbf134991c5563fc4604b8` | 5,598,211 |
| `copdem-glo-90-n07-w011` | 1200 × 1200 | 7–8°N, 10–11°W | `9f9a7fbf7422269a744875daa8c10da005d468f040f16218c500dea5ded08362` | 5,599,449 |
| `copdem-glo-90-n07-w010` | 1200 × 1200 | 7–8°N, 9–10°W | `144aa79fcd39d5957ba9b88d8d2f1dcc5c2bfdc4eaed7a6fa2ac8cab7862d968` | 5,482,718 |
| `copdem-glo-90-n07-w009` | 1200 × 1200 | 7–8°N, 8–9°W | `3db643f4f00d8685d5ca5f1877571d53fa1472bcf7da205b736d94415be1f802` | 5,464,088 |
| `copdem-glo-90-n07-e000` | 1200 × 1200 | 7–8°N, 0–1°E | `22ffc7f0e821ff87d5307caf6290fba930617f8c567260db7c9d7218bdec2914` | 4,688,709 |
| `copdem-glo-90-n07-e001` | 1200 × 1200 | 7–8°N, 1–2°E | `9ac9391f028e773c1e0d36553dd0aef6cacdddf58c987665fba6f659e4d0fb5c` | 5,210,490 |
| `copdem-glo-90-n07-e002` | 1200 × 1200 | 7–8°N, 2–3°E | `96547bdac79fa9057d6b871d75de68a548291314bce10f1a30ff3697d51356a4` | 5,456,623 |
| `copdem-glo-90-n07-e072` | 1200 × 1200 | 7–8°N, 72–73°E | `c7606703af3a60cbb6e53db26d22845334631da493fb310cda3cfde8096eb704` | 55,510 |
| `copdem-glo-90-n07-e073` | 1200 × 1200 | 7–8°N, 73–74°E | `30f027a4c7ae4e6cf9df1a3e4ae6a6ddfe58a486cb8c4093428a427576006610` | 50,617 |
| `copdem-glo-90-n07-e079` | 1200 × 1200 | 7–8°N, 79–80°E | `476b5b4fa2603fbc596403259ee98067cd68bae4db8b2f6afb1283db814af6f2` | 1,256,174 |
| `copdem-glo-90-n07-e080` | 1200 × 1200 | 7–8°N, 80–81°E | `5c5b7d212081094710022b2e2e47d01380de596bc8bb854cc18b578149fb697f` | 5,706,112 |
| `copdem-glo-90-n07-e081` | 1200 × 1200 | 7–8°N, 81–82°E | `9dd7a5c69553743257925840fdea0a252286a2335c84f639477a2114e44f7000` | 4,355,346 |
| `copdem-glo-90-n07-e134` | 1200 × 1200 | 7–8°N, 134–135°E | `3899934b069998db0d5b8938165ac2e993065f0bc5678c73266124c37aecd5a5` | 344,087 |
| `copdem-glo-90-n08-w014` | 1200 × 1200 | 8–9°N, 13–14°W | `129a1d60a8131e27b54e5f1ed1babe17cdee060c4f669edccab9f092be4ce4a9` | 1,115,706 |
| `copdem-glo-90-n08-w013` | 1200 × 1200 | 8–9°N, 12–13°W | `0864b37aa870123dc59402823b161a0690703664e43b2b0439fbfbd5a2c3e4a4` | 5,730,751 |
| `copdem-glo-90-n08-w012` | 1200 × 1200 | 8–9°N, 11–12°W | `2c1aa2317ededcc335477e231af5e90881dd4c501e2138050f78a8f68a4498b8` | 5,638,263 |
| `copdem-glo-90-n08-w011` | 1200 × 1200 | 8–9°N, 10–11°W | `e211b5861a5c4513848704235ede1065ea6253abb58ad94e514765fa30d93620` | 5,559,989 |
| `copdem-glo-90-n08-w010` | 1200 × 1200 | 8–9°N, 9–10°W | `db6435f7214a1a652f461b749c48d69fa8604a6dd64c69e391609fee168283fa` | 5,387,979 |
| `copdem-glo-90-n08-e000` | 1200 × 1200 | 8–9°N, 0–1°E | `82ba4b71de84cc6c0659d2886f80a0b680f26b745c1ae8cd8e3b62c98e8357ed` | 5,237,783 |
| `copdem-glo-90-n08-e001` | 1200 × 1200 | 8–9°N, 1–2°E | `4cb28b4ca7fd5e607fafa067230568f7711852cb2f5c4f84e984b748c4c106ca` | 5,052,274 |
| `copdem-glo-90-n08-e002` | 1200 × 1200 | 8–9°N, 2–3°E | `55456036a22202fd2df298fe5060efd0c9d577ddc2898cae562a1eecfac42fe4` | 5,175,190 |
| `copdem-glo-90-n08-e079` | 1200 × 1200 | 8–9°N, 79–80°E | `f76a9139afd483aeee5d7c86ef4df0ad8cd5592c50a3391faf70dfc546359ef8` | 910,468 |
| `copdem-glo-90-n08-e080` | 1200 × 1200 | 8–9°N, 80–81°E | `7ee4b5f8157ac143a3e40639acad832a797bb96305722a23d3f150e38a1c1e9f` | 5,515,394 |
| `copdem-glo-90-n08-e081` | 1200 × 1200 | 8–9°N, 81–82°E | `a419c2be5cf37aa98c31ead1f3e06c9ea5ef1c46f67953a85777781df3095351` | 1,689,857 |
| `copdem-glo-90-n08-e111` | 1200 × 1200 | 8–9°N, 111–112°E | `74752bdb84bf9e3f53f486a31b42225eb49f4a6980b818b309332b88ee9ac5a6` | 50,226 |
| `copdem-glo-90-n08-e112` | 1200 × 1200 | 8–9°N, 112–113°E | `2cfa072d3bccb067802f5b6048f82323e1572c40365b53eca0b7e82a101264cd` | 50,063 |
| `copdem-glo-90-n08-e113` | 1200 × 1200 | 8–9°N, 113–114°E | `5dbc9e920bbd7958ebc9813a4b0170e89c58b298a1f89860891fff2f1aab7347` | 49,990 |
| `copdem-glo-90-n08-e134` | 1200 × 1200 | 8–9°N, 134–135°E | `cbeb6bcf9812d370ae1f1c310d2433105cbec59187899c6754ea79f8932c4c75` | 52,662 |
| `copdem-glo-90-n09-w014` | 1200 × 1200 | 9–10°N, 13–14°W | `057758db8dea11d60f574364d40f29335718c89514d68d29e103b018bb2efcbc` | 3,405,525 |
| `copdem-glo-90-n09-w013` | 1200 × 1200 | 9–10°N, 12–13°W | `4831d6a80818b50e148840427a7e105056a43620ce3b77e1e5978d5778a300d3` | 5,760,557 |
| `copdem-glo-90-n09-w012` | 1200 × 1200 | 9–10°N, 11–12°W | `60d242a2dc3c2f3e12074b447607b6052933e1ecf677a58f4872c6e7c0ddfcf4` | 5,633,762 |
| `copdem-glo-90-n09-w011` | 1200 × 1200 | 9–10°N, 10–11°W | `50b8b4565117a9e318b7dbe66d4ab7d14d359fe56c0b07e739931912085e196f` | 5,365,664 |
| `copdem-glo-90-n09-e000` | 1200 × 1200 | 9–10°N, 0–1°E | `55f6ce55ecb35497c4c09dbf9e253ab5e8dd90a0108aefa8767881c738e044a6` | 5,161,358 |
| `copdem-glo-90-n09-e001` | 1200 × 1200 | 9–10°N, 1–2°E | `7907b702a4f5199b3868155bcc95224f682006bfdad353015df72218f0063e51` | 5,058,764 |
| `copdem-glo-90-n09-e002` | 1200 × 1200 | 9–10°N, 2–3°E | `a3060c0594529f816acc74ea5ca12e38b65dd1ed1a5f53c6509a80d7d53c53ee` | 4,902,566 |
| `copdem-glo-90-n09-e003` | 1200 × 1200 | 9–10°N, 3–4°E | `e5908541447d81056aecda70ee4372efbef6b11b22f8ed77824492acf362e803` | 4,857,918 |
| `copdem-glo-90-n09-e027` | 1200 × 1200 | 9–10°N, 27–28°E | `976e8f204d6d9ae1147f8ecb0a10ad4ac7091e8f6dfaa889321dfb56e41163dd` | 4,325,890 |
| `copdem-glo-90-n09-e028` | 1200 × 1200 | 9–10°N, 28–29°E | `07cdbe018c341d26418975952bd79ed543dfd37dec42c10f0e34b29f712846a0` | 4,335,594 |
| `copdem-glo-90-n09-e029` | 1200 × 1200 | 9–10°N, 29–30°E | `5e4343d6b5e16b8045fd45a219cb413db76dde7f924f22b55393d1795e434a33` | 4,212,487 |
| `copdem-glo-90-n09-e079` | 1200 × 1200 | 9–10°N, 79–80°E | `177793db0b60fa0962042522e7b2308fa0070d47dd882a45fea3d3f42c3aa7e5` | 606,165 |
| `copdem-glo-90-n09-e080` | 1200 × 1200 | 9–10°N, 80–81°E | `4fd54465d2610f16096203529eca0111024f0b01a5c5a5eb539a07990e164e66` | 2,616,461 |
| `copdem-glo-90-n09-e114` | 1200 × 1200 | 9–10°N, 114–115°E | `aca8a3b3e10cf68637d2b59c0379f1914e2dbb860f3a2e6b70a6fce6c2f08084` | 50,131 |
| `copdem-glo-90-n09-e115` | 1200 × 1200 | 9–10°N, 115–116°E | `54044090434fb1d44401179f7db46afdb9e57a68e4d11310771a88e457f17f00` | 49,894 |
| `copdem-glo-90-n10-w110` | 1200 × 1200 | 10–11°N, 109–110°W | `3347909b0a47ae3b3f6789d09854ea8477b20d11ada61a920f9dd246deca07be` | 53,503 |
| `copdem-glo-90-n10-w016` | 1200 × 1200 | 10–11°N, 15–16°W | `6c3a76a4d2e521f39c5d67953a88a290328fcf77ba77a62952f656601251c10b` | 163,849 |
| `copdem-glo-90-n10-w015` | 1200 × 1200 | 10–11°N, 14–15°W | `910846effd438607524345db6d96fe089443e74c4fa7b10f458684b51e7b433b` | 3,478,752 |
| `copdem-glo-90-n10-w001` | 1200 × 1200 | 10–11°N, 0–1°W | `7c4d47c819973e3192b1eca9fceebbd683165b1fc8041564fb5349ea56ff9818` | 4,980,778 |
| `copdem-glo-90-n10-e000` | 1200 × 1200 | 10–11°N, 0–1°E | `4590246be4bdeb4d4a977a17a57cf0a6010442627a6baf41d0b5848b0bd86995` | 4,973,821 |
| `copdem-glo-90-n10-e001` | 1200 × 1200 | 10–11°N, 1–2°E | `414655389386764875addfa1356baa42590e2afa6ef3fef935f83770b1aaee52` | 5,047,015 |
| `copdem-glo-90-n10-e002` | 1200 × 1200 | 10–11°N, 2–3°E | `d9999e82e51ca145c10016d3d8d8aa203d7a9c1331099636793110da87489359` | 4,833,881 |
| `copdem-glo-90-n10-e003` | 1200 × 1200 | 10–11°N, 3–4°E | `4b833c6cf78bf0b02c3ab1c3177a152d37b42e9bdc59ef572a31d09ac05eaca8` | 4,846,586 |
| `copdem-glo-90-n10-e027` | 1200 × 1200 | 10–11°N, 27–28°E | `4c44e524828437a159569bcdf6c496c73193763acfe2646a694b01bf3a86221f` | 4,331,325 |
| `copdem-glo-90-n10-e028` | 1200 × 1200 | 10–11°N, 28–29°E | `c27aac4c4e67d7278a566e0dc8fa35dda01b4deedeb64a7e4529c7a294f9baf4` | 4,386,118 |
| `copdem-glo-90-n10-e029` | 1200 × 1200 | 10–11°N, 29–30°E | `abf8cd4cc95d036383d122f5f2ebb1063f9bc65aa5680ef1040740238260e4e9` | 4,490,464 |
| `copdem-glo-90-n10-e041` | 1200 × 1200 | 10–11°N, 41–42°E | `1f81051b2b27434ac3efe1115034bed2291bece68aed41c449a0c948c66e866f` | 4,650,725 |
| `copdem-glo-90-n10-e042` | 1200 × 1200 | 10–11°N, 42–43°E | `23586e5821c2ed9ecbe18f40adf8eac3ae0635a15124942d0d94cb15e86f0aa7` | 4,688,187 |
| `copdem-glo-90-n10-e114` | 1200 × 1200 | 10–11°N, 114–115°E | `d9d5226a8b96a724490fb9cf1f8968aea1b5e0ed540b3627a39856833d65f62a` | 51,071 |
| `copdem-glo-90-n10-e115` | 1200 × 1200 | 10–11°N, 115–116°E | `c1d0a2046e0fd0121eb2576eda7f8371e8acd8b2e17789b40684e215696faf87` | 50,179 |
| `copdem-glo-90-n11-w017` | 1200 × 1200 | 11–12°N, 16–17°W | `5a8aeec28b2edaa99dd2593b7e173d2a7b12b90d261189ef60a6df748af3a322` | 854,909 |
| `copdem-glo-90-n11-w016` | 1200 × 1200 | 11–12°N, 15–16°W | `aea0e8c9d4126d6f4f1eb99ea6a9daade4f259bc2c9ecefe11f97c0b9bc99715` | 3,344,523 |
| `copdem-glo-90-n11-w015` | 1200 × 1200 | 11–12°N, 14–15°W | `d73fa8dd806edbb8e52e39cfdb7034663838aef470edf54b8d6357329d3834ae` | 5,805,373 |
| `copdem-glo-90-n11-w014` | 1200 × 1200 | 11–12°N, 13–14°W | `c6c8273de1d58d7ee3b832da4aae3a5f69d3e10012ed7a9f687cd7e474a5e9fe` | 5,646,734 |
| `copdem-glo-90-n11-w001` | 1200 × 1200 | 11–12°N, 0–1°W | `d8e47ed99cea486ac115a95704e9fd44a3077665f151deda3926a10fde2a2df4` | 4,826,032 |
| `copdem-glo-90-n11-e000` | 1200 × 1200 | 11–12°N, 0–1°E | `3a6f2a1c0d6981407720acbbe2bec224d2348a14437abdbcd21e156416836777` | 4,809,343 |
| `copdem-glo-90-n11-e001` | 1200 × 1200 | 11–12°N, 1–2°E | `530e1e624b5de13f2c215289556f7e365f7ef97527b351ba8f5dc4f30c726701` | 4,860,108 |
| `copdem-glo-90-n11-e002` | 1200 × 1200 | 11–12°N, 2–3°E | `9ffbfcfed243ca2a824cbad126bb12077be696432494e6388e100a4f195b9a5c` | 4,872,245 |
| `copdem-glo-90-n11-e003` | 1200 × 1200 | 11–12°N, 3–4°E | `0b735e5fa660be9f7afb15bd73ace3a1327ae818154d905ad2cba08aa761a950` | 4,895,292 |
| `copdem-glo-90-n11-e041` | 1200 × 1200 | 11–12°N, 41–42°E | `9d75f7816dccc3f94f31b2788fd309e28779e7ba6035a779cd1ad92943e138f6` | 4,784,673 |
| `copdem-glo-90-n11-e042` | 1200 × 1200 | 11–12°N, 42–43°E | `e08cc5d2fa2d5bc60320ead107a5d061cb86a9378e6bd4938ab48735aadd7fd3` | 4,991,561 |
| `copdem-glo-90-n11-e043` | 1200 × 1200 | 11–12°N, 43–44°E | `1c3fd6e8a46d74c67ac1f8f2cac8cd9d5808092bfa486fe4d30d925fe0bdbba9` | 1,673,460 |
| `copdem-glo-90-n11-e114` | 1200 × 1200 | 11–12°N, 114–115°E | `c78b8f588dcbcbafb319967008804baf2d22396d21b455d54f9c78cdacf5a266` | 51,252 |
| `copdem-glo-90-n11-e115` | 1200 × 1200 | 11–12°N, 115–116°E | `b17e7eed95f6e0b03de02ee14c350efbfe05a07402cee75d5ff172afeb3e4389` | 50,270 |
| `copdem-glo-90-n12-w071` | 1200 × 1200 | 12–13°N, 70–71°W | `b8c71678f05ba0556fa3827665db7fd5e9325452097a92875776bc2a07c9504d` | 284,026 |
| `copdem-glo-90-n12-w017` | 1200 × 1200 | 12–13°N, 16–17°W | `d2a856c01f61505c0d386104b626191de78560480a5be3b1f7692f4af072c2c7` | 3,932,711 |
| `copdem-glo-90-n12-w016` | 1200 × 1200 | 12–13°N, 15–16°W | `9468f35f78ba8090ac0cd82216b51d096e5305fcc9226408fad41170966a8ab3` | 5,805,118 |
| `copdem-glo-90-n12-w015` | 1200 × 1200 | 12–13°N, 14–15°W | `5a06a82b24d7adc5aaf0d86ab56dbf182a604656a14e406086ec7230834b99fd` | 5,720,321 |
| `copdem-glo-90-n12-w014` | 1200 × 1200 | 12–13°N, 13–14°W | `3cbcf7e43e2ccd6f3d3c4ef3a7b24180907c039287dc8ebbf18bff63ae5fce74` | 5,545,913 |
| `copdem-glo-90-n12-e002` | 1200 × 1200 | 12–13°N, 2–3°E | `24461904ab01fc6c0dae0cd4392807a9aa95e7548d484425791808def4cc08c8` | 4,877,697 |
| `copdem-glo-90-n12-e003` | 1200 × 1200 | 12–13°N, 3–4°E | `d4c6ad97e72008f818ed0b7d238fc682d470ff2359e560fea42a49d8d6c9d6bc` | 4,803,329 |
| `copdem-glo-90-n12-e042` | 1200 × 1200 | 12–13°N, 42–43°E | `ec1f40d44955db378dfd40cdbd3655e3b4fee47adb288489455e91c8b1be4a3b` | 5,035,959 |
| `copdem-glo-90-n12-e043` | 1200 × 1200 | 12–13°N, 43–44°E | `4252e10619b5dbb70f393ac47a6dd55a5baa3d75734d548e387e3336c2036eb6` | 2,428,807 |
| `copdem-glo-90-n13-w092` | 1200 × 1200 | 13–14°N, 91–92°W | `24d43f7358c90535cc24f95164d5e2d9bd621b5b7d003c2dd44a396b842a71fa` | 208,684 |
| `copdem-glo-90-n13-e144` | 1200 × 1200 | 13–14°N, 144–145°E | `39abfd4d4f2ec543b0d4d842e372851f8660733b998eb25e3a9f0f86c828e6f0` | 351,318 |
| `copdem-glo-90-n14-w093` | 1200 × 1200 | 14–15°N, 92–93°W | `b2aa9fd4d024b7248f52be6e9b204c78b604830434b60f4f738965c5f208bdc8` | 1,470,896 |
| `copdem-glo-90-n14-w092` | 1200 × 1200 | 14–15°N, 91–92°W | `db796825f1fdd380b78d358737a0e1efafe04b384a3e163dcb5c6f4617639d8b` | 4,942,678 |
| `copdem-glo-90-n14-w091` | 1200 × 1200 | 14–15°N, 90–91°W | `2d784b26b039cfcf1403c91d8b382b42b9a8478224a3f61df132b4f7e2f279d8` | 5,388,336 |
| `copdem-glo-90-n14-w025` | 1200 × 1200 | 14–15°N, 24–25°W | `39398a5ae2a3e0bc1149830d12075c7aa101149e37b087254b40e6776e66a9d5` | 293,667 |
| `copdem-glo-90-n14-w024` | 1200 × 1200 | 14–15°N, 23–24°W | `a2f1273a488084b625b8e62c6f8881ea23ef0ffddd59b9d32e8a52e5ee8f790b` | 181,159 |
| `copdem-glo-90-n14-e145` | 1200 × 1200 | 14–15°N, 145–146°E | `ea2db2762a1e9570ee0ce8c102591740443b08c3bb2fbcbcc48ccce7b1243dc6` | 129,293 |
| `copdem-glo-90-n15-w093` | 1200 × 1200 | 15–16°N, 92–93°W | `2a1b2c4ac6b26e369f37606a722599f59efac45ad280a35309fcc5e4e916cebd` | 5,282,560 |
| `copdem-glo-90-n15-w092` | 1200 × 1200 | 15–16°N, 91–92°W | `29c46513e2a806211fc20f68346a79ad7e5159bc0e0073d0ce72e98dba914c1c` | 5,305,509 |
| `copdem-glo-90-n15-w091` | 1200 × 1200 | 15–16°N, 90–91°W | `a9aeb4896b8fbfed80aad470882cd4e806b11143710c8bf1fb473025a076cb33` | 5,493,642 |
| `copdem-glo-90-n15-w090` | 1200 × 1200 | 15–16°N, 89–90°W | `cfe4c60d34498eee21da217d36b7cd346c301ffb6fd47850d17c27e592a34835` | 5,476,425 |
| `copdem-glo-90-n15-w089` | 1200 × 1200 | 15–16°N, 88–89°W | `c38ff1336d10ece09d6462eb9e87424013652c4249651a3c62e06cca1c4097ce` | 4,716,475 |
| `copdem-glo-90-n15-w025` | 1200 × 1200 | 15–16°N, 24–25°W | `d53eca7f6fec7b84e4ecb1a4ccaaeb8b1f85af6fb6c4427d916f4fdef4236a88` | 85,645 |
| `copdem-glo-90-n15-w024` | 1200 × 1200 | 15–16°N, 23–24°W | `8702802d6b1f920ef07b6acb6e08b22bf48d7f5fe9663496c94464e016ee16fc` | 604,234 |
| `copdem-glo-90-n15-w023` | 1200 × 1200 | 15–16°N, 22–23°W | `d59e62055c5cdfcb4e40f16b9ba53bbdce7c367a0e9b747cc2ad96573fdf79eb` | 69,010 |
| `copdem-glo-90-n15-e111` | 1200 × 1200 | 15–16°N, 111–112°E | `1434b797457c672900e457bbcd1caeab62389b1a1dc449292d79741251bc1cad` | 51,143 |
| `copdem-glo-90-n15-e117` | 1200 × 1200 | 15–16°N, 117–118°E | `bf69fa24a7d5efc8ba99830272b3b3df241b88e429739ac28fc56cd7bd85ae31` | 49,894 |
| `copdem-glo-90-n15-e145` | 1200 × 1200 | 15–16°N, 145–146°E | `3b7c50b09b05e142ccc062a39e1db8218b923d28dcfc70ea884341e54532ae4a` | 162,861 |
| `copdem-glo-90-n16-w092` | 1200 × 1200 | 16–17°N, 91–92°W | `b0ed1c54763af59816a0ecb4c5cee4abd97f5d5869b2bb2a27003749076e2414` | 5,451,884 |
| `copdem-glo-90-n16-w091` | 1200 × 1200 | 16–17°N, 90–91°W | `a48d98a7776367d1f93f541d2de8c24aca084684152f0b1d2ad7454a4f66eea1` | 5,491,988 |
| `copdem-glo-90-n16-w090` | 1200 × 1200 | 16–17°N, 89–90°W | `419d6aeb5978c5eb0f224cd382a2d596bbe217ff863fa05eb56d817c6a197aa9` | 5,510,033 |
| `copdem-glo-90-n16-w089` | 1200 × 1200 | 16–17°N, 88–89°W | `1ed75a01891e3b78dfb42e581ee719301d295cbdb2182b7608597626d9fd1190` | 3,194,822 |
| `copdem-glo-90-n16-w088` | 1200 × 1200 | 16–17°N, 87–88°W | `88df2994af8740a07e550ba71510b3ec217af4dc3aeb15905109051493ed15c3` | 50,747 |
| `copdem-glo-90-n16-w026` | 1200 × 1200 | 16–17°N, 25–26°W | `205f0e3a4ca8ed90ac4fae3fbd1a9fd038111cffe479e67a4cb68f266fce7eb2` | 183,504 |
| `copdem-glo-90-n16-w025` | 1200 × 1200 | 16–17°N, 24–25°W | `f9f706686586e897f01599aa5ff30954aebf72926d4f8e804e7e970dfedeb0a6` | 369,496 |
| `copdem-glo-90-n16-w023` | 1200 × 1200 | 16–17°N, 22–23°W | `f2d7fae97e8fa8858abe6797dcff4ea4191b4079ee0d244bca119acbba9a4070` | 499,887 |
| `copdem-glo-90-n16-e111` | 1200 × 1200 | 16–17°N, 111–112°E | `4ca8ebb06068e7ffa0b1da0337e1041ef5ed3418c9ae5fa61138c7adc09648eb` | 53,002 |
| `copdem-glo-90-n16-e112` | 1200 × 1200 | 16–17°N, 112–113°E | `0b72ba5f6c6921fbec710ee9a785e2e9d44078cfc136a163f870db72c5f72dc7` | 56,523 |
| `copdem-glo-90-n16-e145` | 1200 × 1200 | 16–17°N, 145–146°E | `388b99a28dff6b0cb5a5df144f672b2c5f15a8ce0b0eec798ab9a8343790ee27` | 75,266 |
| `copdem-glo-90-n16-e146` | 1200 × 1200 | 16–17°N, 146–147°E | `38e8b3be442c356e1391e985446fae7f95fd4b3491a99f2090f94b819e1dc201` | 51,430 |
| `copdem-glo-90-n17-w092` | 1200 × 1200 | 17–18°N, 91–92°W | `2120ba2d16e8ae52edb28470d12ec9e905b719283bd8e9e9359ec4041800e8cc` | 5,673,556 |
| `copdem-glo-90-n17-w091` | 1200 × 1200 | 17–18°N, 90–91°W | `a7ab340b3938b932499c9ee936dd5995bd5d9a9ad2efe490dd4f2354a806eb49` | 5,491,902 |
| `copdem-glo-90-n17-w090` | 1200 × 1200 | 17–18°N, 89–90°W | `52fd256b7354ebcf5e648f96960cf52e3ffb0f61e76b3e79c50b4d5eb05a6af0` | 5,417,386 |
| `copdem-glo-90-n17-w089` | 1200 × 1200 | 17–18°N, 88–89°W | `237bb2f0b0d1cea4df16ed136d6e9ebf4f36f339f238c5fa864320adc3062775` | 4,519,054 |
| `copdem-glo-90-n17-w088` | 1200 × 1200 | 17–18°N, 87–88°W | `811666d61d5d954170c9302905db0b943398e21d5861648b29dcfc0cca5feff3` | 199,389 |
| `copdem-glo-90-n17-w078` | 1200 × 1200 | 17–18°N, 77–78°W | `84a95af905bedcebe3fae96f768473743196ddb59fc56550c301c3033e7855ee` | 824,780 |
| `copdem-glo-90-n17-w077` | 1200 × 1200 | 17–18°N, 76–77°W | `44da4ebbc4db5251d0a2c99188674a16ad98339de4e70d56d1ec49e0abf456fc` | 545,552 |
| `copdem-glo-90-n17-w076` | 1200 × 1200 | 17–18°N, 75–76°W | `f1d34d61fec58179dfadefe381a0817204a0072651841d42ac4ef283aa72e865` | 50,074 |
| `copdem-glo-90-n17-w072` | 1200 × 1200 | 17–18°N, 71–72°W | `6c5cfcbb7d9157f81445d52da872f718a55eacfdab561ce8ca997f5ae1a8122c` | 734,718 |
| `copdem-glo-90-n17-w026` | 1200 × 1200 | 17–18°N, 25–26°W | `e448867a3c3fed3456c670e589b7afc909cb78b26fe51b92a3353c35bb0a7909` | 353,363 |
| `copdem-glo-90-n17-w025` | 1200 × 1200 | 17–18°N, 24–25°W | `b722d959418c643c0a1ce21a3f6ae11b2eb8c451fdeaf1aa3093388944733b74` | 66,501 |
| `copdem-glo-90-n17-e145` | 1200 × 1200 | 17–18°N, 145–146°E | `e907a908e77714c9989ce5afafe3f611452329269bcc4c23d2a42b30b67670a9` | 63,371 |
| `copdem-glo-90-n18-w090` | 1200 × 1200 | 18–19°N, 89–90°W | `837695aab72b8f195033502f6aee525d394ba8c4d329b3e28c5078452d84798d` | 5,368,853 |
| `copdem-glo-90-n18-w089` | 1200 × 1200 | 18–19°N, 88–89°W | `1cba40181948963b9e40196e4afaf49567996ab4ac2733b4d288dc77ea4aa2c7` | 5,077,363 |
| `copdem-glo-90-n18-w088` | 1200 × 1200 | 18–19°N, 87–88°W | `0e6781e3d8547863944ff8c9d8a42f1ccffd630ec3df5d9c74714eb3569b473e` | 1,349,566 |
| `copdem-glo-90-n18-w079` | 1200 × 1200 | 18–19°N, 78–79°W | `9ad89300ba816497dc84ca4154b860fe096e382d63460c941e7b39bd71354d78` | 557,292 |
| `copdem-glo-90-n18-w078` | 1200 × 1200 | 18–19°N, 77–78°W | `603493e0fd38891c770f8fe3399f63eb899fc7fe2f5638763e93b3c65eb1bc42` | 2,726,060 |
| `copdem-glo-90-n18-w077` | 1200 × 1200 | 18–19°N, 76–77°W | `7e0c74a095ba7dd4b05b0ccb9f1cd53f7b3a5ca01f8d4a7275a2f4da97e692b1` | 1,154,956 |
| `copdem-glo-90-n18-w076` | 1200 × 1200 | 18–19°N, 75–76°W | `323ccd2c51ad95b98ddf08e2a1353153a58c8cb7309ec0ae699a3a1679c9dd5f` | 53,636 |
| `copdem-glo-90-n18-w075` | 1200 × 1200 | 18–19°N, 74–75°W | `6ba3250aae788fecba1bec82f67341f293f3413db572e86a96c08eea6265835c` | 1,039,269 |
| `copdem-glo-90-n18-w074` | 1200 × 1200 | 18–19°N, 73–74°W | `096f1d5a1d67951b1b312bd9dd5ff3b0920877853ef7ea389036779b6cac9516` | 2,299,157 |
| `copdem-glo-90-n18-w073` | 1200 × 1200 | 18–19°N, 72–73°W | `13a0046a75d1dec0430a4e6e9d1b6b4d610a1e924327153c1e4d546caf6eaa02` | 3,280,885 |
| `copdem-glo-90-n18-w072` | 1200 × 1200 | 18–19°N, 71–72°W | `3b6f182f89e3cd35fbb4b8ef77c1aa65cf06d3c747958c247e4f9086a53cdaaa` | 5,052,190 |
| `copdem-glo-90-n18-w071` | 1200 × 1200 | 18–19°N, 70–71°W | `ba903bed89899c634b60508df44d7ffeacc01947aaf54ff5535926523853ec60` | 4,053,952 |
| `copdem-glo-90-n18-w070` | 1200 × 1200 | 18–19°N, 69–70°W | `7590803b76164c9e02044bb2064dd5ffe281154f94ea183b1bcf91f5844c25b6` | 3,323,394 |
| `copdem-glo-90-n18-w069` | 1200 × 1200 | 18–19°N, 68–69°W | `6a3a594481fe43296702205dddafef661f630be0495fe4d32ab38fae426594ba` | 2,099,041 |
| `copdem-glo-90-n18-w064` | 1200 × 1200 | 18–19°N, 63–64°W | `bbb990b6b4216874eba66a3bfa66135940ad16c72fdc8969e1a29ec87b24cc29` | 152,328 |
| `copdem-glo-90-n18-w063` | 1200 × 1200 | 18–19°N, 62–63°W | `7119757b31da25841331e03ebe969430a2e2a192154ef9075f3ffbbf49265338` | 62,563 |
| `copdem-glo-90-n18-e145` | 1200 × 1200 | 18–19°N, 145–146°E | `1ac9c015bd23a8418e71d4d509aa77016e3bf15bba338631f6a34f7465f52f12` | 113,017 |
| `copdem-glo-90-n19-w074` | 1200 × 1200 | 19–20°N, 73–74°W | `b2b8d6d63b709617acaf880fe59d67cd5914ec53f897468b6d50e3044cb45985` | 708,307 |
| `copdem-glo-90-n19-w073` | 1200 × 1200 | 19–20°N, 72–73°W | `211bfb40c27e1cf60bec5d96ec23a59060820aa203481d5ee8f1c2968872340e` | 4,162,699 |
| `copdem-glo-90-n19-w072` | 1200 × 1200 | 19–20°N, 71–72°W | `c4c339d0b3c5aed2d237180cb199e3baf42f9dacea5eea283fc33ccc08ef4588` | 4,722,850 |
| `copdem-glo-90-n19-w071` | 1200 × 1200 | 19–20°N, 70–71°W | `e29e18bf58d81cb06bada0661f185f8af337938c1dc5d833843a2d5da0435c3f` | 4,328,993 |
| `copdem-glo-90-n19-w070` | 1200 × 1200 | 19–20°N, 69–70°W | `ab102b379e266c3e51870aed7d2cffc5f52f522390dbfaf7fda107788fc9a32a` | 1,526,760 |
| `copdem-glo-90-n19-w069` | 1200 × 1200 | 19–20°N, 68–69°W | `3288a3344fdc460ac2bb6f061253529487f0c3f403684b44880f7c1855086a32` | 66,727 |
| `copdem-glo-90-n19-e145` | 1200 × 1200 | 19–20°N, 145–146°E | `53656873658a237eac2468c9277d26de7fbd57990ffd28e6e6a975631aef06f6` | 56,307 |
| `copdem-glo-90-n20-w073` | 1200 × 1200 | 20–21°N, 72–73°W | `4c58f0b06c721231e038993dc27697fce1dba825b93741867c58b4aa117bcbce` | 145,426 |
| `copdem-glo-90-n20-e144` | 1200 × 1200 | 20–21°N, 144–145°E | `aa70ad80141a061ce9eb8ec18476d9e103a561f3a567736e1d8bf5e36d24b75e` | 52,237 |
| `copdem-glo-90-n20-e145` | 1200 × 1200 | 20–21°N, 145–146°E | `ee556600eba3f3d973cbc35a5ecc0475d704ea35d9658ac188d8f535137e58cb` | 53,125 |
| `copdem-glo-90-n21-w073` | 1200 × 1200 | 21–22°N, 72–73°W | `1d7a0ea7e3dcd779773b1a2281bf9157fcd3239f412edbc33fc9ee4f299c58b3` | 235,357 |
| `copdem-glo-90-n21-w072` | 1200 × 1200 | 21–22°N, 71–72°W | `1b191213356071f2477c28b4fa6e84b42ab45e734f1f803cf77eb21c8ccf96a5` | 488,897 |
| `copdem-glo-90-n21-e033` | 1200 × 1200 | 21–22°N, 33–34°E | `6b44e798c3eafdc71fe6b3951b4817b4e2f8fdd80bc69b0bdbbe2982510b62f2` | 5,030,857 |
| `copdem-glo-90-n21-e034` | 1200 × 1200 | 21–22°N, 34–35°E | `f6876b29cd30a7d1a78c1a92e3850a519fe7cc9f0804b50350c8036f420edde8` | 5,233,764 |
| `copdem-glo-90-n21-e035` | 1200 × 1200 | 21–22°N, 35–36°E | `ac76435b221e4dd671d0a88e46ead9b84cb76ff63517bc53b97651c907aee5b7` | 5,159,068 |
| `copdem-glo-90-n21-e036` | 1200 × 1200 | 21–22°N, 36–37°E | `5052f71d0513ad72de5aaf8341cb246ca77c00357bd79c3ba53d656098137f8c` | 5,224,485 |
| `copdem-glo-90-n22-e034` | 1200 × 1200 | 22–23°N, 34–35°E | `bad5b7b28091e5c0185d5b38780dcdc62964cdce26bc35b9a7245c49d773d15c` | 5,092,929 |
| `copdem-glo-90-n22-e035` | 1200 × 1200 | 22–23°N, 35–36°E | `b982cd62e78e6caeca4055d4114e004c6cd4072834ec53844603d56df8dfe28a` | 4,941,841 |
| `copdem-glo-90-n22-e036` | 1200 × 1200 | 22–23°N, 36–37°E | `ddc677c050c35608a57a98a44048f4ae36369a970cad26e642dfc2cd0514c8e3` | 1,999,178 |
| `copdem-glo-90-n22-e052` | 1200 × 1200 | 22–23°N, 52–53°E | `5df76c8a85315c51254063de56374c3fd5198b388f77ea7a0ff85dd3616c5076` | 5,568,840 |
| `copdem-glo-90-n22-e053` | 1200 × 1200 | 22–23°N, 53–54°E | `d4b166e7dd75db7609c49994e5e699bfdd1ffce2a7053303ce8641620c7f4f54` | 5,766,638 |
| `copdem-glo-90-n22-e054` | 1200 × 1200 | 22–23°N, 54–55°E | `ba3c09cd4b8dae490302057f3449d5df01725018b537353d13069d47a480100f` | 5,656,377 |
| `copdem-glo-90-n22-e055` | 1200 × 1200 | 22–23°N, 55–56°E | `af1a1ec06cf87788885c72233d86a772f3bda3f75ef0b2b779074ab131e315dd` | 4,958,977 |
| `copdem-glo-90-n23-e035` | 1200 × 1200 | 23–24°N, 35–36°E | `1ab63c678a4f426d9249068ec725f0487531aec40bd84a0b1ea8b97b345f289c` | 3,165,393 |
| `copdem-glo-90-n23-e051` | 1200 × 1200 | 23–24°N, 51–52°E | `eb8f27697472c37b578e04dc29d0889d43612e9617eb913c067276c8ce2ee01e` | 5,373,087 |
| `copdem-glo-90-n23-e052` | 1200 × 1200 | 23–24°N, 52–53°E | `5e9899a23f7c5f678497821684f4cf27cc55ca4553b69c3890ee162484eeadc3` | 5,443,193 |
| `copdem-glo-90-n23-e053` | 1200 × 1200 | 23–24°N, 53–54°E | `174bb8d89e2d9729b175b06b123ad5458186f2574a118de9c9cbdb9e01107b48` | 5,646,288 |
| `copdem-glo-90-n23-e054` | 1200 × 1200 | 23–24°N, 54–55°E | `c69cbd87d25892f8c6f7ae84747d2cca8dc5486fb86aeadf074eb752c9562c13` | 5,606,619 |
| `copdem-glo-90-n23-e055` | 1200 × 1200 | 23–24°N, 55–56°E | `50b4eb036ac920739c5b472c0727db18c115a1582a750b739d4a195707d87cf6` | 4,979,601 |
| `copdem-glo-90-n24-e050` | 1200 × 1200 | 24–25°N, 50–51°E | `1075ced3f111a1560975c58374c2d826f53934587d58377e017660f1b33f9c66` | 5,115,125 |
| `copdem-glo-90-n24-e051` | 1200 × 1200 | 24–25°N, 51–52°E | `b6456d54f20135a2bcd6ee4a0b00d56636b1b176a74990543a2f4fbc6f7c3e7a` | 3,207,928 |
| `copdem-glo-90-n24-e052` | 1200 × 1200 | 24–25°N, 52–53°E | `11023d2b1fd75d51eb532d606e30f64bab3aab56f217978bfc3ba48a138c5130` | 664,079 |
| `copdem-glo-90-n24-e053` | 1200 × 1200 | 24–25°N, 53–54°E | `db9940448bfdab27764c573b5a6e0aeb443eaacfd2a6a650c8a444bc2222d796` | 897,737 |
| `copdem-glo-90-n24-e054` | 1200 × 1200 | 24–25°N, 54–55°E | `d26802db4dc3cc31a69cc9b17ac7a1fdf46244ff0cbccd90618c6c1b1d1ab2c9` | 3,287,894 |
| `copdem-glo-90-n24-e055` | 1200 × 1200 | 24–25°N, 55–56°E | `645f4d6002de1dcfe8e4b5a293927760f9d67d7fe7fa5b2fbc9e678750446bd9` | 5,076,829 |
| `copdem-glo-90-n24-e056` | 1200 × 1200 | 24–25°N, 56–57°E | `e7d96dd2ae8279246122b908dc0b7c4cd1dfd5779e92119d8e009de4739713dd` | 3,703,497 |
| `copdem-glo-90-n25-e051` | 1200 × 1200 | 25–26°N, 51–52°E | `78e44feab1a915b487a25cbb5c20e57c42234f8c4a670bde6bad0495906c469f` | 3,229,231 |
| `copdem-glo-90-n25-e052` | 1200 × 1200 | 25–26°N, 52–53°E | `7108808479f0c9c413fd1fd5bb6458399a5e2334ade15f8e2423ec71a7eb87a2` | 56,889 |
| `copdem-glo-90-n25-e054` | 1200 × 1200 | 25–26°N, 54–55°E | `c87cce2da06ae043f48ae2d6ad7dd7e91aaed280fdf243f0f86e9090b20e8ac1` | 80,857 |
| `copdem-glo-90-n25-e055` | 1200 × 1200 | 25–26°N, 55–56°E | `bf5a4be751400f6d610de649369c663065135d7b04e59ae0b7ace2a4b2e062c3` | 2,622,891 |
| `copdem-glo-90-n25-e056` | 1200 × 1200 | 25–26°N, 56–57°E | `edadb1e812aeeb11d12e8a37ecd9befce27bfd440249d5bb9a391b6a8bd5392a` | 2,159,668 |
| `copdem-glo-90-n25-e123` | 1200 × 1200 | 25–26°N, 123–124°E | `772154ab6f38897f1fbd7b453323cad7923e24581dedbe9566c3a1357d92197e` | 55,661 |
| `copdem-glo-90-n25-e124` | 1200 × 1200 | 25–26°N, 124–125°E | `3565f9ec245bbc401eada2055350328ab6597e8a7b8ab0edc04d4ece53d02eb9` | 50,107 |
| `copdem-glo-90-n26-e051` | 1200 × 1200 | 26–27°N, 51–52°E | `2b1a672ea51ad53117ccb6d9844227c7a6a178db4ec70877d0bab01ba6b39142` | 282,744 |
| `copdem-glo-90-n26-e056` | 1200 × 1200 | 26–27°N, 56–57°E | `6d490882099afcd3e6f1b58978891e025f1becde414bb5d5b4758c9b0748814e` | 882,441 |
| `copdem-glo-90-n26-e093` | 1200 × 1200 | 26–27°N, 93–94°E | `0ce812ad3ce49560ee158e8f17792de3e3b66bfcfa80a482504a46f715c4b895` | 5,265,788 |
| `copdem-glo-90-n27-e093` | 1200 × 1200 | 27–28°N, 93–94°E | `4458995d5d0ed9042b8d37d6a83a70f6f1b2d417642bc4f6d3becfe8278d16c1` | 5,479,084 |
| `copdem-glo-90-n27-e094` | 1200 × 1200 | 27–28°N, 94–95°E | `38faa6db7ca92de1fe7718bd10e50eea81b81bcd1ae167d8670b34319e9c12a2` | 5,195,583 |
| `copdem-glo-90-n27-e095` | 1200 × 1200 | 27–28°N, 95–96°E | `aa766886e1e3228c9bbed1af11707341eebc7e994becde6f36f8f81116c941bf` | 5,146,623 |
| `copdem-glo-90-n27-e096` | 1200 × 1200 | 27–28°N, 96–97°E | `2814696a853d560e7a2df708d78ea60adf226061e28085baf8bec90a02ade4d6` | 5,437,179 |
| `copdem-glo-90-n27-e097` | 1200 × 1200 | 27–28°N, 97–98°E | `2f6be6d9289fd26af878bd0c66ed3825a6c082fb6efa7a50af8054769ee2b20b` | 5,462,280 |
| `copdem-glo-90-n28-e047` | 1200 × 1200 | 28–29°N, 47–48°E | `e5cd08c0e979cb0732c6cf7a949feb525895217da27a31f640d4dbbb6b1d2595` | 4,502,081 |
| `copdem-glo-90-n28-e048` | 1200 × 1200 | 28–29°N, 48–49°E | `d8e3c0503bd190d4ff94bef3d3f55115da63443f65bff3c8ce37316bd614abf9` | 2,518,649 |
| `copdem-glo-90-n28-e092` | 1200 × 1200 | 28–29°N, 92–93°E | `1b27f58298649fcef4109e39fe4ad7bdf12d3a0d35de7697986ea0943c8085ec` | 4,814,343 |
| `copdem-glo-90-n28-e093` | 1200 × 1200 | 28–29°N, 93–94°E | `63ae8794bb6fbd09fb718fd0ea4d27c451647a72a0c2965e883a109da34a159d` | 5,174,234 |
| `copdem-glo-90-n28-e094` | 1200 × 1200 | 28–29°N, 94–95°E | `9d847ab53447d6f8907f8fe81cd9b7f55394dd4aae1050c78684c24ee2675b77` | 5,481,383 |
| `copdem-glo-90-n28-e095` | 1200 × 1200 | 28–29°N, 95–96°E | `7d281b19de78c6fdf196d77fd679b86b251aa219e9ce73e82798b2389461c053` | 5,410,557 |
| `copdem-glo-90-n28-e096` | 1200 × 1200 | 28–29°N, 96–97°E | `f68d87b174292325863acc18c718ee7e248e53c3da5e884650560f789473251a` | 5,229,090 |
| `copdem-glo-90-n28-e097` | 1200 × 1200 | 28–29°N, 97–98°E | `bb9ff5697cfe5b78cb000f90ce73dc7ec201093a5225c354f58372c92a4ea434` | 5,064,578 |
| `copdem-glo-90-n29-e034` | 1200 × 1200 | 29–30°N, 34–35°E | `fc87d7f0c5e891b75d48cf855a076499b260020272fac0f405decb6a3586bede` | 4,733,394 |
| `copdem-glo-90-n29-e035` | 1200 × 1200 | 29–30°N, 35–36°E | `f95d5228292fafec49acd7c6c70a3893eec96d613ce6bf23e75f84e3be2b07de` | 5,088,996 |
| `copdem-glo-90-n29-e036` | 1200 × 1200 | 29–30°N, 36–37°E | `ca380326cd13d503e2e8c6442969878b162bb9744081992b581c23c222d39fb8` | 4,614,942 |
| `copdem-glo-90-n29-e037` | 1200 × 1200 | 29–30°N, 37–38°E | `e6e266e7eb84aa57a0e3358a519fc594086f8b59dd71eb83199852d612c29467` | 4,789,086 |
| `copdem-glo-90-n29-e046` | 1200 × 1200 | 29–30°N, 46–47°E | `0b5cf8662a6d8ddc45322ed4c267b6a058c9f865f1aa3d9ea64e790159c4745a` | 4,461,146 |
| `copdem-glo-90-n29-e047` | 1200 × 1200 | 29–30°N, 47–48°E | `dd50fd6c014ca9c1378ce64e772289911b0cf9c49580c764497d80b5a4c6544d` | 4,784,225 |
| `copdem-glo-90-n29-e048` | 1200 × 1200 | 29–30°N, 48–49°E | `986094217cf00e49dd529134c9e8b3cca13a3ba4ee45b1833e89d76db54b1545` | 1,218,859 |
| `copdem-glo-90-n29-e094` | 1200 × 1200 | 29–30°N, 94–95°E | `c2768dbfe1cbe3d503fd902c6a14d53f9912990ede313dc97baf6b77a9a73bee` | 4,966,959 |
| `copdem-glo-90-n29-e095` | 1200 × 1200 | 29–30°N, 95–96°E | `11122486b316a9f6f6b8ff3dde64a7364a38a651fd2622018deb0501b2781bb6` | 5,191,971 |
| `copdem-glo-90-n29-e096` | 1200 × 1200 | 29–30°N, 96–97°E | `591ae3015ce7edcf8217920fcfd9bd0677f4c8564fa611729b3e26c15feca8c9` | 4,918,512 |
| `copdem-glo-90-n30-e034` | 1200 × 1200 | 30–31°N, 34–35°E | `506968b0454a2c9f48fcdec200b45e5af7036c7ed210e78298cf7b32f00ac4e4` | 5,143,778 |
| `copdem-glo-90-n30-e035` | 1200 × 1200 | 30–31°N, 35–36°E | `e299bf7c3f36488bde30a4ffe5181760cee7320c9edc7607a96ab46583517d57` | 5,170,789 |
| `copdem-glo-90-n30-e036` | 1200 × 1200 | 30–31°N, 36–37°E | `8d4ed3aaf2399b46152689f3c2814fd98a11331c6abcee04124bc9760c503e5f` | 4,409,336 |
| `copdem-glo-90-n30-e037` | 1200 × 1200 | 30–31°N, 37–38°E | `55aabbe9e2407a9e9a765e5e8a67b4312b8414d48dd198a3a0ae24459adcea35` | 4,493,597 |
| `copdem-glo-90-n30-e047` | 1200 × 1200 | 30–31°N, 47–48°E | `7cc65fe25032088a8d61e7c6cf8daf55188d725a672a58e28b2b80e529691a3f` | 5,522,768 |
| `copdem-glo-90-n30-e048` | 1200 × 1200 | 30–31°N, 48–49°E | `2b8a5991e712a4bca36837073b7466b01f4c1d5d5ae5eb29672cdf9037d83839` | 5,166,899 |
| `copdem-glo-90-n30-e079` | 1200 × 1200 | 30–31°N, 79–80°E | `e9c8f087f023625792ae3052c63c97462acb3382f70a22d54dcbb8fc669e0423` | 5,163,926 |
| `copdem-glo-90-n30-e080` | 1200 × 1200 | 30–31°N, 80–81°E | `4698d4a3eb459f10fef09c00c082be136cd82addc9070a6d76dee1ee223614d4` | 4,803,425 |
| `copdem-glo-90-n31-e034` | 1200 × 1200 | 31–32°N, 34–35°E | `3c4904fbe0511d1b9ed7a0d71fd1a5d48a35be9043cff26ad6f22e2a158e79c6` | 3,484,492 |
| `copdem-glo-90-n31-e035` | 1200 × 1200 | 31–32°N, 35–36°E | `d202d0901de8ec7afb971fcaa57a24cf696d2736a6e45d82ab601bd803e45032` | 5,065,285 |
| `copdem-glo-90-n31-e036` | 1200 × 1200 | 31–32°N, 36–37°E | `d2858eb2639d78d5ccea1e01f4436332317cf8d7a0be119fc257719a277ce10d` | 4,628,875 |
| `copdem-glo-90-n31-e037` | 1200 × 1200 | 31–32°N, 37–38°E | `709628a0319678df17e38cee97b109f3bf931635c291ad94f16706f97306d975` | 4,582,767 |
| `copdem-glo-90-n31-e038` | 1200 × 1200 | 31–32°N, 38–39°E | `4edb063f944bd4cd2fddcafa0f2e4b03150b8d72b2fb0624e1f061271e94498f` | 4,474,094 |
| `copdem-glo-90-n31-e039` | 1200 × 1200 | 31–32°N, 39–40°E | `764d4b38ea9690ea0404f689e8e475315c86ce376dfb8d523de57cd89b27615b` | 4,248,449 |
| `copdem-glo-90-n31-e078` | 1200 × 1200 | 31–32°N, 78–79°E | `dd7888b8885ba6940383eb96fd856bd12c89baa7adb665c111bd5e68648d4eea` | 4,992,262 |
| `copdem-glo-90-n31-e079` | 1200 × 1200 | 31–32°N, 79–80°E | `3af64424c889c19d79814e348c4cafbb4e26055f92da490b61138b7b8d2e3e59` | 4,746,211 |
| `copdem-glo-90-n32-w065` | 1200 × 1200 | 32–33°N, 64–65°W | `ff784144a7e65b9e9a7c983ed5574739d7b0663366b2e2645ae195f64bd21f1e` | 102,845 |
| `copdem-glo-90-n32-e034` | 1200 × 1200 | 32–33°N, 34–35°E | `e03e3ebdc4997fb5e9ec363d1c010580cbabf0e9656e8ae8aba1fd616f677360` | 763,775 |
| `copdem-glo-90-n32-e035` | 1200 × 1200 | 32–33°N, 35–36°E | `73b9024a66a513871e3c0a7a1a8012311269b9fc89c6029475a1dc5a44444937` | 5,419,070 |
| `copdem-glo-90-n32-e036` | 1200 × 1200 | 32–33°N, 36–37°E | `9bd84bafa65a3c2ca66698e6bd6fcd78e140a1779dab4a698961f216dea824e7` | 4,631,025 |
| `copdem-glo-90-n32-e037` | 1200 × 1200 | 32–33°N, 37–38°E | `14634b9de72166969fa349b3869239ed6b12f8baa2ee3c462587767dce8f97fa` | 4,504,887 |
| `copdem-glo-90-n32-e038` | 1200 × 1200 | 32–33°N, 38–39°E | `d3f8f57ac1eece7f6b3f9790f10b99eb094e8ec5a0acc67898a0592b109d9198` | 4,265,926 |
| `copdem-glo-90-n32-e039` | 1200 × 1200 | 32–33°N, 39–40°E | `3d306e84cd93bd6431a05750f1ba9ff2a8b0f7ee590e775d5f0696b532729df5` | 4,274,326 |
| `copdem-glo-90-n32-e078` | 1200 × 1200 | 32–33°N, 78–79°E | `1fadd2d95e440d3e6814b7b111e570eb7192846553e8ca1b2ca601d108a7635c` | 4,697,965 |
| `copdem-glo-90-n32-e079` | 1200 × 1200 | 32–33°N, 79–80°E | `4bf0de099dc1e02d523db64cd031cd092a82a8bb5e9f080bc630309b37958821` | 4,579,453 |
| `copdem-glo-90-n33-e035` | 1200 × 1200 | 33–34°N, 35–36°E | `470462fd940b0ab2c550484fc69d86c02d47137d8eba81d16277a5737dd24655` | 3,622,653 |
| `copdem-glo-90-n33-e036` | 1200 × 1200 | 33–34°N, 36–37°E | `75f356623c143d7f2b06115add8009f8a8dd20c46d726ff88c39ab4c793e4c40` | 4,686,050 |
| `copdem-glo-90-n33-e038` | 1200 × 1200 | 33–34°N, 38–39°E | `88e4f0ea70502a4576ab8ce89320613369a7266c916646c7577b41200eecca61` | 4,282,401 |
| `copdem-glo-90-n33-e078` | 1200 × 1200 | 33–34°N, 78–79°E | `627b6e006d481a3522643764886438b70a3a28e5177dd1deeb967ae4e126a554` | 4,538,335 |
| `copdem-glo-90-n33-e079` | 1200 × 1200 | 33–34°N, 79–80°E | `97f19aad0eaa4a0099cf6936ffd38f6efb2e0b989e845679b45fab6e9d764d58` | 4,501,275 |
| `copdem-glo-90-n34-e035` | 1200 × 1200 | 34–35°N, 35–36°E | `3a2baabede76b17a7b25aab85e1f18a416e5d0217b2349f21cc6f8bed2a725cd` | 1,128,237 |
| `copdem-glo-90-n34-e036` | 1200 × 1200 | 34–35°N, 36–37°E | `a5c6d1e657b5dc4da9a301d4a1e5b15a5c357aa5b1b91358573d8b20c277ac6f` | 5,009,590 |
| `copdem-glo-90-n34-e078` | 1200 × 1200 | 34–35°N, 78–79°E | `387e90ea00ab80d7a932370931b00960490b5fa32336ccb23c127453edbb69e8` | 4,730,763 |
| `copdem-glo-90-n34-e079` | 1200 × 1200 | 34–35°N, 79–80°E | `a4aa3b6b12087aaf3c24830cd2d589450e78cb84755b45ca7e5b00a351c48315` | 4,267,282 |
| `copdem-glo-90-n34-e080` | 1200 × 1200 | 34–35°N, 80–81°E | `c66ce0f19dd662b355ef78b48d6978aef04a51f8b1c2c9f5644938ffcfe0dcac` | 4,353,205 |
| `copdem-glo-90-n35-e077` | 1200 × 1200 | 35–36°N, 77–78°E | `2e3867da7cdbfbc0cbf78daf395187ef5cfa14d2df0cf8b5c2d02d3f4054a3e0` | 4,732,508 |
| `copdem-glo-90-n35-e078` | 1200 × 1200 | 35–36°N, 78–79°E | `0b9ddb2ed25f8aff9871e7815572895aa1884f368ff136a91ad0941d29e88d23` | 4,590,833 |
| `copdem-glo-90-n35-e079` | 1200 × 1200 | 35–36°N, 79–80°E | `e2f0077584aa06434cbc1da447c3463e2953242137d7011f18894d3d67b649ae` | 4,366,530 |
| `copdem-glo-90-n35-e080` | 1200 × 1200 | 35–36°N, 80–81°E | `cfbcb21d174df7c0e0cf0556364e181428a46608e981b75b4a0b6c7acd2cb4aa` | 4,538,915 |
| `copdem-glo-90-n36-w006` | 1200 × 1200 | 36–37°N, 5–6°W | `83645e21606c01f5ca985158db9f18175d6fb81242f28cc3ef75455edcfa8608` | 4,634,480 |
| `copdem-glo-90-n37-e131` | 1200 × 1200 | 37–38°N, 131–132°E | `8e84ee890b6c1ad1af7fd9141d754e833e69f5c43ccc86021a1d35d4b98964da` | 50,348 |
| `copdem-glo-90-n38-e045` | 1200 × 1200 | 38–39°N, 45–46°E | `af31bf3426347b193e8a1950a8359491b8f1c317b6592cd54ad3bf21c5054194` | 4,535,039 |
| `copdem-glo-90-n38-e046` | 1200 × 1200 | 38–39°N, 46–47°E | `da3e0f24ab2291668b41b7b2d605e9a972fd64af512f7fb6064976491b865dac` | 5,067,628 |
| `copdem-glo-90-n38-e048` | 1200 × 1200 | 38–39°N, 48–49°E | `f9d106b51edf368550cee0712329e5990fdcf953b0f337c6366c4b7d1b0706cb` | 4,600,824 |
| `copdem-glo-90-n39-e019` | 1200 × 1200 | 39–40°N, 19–20°E | `bae57a760977b54eccc41e34a4a3766f009993c67c0ece47d6eef37040c78e26` | 511,084 |
| `copdem-glo-90-n39-e020` | 1200 × 1200 | 39–40°N, 20–21°E | `108d87a9158c9da3a0028ed6769b84e274b643f6252ccbf41100c01103304120` | 4,282,829 |
| `copdem-glo-90-n39-e044` | 1200 × 1200 | 39–40°N, 44–45°E | `b595d44ba6b0ae3f90933037f5fde9c5cc064e582a66bd9afdbc2568c15892ff` | 4,798,381 |
| `copdem-glo-90-n39-e045` | 1200 × 1200 | 39–40°N, 45–46°E | `d2679d210183e2ad5832072c7cec7215f22db778f17e5e95404d8fd65db6b0c7` | 4,945,380 |
| `copdem-glo-90-n39-e046` | 1200 × 1200 | 39–40°N, 46–47°E | `69856fe6ac02b8e4feb6fe3e5dd5f0dfc354607eaebd9c80662f047529fff3ed` | 5,224,435 |
| `copdem-glo-90-n39-e047` | 1200 × 1200 | 39–40°N, 47–48°E | `f9a0169d189955caba49c524ff0d239f2d9967322d11cc92a4d39d570d0869c5` | 5,328,361 |
| `copdem-glo-90-n39-e048` | 1200 × 1200 | 39–40°N, 48–49°E | `ef7ba76f8850d1ac2a71e9e96455daec9dd8e1e9e594213a1a5a6e709d90eb76` | 5,396,208 |
| `copdem-glo-90-n39-e049` | 1200 × 1200 | 39–40°N, 49–50°E | `c33372a8fc268848e60278ab676091fe1769d0648ea3480685e0ae59ab3fc4ff` | 1,640,528 |
| `copdem-glo-90-n40-e019` | 1200 × 1200 | 40–41°N, 19–20°E | `f082896b86ec7fe4917578c6e91d263fcd490263f2c0dee2c0d59d7f8dcd0c5a` | 3,155,651 |
| `copdem-glo-90-n40-e020` | 1200 × 1200 | 40–41°N, 20–21°E | `2374405be73fa7f40f3e8bc42a7b251849fe1500018af21aea7254eece1d7fdf` | 5,272,913 |
| `copdem-glo-90-n40-e021` | 1200 × 1200 | 40–41°N, 21–22°E | `eac82565afdbe6ff2bf6f9caf276168c6da67cfc06d7157194ea96b9ffab85e9` | 5,023,019 |
| `copdem-glo-90-n40-e043` | 1200 × 1200 | 40–41°N, 43–44°E | `528af41185c235fc0777187d01b669ab046378a824bbf528a1652308d6621fa0` | 4,706,941 |
| `copdem-glo-90-n40-e044` | 1200 × 1200 | 40–41°N, 44–45°E | `2ab39d0ebb9801ddfd836831e112639ca6567b1cb4d9400e5b3a635f722cef84` | 4,849,296 |
| `copdem-glo-90-n40-e045` | 1200 × 1200 | 40–41°N, 45–46°E | `89147da6fbd1dd2838262e301ff36eb3e066e04e6499b19dc7f75793f66714f5` | 4,407,283 |
| `copdem-glo-90-n40-e046` | 1200 × 1200 | 40–41°N, 46–47°E | `b36780b10c6868622c49f18001126428d0ccad6d758e732b70fc5ba38f20974c` | 4,934,270 |
| `copdem-glo-90-n40-e047` | 1200 × 1200 | 40–41°N, 47–48°E | `6eb3f21a1c5172ef978a1c75263ac3a672fd680581a032b87ca79f13023f0e06` | 5,420,862 |
| `copdem-glo-90-n40-e048` | 1200 × 1200 | 40–41°N, 48–49°E | `89bf786b2accce3e832f9545bccd3cb7856cef7b77c970c33ddc877ed6fab835` | 5,368,833 |
| `copdem-glo-90-n40-e049` | 1200 × 1200 | 40–41°N, 49–50°E | `8a91458d85a8154501f66db280a64d23a9a0b5b726a345d5a30daef78342edb7` | 3,526,223 |
| `copdem-glo-90-n40-e050` | 1200 × 1200 | 40–41°N, 50–51°E | `ae854d7fe28f6aca0b81a1231124d5eaf3527ef8c22f7d9c7c4761758c2dcd08` | 459,165 |
| `copdem-glo-90-n41-e012` | 1200 × 1200 | 41–42°N, 12–13°E | `6f3d675002c210376ecbff442c8f34dd9dc29d640c92d646640521394082efb0` | 2,292,316 |
| `copdem-glo-90-n41-e019` | 1200 × 1200 | 41–42°N, 19–20°E | `dd5c6573613706a0453c40558d2d249739f9b66111028558001e52ce1e8a9cc6` | 3,240,529 |
| `copdem-glo-90-n41-e020` | 1200 × 1200 | 41–42°N, 20–21°E | `507aa9a7a00337fcaf6ee77ba1f6dd6acee5eab607b62c397c68be274429c342` | 5,210,728 |
| `copdem-glo-90-n41-e021` | 1200 × 1200 | 41–42°N, 21–22°E | `d8372f53d787fe677398c86f8c36db1ba9fd2a70abfb8b5e4a66c4ca5974ce69` | 5,271,722 |
| `copdem-glo-90-n41-e022` | 1200 × 1200 | 41–42°N, 22–23°E | `5692b2c74afbb15d06d23fa78de757bf4c2a00159087b4b14779ff1102909659` | 5,418,575 |
| `copdem-glo-90-n41-e023` | 1200 × 1200 | 41–42°N, 23–24°E | `b5dfa2e5ed2ef0494605dbb68f4928679a3ffc9027b6d5c247552b3de7ee6269` | 5,382,675 |
| `copdem-glo-90-n41-e024` | 1200 × 1200 | 41–42°N, 24–25°E | `a0fc646494d97f5353858136448fb0d951f6471befbc3ab2964387eb6d5c6070` | 5,385,595 |
| `copdem-glo-90-n41-e025` | 1200 × 1200 | 41–42°N, 25–26°E | `1b05d61196c1eecf2933e8e97783d2c074141d366035c4ef594f9d2475f2a389` | 5,550,576 |
| `copdem-glo-90-n41-e026` | 1200 × 1200 | 41–42°N, 26–27°E | `303e1174eb301afdfb380e1a2900c8e9a9f4722848d05d72efb110212af577a3` | 5,620,042 |
| `copdem-glo-90-n41-e027` | 1200 × 1200 | 41–42°N, 27–28°E | `adb4c6b782290888d744a9d45b62e8abbcf11e7b11955e1f318adf01cc40bf43` | 5,463,708 |
| `copdem-glo-90-n41-e028` | 1200 × 1200 | 41–42°N, 28–29°E | `c34a04069b595811fef9393331dc8e742af7a24d6cd56ec7b2aaebbe24119fd4` | 2,430,196 |
| `copdem-glo-90-n41-e041` | 1200 × 1200 | 41–42°N, 41–42°E | `23effeb816f3e40fd3ca5acadbdfec01fe84293810903c377421730ba51d3bf4` | 3,243,769 |
| `copdem-glo-90-n41-e042` | 1200 × 1200 | 41–42°N, 42–43°E | `9248fc8df9103568a4c230e4b0d8f098901ea0012d216267270e77f1d95d37d9` | 5,193,020 |
| `copdem-glo-90-n41-e043` | 1200 × 1200 | 41–42°N, 43–44°E | `676af62030d22112e1cc5e44aaa61935b4b44f6d91fd72ffa76281a83a0bfdc0` | 4,825,701 |
| `copdem-glo-90-n41-e044` | 1200 × 1200 | 41–42°N, 44–45°E | `e97ed35b1c879eb54a13810a6254a69d1b8969175db31fd16fdc29dc3c01a3f1` | 5,044,573 |
| `copdem-glo-90-n41-e045` | 1200 × 1200 | 41–42°N, 45–46°E | `2356822abb8c3af3c44cb717880b6693ec5256e771d66176f16e8546d15e42d4` | 5,061,454 |
| `copdem-glo-90-n41-e046` | 1200 × 1200 | 41–42°N, 46–47°E | `a8cacb89bb6a3f19c88f77279ab7f88c73b742fcf6f0658e06bdcf5072952078` | 5,126,146 |
| `copdem-glo-90-n41-e047` | 1200 × 1200 | 41–42°N, 47–48°E | `4747ef013e15c86feb2e49cb17f80f8c13827d39a6b802dfd927e0ea1db7f9f4` | 5,176,926 |
| `copdem-glo-90-n41-e048` | 1200 × 1200 | 41–42°N, 48–49°E | `bbebc70f6499dec35d7b6cbf2eecbb75063ba24d0e2b2b547ea2ca162fe3d448` | 4,516,044 |
| `copdem-glo-90-n41-e049` | 1200 × 1200 | 41–42°N, 49–50°E | `6cc0804d8baeb57eb1d6482204e3b32056acd3ea5e765c08be278c5625ca7f29` | 430,768 |
| `copdem-glo-90-n42-e015` | 1200 × 1200 | 42–43°N, 15–16°E | `5eff509452b24444f825fa53dd53b079babb61b64c5b5d46ff7926e3909f4d2e` | 55,671 |
| `copdem-glo-90-n42-e016` | 1200 × 1200 | 42–43°N, 16–17°E | `2472c62da495bfe154b189d940ae371db27b084c319c1f1261f61806b624fad1` | 247,336 |
| `copdem-glo-90-n42-e017` | 1200 × 1200 | 42–43°N, 17–18°E | `435708b70685251296cdb9c013a205318d5dcb628f674895740f71bac08b335d` | 919,056 |
| `copdem-glo-90-n42-e018` | 1200 × 1200 | 42–43°N, 18–19°E | `bdca35026aea0314b1a13a880692deecc931419b5f194e8ed35d773947001520` | 3,053,267 |
| `copdem-glo-90-n42-e019` | 1200 × 1200 | 42–43°N, 19–20°E | `a87b010e5031000e54845fa561b5e700685a263654d7315faada267ddf9cf965` | 5,186,615 |
| `copdem-glo-90-n42-e020` | 1200 × 1200 | 42–43°N, 20–21°E | `bfd182b3fbbecf9b5b380c3d2b7aa92944f246b1102b7aa338841524492a0319` | 5,272,883 |
| `copdem-glo-90-n42-e021` | 1200 × 1200 | 42–43°N, 21–22°E | `e3a873239cb6862a9e5189ea11e71225eb680b168e8c6843982d65b6b4f06740` | 5,299,715 |
| `copdem-glo-90-n42-e022` | 1200 × 1200 | 42–43°N, 22–23°E | `6c018c9f1bd2ca811927e5489adfd91ec720f51c24926790f4c1e3bfe7068e57` | 5,292,764 |
| `copdem-glo-90-n42-e023` | 1200 × 1200 | 42–43°N, 23–24°E | `47cd6cfc6fe056f41312ee97688e561bced806e5f731553e223b7a3eb3218edf` | 5,148,230 |
| `copdem-glo-90-n42-e024` | 1200 × 1200 | 42–43°N, 24–25°E | `148793aab9d2003f62e333f335d7746f878079cebc2abd5543bf1d676ca8fa7f` | 5,168,748 |
| `copdem-glo-90-n42-e025` | 1200 × 1200 | 42–43°N, 25–26°E | `d7128857b6f0d60a7266bcc2e9ab089184cbc3a28b04117de7f1a6590730b11b` | 5,280,233 |
| `copdem-glo-90-n42-e026` | 1200 × 1200 | 42–43°N, 26–27°E | `cca9dd505e78a78f997ae33851f7f4f13d847d0ff4b65dbec81996398acc9752` | 5,271,202 |
| `copdem-glo-90-n42-e027` | 1200 × 1200 | 42–43°N, 27–28°E | `1c6ea3d1bf5c026a3395c7c3cc86caa0645a316de2dd0bbd7c0ab3685503e56b` | 4,457,741 |
| `copdem-glo-90-n42-e028` | 1200 × 1200 | 42–43°N, 28–29°E | `c5bcccd730128703a96ca0a21ace398d18914aff78b4f005ef47ae603d9531cc` | 56,024 |
| `copdem-glo-90-n42-e040` | 1200 × 1200 | 42–43°N, 40–41°E | `cc16529b1e1f9529c8cb7f3d1186cadc93b7b09e81deeed5f6837f4155d9c7f4` | 55,012 |
| `copdem-glo-90-n42-e041` | 1200 × 1200 | 42–43°N, 41–42°E | `9f7185e85ad8365367d524646f245a4d23eb5f33f85db1ec8ab23db0660e0dee` | 3,082,047 |
| `copdem-glo-90-n42-e042` | 1200 × 1200 | 42–43°N, 42–43°E | `3f9bd099c4b624bcf185ba76347cd73378174548040e783a7253add5547983d0` | 5,461,130 |
| `copdem-glo-90-n42-e043` | 1200 × 1200 | 42–43°N, 43–44°E | `e6d570d798f7f0e956709bb3f790afa21d3494c7167af653ae9eabf21019cd09` | 5,230,968 |
| `copdem-glo-90-n42-e044` | 1200 × 1200 | 42–43°N, 44–45°E | `5f3d179c48160d0b06b05dfd46b58da05e032ea09a6c34056a890415d3eeac72` | 5,167,435 |
| `copdem-glo-90-n42-e045` | 1200 × 1200 | 42–43°N, 45–46°E | `db6e817ccb5509310b6d0332737258ec4cecb0adcbd7d19d32a898f0c02b0655` | 5,259,225 |
| `copdem-glo-90-n42-e046` | 1200 × 1200 | 42–43°N, 46–47°E | `074e8522ec97004cd508dc4e568a21e8351310d60e0e1fcf66220546998b9f95` | 5,234,521 |
| `copdem-glo-90-n43-e007` | 1200 × 1200 | 43–44°N, 7–8°E | `9fb02abe4417bcf48ac96e8f537974beeb36b4a1cbd3ce04b681cd1b077b068d` | 1,585,352 |
| `copdem-glo-90-n43-e012` | 1200 × 1200 | 43–44°N, 12–13°E | `b173d23287389a8f5932cfa8fee4ec20a1f65dda5a5e2d6b2509db1dee8f649a` | 5,413,438 |
| `copdem-glo-90-n43-e015` | 1200 × 1200 | 43–44°N, 15–16°E | `d26f3c3a4cdeba90afb16c2d2f91303239c6dcf49b7cd552fa6d3d1fa96b74d9` | 1,089,707 |
| `copdem-glo-90-n43-e016` | 1200 × 1200 | 43–44°N, 16–17°E | `00e61d56f17deaa56bf7d603a2cce1eb1346d25b4f3c0e58ba955e02b23e902d` | 3,455,530 |
| `copdem-glo-90-n43-e017` | 1200 × 1200 | 43–44°N, 17–18°E | `f2aa16180a75f00701311d7d31038aaf6149e6ffa75779fda0a6297739a7f132` | 5,003,892 |
| `copdem-glo-90-n43-e018` | 1200 × 1200 | 43–44°N, 18–19°E | `20493efe8f15d589ab53bdcc41658c26f309577303a20075a468a818fd3ce065` | 5,252,305 |
| `copdem-glo-90-n43-e019` | 1200 × 1200 | 43–44°N, 19–20°E | `ddba5b9b2619455e620481e9829f476638f08098e694c8394477d33daeb2cca7` | 5,311,238 |
| `copdem-glo-90-n43-e020` | 1200 × 1200 | 43–44°N, 20–21°E | `412740a24ae546be66b83d12d3eb8886f50e9164ea7a92e2fb79a7526c92de51` | 5,394,499 |
| `copdem-glo-90-n43-e021` | 1200 × 1200 | 43–44°N, 21–22°E | `33430bbcdde0db4c160b8f3b0297d92db6af999b816c893586874d5f5d23b0b1` | 5,409,214 |
| `copdem-glo-90-n43-e022` | 1200 × 1200 | 43–44°N, 22–23°E | `bc29b3421c9561eb512b075f60ec139998fc58e0068a78406737a73e99f27020` | 5,380,328 |
| `copdem-glo-90-n43-e023` | 1200 × 1200 | 43–44°N, 23–24°E | `d016d864118f44fb1b45c9ceea15b8907a2ba8ba02fd1f90b990586a79fa7140` | 5,195,401 |
| `copdem-glo-90-n43-e024` | 1200 × 1200 | 43–44°N, 24–25°E | `134b1980d6fe8af3123348bc66a5eeaf6fdca5311957990fd831221ffd2669ce` | 5,179,038 |
| `copdem-glo-90-n43-e025` | 1200 × 1200 | 43–44°N, 25–26°E | `d846bedc0d582f7c7aa78422f677f9453b2060ad4b31214472b167c8ba8dc46f` | 5,291,165 |
| `copdem-glo-90-n43-e026` | 1200 × 1200 | 43–44°N, 26–27°E | `7f33dca02c435349e7c20617f47b0c95c20c28728bac661c32f907c73ad9b56d` | 5,331,127 |
| `copdem-glo-90-n43-e027` | 1200 × 1200 | 43–44°N, 27–28°E | `3eb7c5fab41857ba0b1a4345c1c4fdb9dc888d77d744e003ec4fd44e5ccc8863` | 5,251,878 |
| `copdem-glo-90-n43-e028` | 1200 × 1200 | 43–44°N, 28–29°E | `472bb56ae74fbaf9e53117ba3a440385786b8acfbd491621f1b7d7c36acdab98` | 1,995,519 |
| `copdem-glo-90-n43-e040` | 1200 × 1200 | 43–44°N, 40–41°E | `5322bfd8b53f2d420bb7eaa6f4f839ea50bacda219834e0a79f4c8535ad66ad7` | 4,530,580 |
| `copdem-glo-90-n43-e041` | 1200 × 1200 | 43–44°N, 41–42°E | `6f7a92d63989c49c96e23253b04812c746571032f3220d0c23b623b0634f171b` | 5,223,361 |
| `copdem-glo-90-n43-e042` | 1200 × 1200 | 43–44°N, 42–43°E | `db521e20dec0f96205de07bf355c9b7b0a51c31e96452d08f8d7a2d2a10ba4ae` | 5,118,150 |
| `copdem-glo-90-n43-e043` | 1200 × 1200 | 43–44°N, 43–44°E | `cb81da6355007f5b2fab0be5469a8662f75e762b534ecaebeefc6f7e2a11c70b` | 4,972,859 |
| `copdem-glo-90-n43-e145` | 1200 × 1200 | 43–44°N, 145–146°E | `c65f9de77ffb7738b7f44a11d96fff2c7b4cbe45be46a337f02c8eec40d901ae` | 1,741,167 |
| `copdem-glo-90-n43-e146` | 1200 × 1200 | 43–44°N, 146–147°E | `f53ef6f6fd1764d7ae6106aa7951502994c16d2a16069bada771972d131a654b` | 314,701 |
| `copdem-glo-90-n44-e013` | 1200 × 1200 | 44–45°N, 13–14°E | `55c5b88ac1329e5f5db5f156ea4a3ae4c708e962b48a87f99312ab98456d97d2` | 294,608 |
| `copdem-glo-90-n44-e014` | 1200 × 1200 | 44–45°N, 14–15°E | `d2b049cee625d792d96c3c2880861e028c7db02b4084280cada0b36e96b80e95` | 1,096,690 |
| `copdem-glo-90-n44-e015` | 1200 × 1200 | 44–45°N, 15–16°E | `9cb2975e0c48f8f00f7b7673d0d0fe60bc0f277255feb2f9d249758be91c7a26` | 4,859,003 |
| `copdem-glo-90-n44-e016` | 1200 × 1200 | 44–45°N, 16–17°E | `923d80858008d40117957080a261470f1ccb7f9b2a0a3846bba168caf8f74682` | 5,280,379 |
| `copdem-glo-90-n44-e017` | 1200 × 1200 | 44–45°N, 17–18°E | `1592889ffa18c09dd1f4ed8b7d36e00a4fd2c3614bbce0367190dbc8d9772aec` | 5,454,089 |
| `copdem-glo-90-n44-e018` | 1200 × 1200 | 44–45°N, 18–19°E | `8f860a707a90661d8450296fbc7d8fabcd5d5b733a2a6596ac3d49a6ba71b17c` | 5,486,582 |
| `copdem-glo-90-n44-e019` | 1200 × 1200 | 44–45°N, 19–20°E | `448c4cccb87e23ac2a1a8932a24514a28482b880c5b2accd6571aa52396edf61` | 5,431,501 |
| `copdem-glo-90-n44-e020` | 1200 × 1200 | 44–45°N, 20–21°E | `11d5de0e4f5678a2c138932e2c806cba247d1b90591097b16417476b1cbbdce4` | 5,328,178 |
| `copdem-glo-90-n44-e021` | 1200 × 1200 | 44–45°N, 21–22°E | `993192d6791be759452a127e9056fe8e015ad045e13e3e18f22a2bf2d9334d4f` | 5,434,902 |
| `copdem-glo-90-n44-e022` | 1200 × 1200 | 44–45°N, 22–23°E | `3c94f112c8d3450380ea0d9bb9bc085c8ce2630a9cb51d60e3de193fd594031c` | 5,421,246 |
| `copdem-glo-90-n44-e023` | 1200 × 1200 | 44–45°N, 23–24°E | `e49511c5484ac7ac31564a19f118ce6735c26c61145ccc7e4b042e96c572dee5` | 5,275,469 |
| `copdem-glo-90-n44-e026` | 1200 × 1200 | 44–45°N, 26–27°E | `483d4600557633e66114538f1be54bbe68b66d982310831764cf435af5b51c2f` | 5,148,411 |
| `copdem-glo-90-n44-e027` | 1200 × 1200 | 44–45°N, 27–28°E | `5dee33adb594a032b344a45e7c485361ca9e6a4807a053f1a177ebb7b7d07ce9` | 5,448,618 |
| `copdem-glo-90-n44-e145` | 1200 × 1200 | 44–45°N, 145–146°E | `a274c55c5f2792e0777b310d88637b98d271168e35339a64738febeba18e01ed` | 629,592 |
| `copdem-glo-90-n44-e146` | 1200 × 1200 | 44–45°N, 146–147°E | `67f2570f01a0794c286ae0879198acd42834cc2b517ac3ecf58003605540a3b4` | 645,075 |
| `copdem-glo-90-n44-e147` | 1200 × 1200 | 44–45°N, 147–148°E | `3bfb8120d808baccdb8953936ff95990e5d7f74b7ca0257ec7a9a7dc7e31493d` | 740,097 |
| `copdem-glo-90-n45-e013` | 1200 × 1200 | 45–46°N, 13–14°E | `c9265edc17c3c6454cadcb41a724ae80306ff73af5d744bedbdfc0d5701a8bf2` | 3,213,925 |
| `copdem-glo-90-n45-e014` | 1200 × 1200 | 45–46°N, 14–15°E | `9aacd75eed63b51457ebcf27cd1ada01322b627e417499938a842a3695e62044` | 4,883,037 |
| `copdem-glo-90-n45-e015` | 1200 × 1200 | 45–46°N, 15–16°E | `ab28e93be4244be5d4d0e1111314f0eef0be6ab8d53d1ec21c042b98b0eaf3f0` | 5,535,696 |
| `copdem-glo-90-n45-e016` | 1200 × 1200 | 45–46°N, 16–17°E | `1d82d8d143df400eb5cdaeaeed68774b024d1d6551e2ec016705350d448bc4c9` | 5,537,005 |
| `copdem-glo-90-n45-e017` | 1200 × 1200 | 45–46°N, 17–18°E | `a8221ebe92639087c00b0ebce36da032e596bba1614bfc42dc23e5b757c94eee` | 5,449,565 |
| `copdem-glo-90-n45-e018` | 1200 × 1200 | 45–46°N, 18–19°E | `8dea6990c886af51770f8030f1c3cb22518ff49b5caca5d9792e27019f3fa995` | 5,199,913 |
| `copdem-glo-90-n45-e019` | 1200 × 1200 | 45–46°N, 19–20°E | `12d8a7ca7eab0bf1605b653465ac527e2a506d17a8f4599efcbf5a9325d4dbcd` | 5,027,167 |
| `copdem-glo-90-n45-e020` | 1200 × 1200 | 45–46°N, 20–21°E | `9fcd72d6538513e5088f9ad3bfa87857e919048fdd5e321eb8ee6a2d9223289d` | 4,813,177 |
| `copdem-glo-90-n45-e021` | 1200 × 1200 | 45–46°N, 21–22°E | `f55e14bf483ae6017b436ab9db8128dd252b31e8d55a8730b7d3f2d8d1008955` | 5,210,346 |
| `copdem-glo-90-n45-e028` | 1200 × 1200 | 45–46°N, 28–29°E | `1d3f7166d2b2cee0c74722d0f144ce94cf643b9240ad03a979486a32e07a5a5a` | 5,436,924 |
| `copdem-glo-90-n45-e147` | 1200 × 1200 | 45–46°N, 147–148°E | `9d626c4ea17fafb6a82cd2e154899813d83d5eacf53b1fc1ce08aa6bca8fd383` | 675,072 |
| `copdem-glo-90-n45-e148` | 1200 × 1200 | 45–46°N, 148–149°E | `d231fa13350a1ac902e17d2cabbd8d541d211087809f1f614f089acb592c6c33` | 838,482 |
| `copdem-glo-90-n46-w057` | 1200 × 1200 | 46–47°N, 56–57°W | `58f23ea20af223ae13706473c616b4ebf8877056b5256660d531ed3d2d34d768` | 164,104 |
| `copdem-glo-90-n46-e009` | 1200 × 1200 | 46–47°N, 9–10°E | `6860c0b93a271f3b702a4a16aa885ba19c4907e808c56f94a620c6a657aa5f18` | 5,225,729 |
| `copdem-glo-90-n46-e010` | 1200 × 1200 | 46–47°N, 10–11°E | `4c0c831912f0ca9fe1cd53b27509aa30e76b08cf98a092b070aa9177c1a010e0` | 5,191,807 |
| `copdem-glo-90-n46-e011` | 1200 × 1200 | 46–47°N, 11–12°E | `0bf8bc145ec8f14bbc35c8f04b4ddbcfa42d9a5827245c0a1f0494d5caea174c` | 5,318,398 |
| `copdem-glo-90-n46-e012` | 1200 × 1200 | 46–47°N, 12–13°E | `da3d8d471b8fbf7fffdd80cf157e5f405fb396105aee1401766c60658c69d431` | 5,337,700 |
| `copdem-glo-90-n46-e013` | 1200 × 1200 | 46–47°N, 13–14°E | `e45877090957632144376559bd96960a9b8d94cf8204e33279b03a7abf20f5d6` | 5,369,136 |
| `copdem-glo-90-n46-e014` | 1200 × 1200 | 46–47°N, 14–15°E | `befd104b7bdfd2f7ca39445f823776730d8d6bcc6122054dde03d247194a3bae` | 5,360,415 |
| `copdem-glo-90-n46-e015` | 1200 × 1200 | 46–47°N, 15–16°E | `d3bfb2cb378f3944c04d4f19b9726791f0880274e1147113e51cee674fd4b6dd` | 5,451,632 |
| `copdem-glo-90-n46-e016` | 1200 × 1200 | 46–47°N, 16–17°E | `52482e0d6d831d00ff2592265b01b121816b6f97f9bef148034f32c705169a7c` | 5,321,819 |
| `copdem-glo-90-n46-e017` | 1200 × 1200 | 46–47°N, 17–18°E | `7c99615594cb0dfbb1f7f1fc239c57ff376d630bc5bb4fe05f864db494e17b0a` | 5,193,619 |
| `copdem-glo-90-n46-e018` | 1200 × 1200 | 46–47°N, 18–19°E | `48f43e86e8c31e40b797858dfa7a00557441fd987f18e45e6ebf5c006b5e4221` | 5,353,382 |
| `copdem-glo-90-n46-e019` | 1200 × 1200 | 46–47°N, 19–20°E | `493cedaf5868e9fbcc9f4cb2c78089992fdfed9bb168b288ad405ce30bf78fb1` | 5,217,441 |
| `copdem-glo-90-n46-e020` | 1200 × 1200 | 46–47°N, 20–21°E | `5de398ceb80a249788012a19428f453a5519e6e84005fdd423e5210962aa588b` | 4,840,776 |
| `copdem-glo-90-n46-e021` | 1200 × 1200 | 46–47°N, 21–22°E | `0363d66387d7602dc59a02958ce3d5a7df28557ff9be3088dd5807ca3642e8f1` | 4,999,366 |
| `copdem-glo-90-n46-e028` | 1200 × 1200 | 46–47°N, 28–29°E | `1fd0ba2af245f555255ad2c12e62e340dfc7311806af3b135e864269093746cf` | 5,606,581 |
| `copdem-glo-90-n46-e029` | 1200 × 1200 | 46–47°N, 29–30°E | `1ea2d691f760a3205ead41ac04a15555fa3152eb7af95297eff2026435a61984` | 5,554,278 |
| `copdem-glo-90-n46-e030` | 1200 × 1200 | 46–47°N, 30–31°E | `4a0970b6176fc0c0a700b188721940e3f5330bbd2dc144abd236fd419319b397` | 4,123,388 |
| `copdem-glo-90-n47-w057` | 1200 × 1200 | 47–48°N, 56–57°W | `ebe93af7a15b4be56cdcd1da127f2533ab8c063836e251cc6ae64ca23cd05fef` | 2,219,851 |
| `copdem-glo-90-n47-e010` | 1200 × 1200 | 47–48°N, 10–11°E | `ec174c3c040e2795a98e8587107f051c109ca3d46e4dfafb4e67cb1f9fcf1f63` | 5,123,144 |
| `copdem-glo-90-n47-e011` | 1200 × 1200 | 47–48°N, 11–12°E | `fa7d946d23516eafb3eb6f4c2f98de750b0dd711a82bb65bb12a3c00bc01a252` | 5,103,740 |
| `copdem-glo-90-n47-e012` | 1200 × 1200 | 47–48°N, 12–13°E | `621d7ac02a22713cd4a63fa7693441831b428657e3f1d7e23badf48539a7423c` | 5,181,456 |
| `copdem-glo-90-n47-e013` | 1200 × 1200 | 47–48°N, 13–14°E | `a2f06dae73a11693abd71798a2fa131cd04434b0d6900c932df3a73d13dc8900` | 5,219,385 |
| `copdem-glo-90-n47-e014` | 1200 × 1200 | 47–48°N, 14–15°E | `105c237e4fc49aa0aa8e1891789cfeb5d4dadc97f530256b75e2f943b069718b` | 5,320,167 |
| `copdem-glo-90-n47-e015` | 1200 × 1200 | 47–48°N, 15–16°E | `c14a9116fc0f560d7f652b4cdd08f8e335fc14129eabac81f8008a53b3600d5a` | 5,393,186 |
| `copdem-glo-90-n47-e016` | 1200 × 1200 | 47–48°N, 16–17°E | `780b977a214ec6d9ef7abb0ca6a932e2b4da01c39517ef99a713f1c0e67eb21f` | 5,123,523 |
| `copdem-glo-90-n47-e017` | 1200 × 1200 | 47–48°N, 17–18°E | `6ad46a45568875b18827b7dd0f5bb522cf6dab49efc8d412c8c6b18f521d7242` | 5,159,101 |
| `copdem-glo-90-n47-e018` | 1200 × 1200 | 47–48°N, 18–19°E | `bafbcd7315cce638fc1bfefd51d93d16d83ae66d9a6647081673f7c4a3815775` | 5,266,429 |
| `copdem-glo-90-n47-e019` | 1200 × 1200 | 47–48°N, 19–20°E | `9f20e4eeacde09733d058756b775d04de25a6bfd8d41b3b5b69acf2460469942` | 5,262,299 |
| `copdem-glo-90-n47-e020` | 1200 × 1200 | 47–48°N, 20–21°E | `110b7f0aa28f6e68e20f5a2c31c3e5ec338ded5162171ca80562b8fcabc49d3f` | 4,944,702 |
| `copdem-glo-90-n47-e021` | 1200 × 1200 | 47–48°N, 21–22°E | `9824dab08aba5ab5af021114becf80d8cc1ffd19c127e479117ce0357871990c` | 5,032,324 |
| `copdem-glo-90-n47-e022` | 1200 × 1200 | 47–48°N, 22–23°E | `e53196bcd22390689b52a2a6e787252d295d7c81e189d49eb0349631d6626efd` | 5,158,190 |
| `copdem-glo-90-n47-e027` | 1200 × 1200 | 47–48°N, 27–28°E | `fb7ad781167768773ff07503cf76c56e9e74a4cc98a853e5c8ea21c8b3cf5ef5` | 5,530,564 |
| `copdem-glo-90-n47-e028` | 1200 × 1200 | 47–48°N, 28–29°E | `db51cc684d98c372de253617eeb91d50357017f0e4a3a78870db410cb48af63e` | 5,516,883 |
| `copdem-glo-90-n47-e029` | 1200 × 1200 | 47–48°N, 29–30°E | `a53459feb8583ffa37677339e50def49f45348400fd14363827fc3cb9abfb83b` | 5,459,574 |
| `copdem-glo-90-n48-e012` | 1200 × 1200 | 48–49°N, 12–13°E | `d521add1f23832bb75da207bf357941a4ede0ab4242a447d714154a2a4ad0869` | 5,114,422 |
| `copdem-glo-90-n48-e013` | 1200 × 1200 | 48–49°N, 13–14°E | `471f5b1be97511fd53a19296a50b9f4635d1777f1f0c0604a676342acd9bb246` | 5,201,440 |
| `copdem-glo-90-n48-e014` | 1200 × 1200 | 48–49°N, 14–15°E | `ddde52136224993e97339f85847187620abd98d42b4a055e068f4a862d868edf` | 5,160,447 |
| `copdem-glo-90-n48-e015` | 1200 × 1200 | 48–49°N, 15–16°E | `d3ab43171c163e0b69426134d380d628af6c935972ba646ce8be3edff855f27e` | 5,184,891 |
| `copdem-glo-90-n48-e016` | 1200 × 1200 | 48–49°N, 16–17°E | `2982cd7e677dc2c88095466a944d41654ac6e3137bcb3c42d991678fca7bdfcc` | 5,123,607 |
| `copdem-glo-90-n48-e017` | 1200 × 1200 | 48–49°N, 17–18°E | `63473b8630cc126d6a39d2e47bbbbe75e751f5e1942a80f2b1b6827843099d5d` | 5,188,273 |
| `copdem-glo-90-n48-e018` | 1200 × 1200 | 48–49°N, 18–19°E | `4144a703f0e82885b25238878762ac45d8461cbe0b4b2e18455e71e6f2bc93f8` | 5,356,749 |
| `copdem-glo-90-n48-e019` | 1200 × 1200 | 48–49°N, 19–20°E | `fb5a2f854f50119bd83870d3c0fb6356ab00fd8610021a415d2a0e7ee4dd7ff8` | 5,391,628 |
| `copdem-glo-90-n48-e020` | 1200 × 1200 | 48–49°N, 20–21°E | `ee168c159c1249d83c6411f309a53e524cecba3d492b0437ef246bcd8ac33e32` | 5,403,053 |
| `copdem-glo-90-n48-e021` | 1200 × 1200 | 48–49°N, 21–22°E | `68d3844397897734b3fdff615f54afc2b63a2ae638cef9f78425408a4e71ed5e` | 5,280,252 |
| `copdem-glo-90-n48-e022` | 1200 × 1200 | 48–49°N, 22–23°E | `8d24aa8fd9383689faaabf1fb077b65f9f61165e3c49680eec52f9a9f95a0426` | 5,273,774 |
| `copdem-glo-90-n48-e026` | 1200 × 1200 | 48–49°N, 26–27°E | `a496fdabcadb54b003e1610a562ef60691fc60f2fc4bf5bb51465ad6db72af66` | 5,177,466 |
| `copdem-glo-90-n48-e027` | 1200 × 1200 | 48–49°N, 27–28°E | `f122343528b605e7bc073fefbc6961feb606dd78c78e209d6e4c0b90c2e3b08f` | 5,162,394 |
| `copdem-glo-90-n48-e028` | 1200 × 1200 | 48–49°N, 28–29°E | `4e28abd901572175461a5acc2d485f314e3a9873d5f94b0a569c204f4d285602` | 5,168,241 |
| `copdem-glo-90-n49-w003` | 1200 × 1200 | 49–50°N, 2–3°W | `752852b2ace41e0f250f735136016a6ecb9afbead7551edc2ab1dc296c7d7844` | 231,297 |
| `copdem-glo-90-n49-e004` | 1200 × 1200 | 49–50°N, 4–5°E | `0f75428abc4bbe1f50dfe154f935494863dc1dc46272b7384286ee002704fb4c` | 5,341,812 |
| `copdem-glo-90-n49-e012` | 1200 × 1200 | 49–50°N, 12–13°E | `bf95d93311ec2c6fd37feb4430fe92783bd354aabcf90a965273ee8196f50821` | 5,224,133 |
| `copdem-glo-90-n49-e013` | 1200 × 1200 | 49–50°N, 13–14°E | `fcd217dab9e5d8d198f424c24cd9a446dbdab955e517255805033a0faf9607e2` | 5,153,362 |
| `copdem-glo-90-n49-e014` | 1200 × 1200 | 49–50°N, 14–15°E | `a19c97beadd7b28f84778e9898e7865dee934bf4a4ed768cf68a03554cdc94c3` | 5,154,933 |
| `copdem-glo-90-n49-e015` | 1200 × 1200 | 49–50°N, 15–16°E | `bda694090c111d7df62b47abd0c4fca4e5700df278996da09692704e47765090` | 5,089,776 |
| `copdem-glo-90-n49-e016` | 1200 × 1200 | 49–50°N, 16–17°E | `533c5950efa180dce2dcdd490248807e45b961273b5f4a3453f13e5216222232` | 5,246,193 |
| `copdem-glo-90-n49-e017` | 1200 × 1200 | 49–50°N, 17–18°E | `0f4baab3fe4011c4eca1ebe166926b6fd4b0d2232921e51d70b6f5a2545b131e` | 5,214,165 |
| `copdem-glo-90-n49-e018` | 1200 × 1200 | 49–50°N, 18–19°E | `888ff92a52a78fa1c364a32b05e07e66f1a85baf1d007e3525bfe0c1c3f39d74` | 5,319,930 |
| `copdem-glo-90-n49-e019` | 1200 × 1200 | 49–50°N, 19–20°E | `e2b07fd96671645639d03ab3b6ef52b2e9d5627f05338245c03cddb0c481307f` | 5,179,923 |
| `copdem-glo-90-n49-e020` | 1200 × 1200 | 49–50°N, 20–21°E | `e7ad485c42fbac3ce4c5e41741f717a31ef667b109f642fdfbc038ce6576265f` | 5,252,076 |
| `copdem-glo-90-n49-e021` | 1200 × 1200 | 49–50°N, 21–22°E | `596f76ddb157fd1661fec2b3fe8b8ee0532c06724cc97d2ba326a5283f6223b7` | 5,341,486 |
| `copdem-glo-90-n49-e022` | 1200 × 1200 | 49–50°N, 22–23°E | `306e3d2923812db5bffb56f8bffe7a59b46e6f47932978adcff025e6f8b66c53` | 5,313,758 |
| `copdem-glo-90-n50-e002` | 800 × 1200 | 50–51°N, 2–3°E | `a785f9f9010da9226efbae231c4dba78ee52719c277e91ae3af6e51d00c20164` | 3,870,657 |
| `copdem-glo-90-n50-e003` | 800 × 1200 | 50–51°N, 3–4°E | `051dd549a412b26df2a51d1d4531ad2b725ca27d3caa409d9a5f7f44e208d381` | 3,848,419 |
| `copdem-glo-90-n50-e004` | 800 × 1200 | 50–51°N, 4–5°E | `91644530457e328c7c8a7b756a10887a2413b64dc4d5893e1c250f6d4f737e7c` | 3,784,886 |
| `copdem-glo-90-n50-e012` | 800 × 1200 | 50–51°N, 12–13°E | `00949867ea51d16f057fd9ca3ef3d240430bb1ba5bdec5b9ae29ed2a6d35dee4` | 3,597,171 |
| `copdem-glo-90-n50-e013` | 800 × 1200 | 50–51°N, 13–14°E | `7f8b8256f8a40db420199fb91a9ac5e288acb6b33fdf62d2f6b5f5fe3f1ec2f9` | 3,595,889 |
| `copdem-glo-90-n50-e014` | 800 × 1200 | 50–51°N, 14–15°E | `190096e238b7b3624d965670664714cd32fadf357a19b6799da8a023f56f0174` | 3,611,521 |
| `copdem-glo-90-n50-e015` | 800 × 1200 | 50–51°N, 15–16°E | `92064afbac2d83951a1f0cd267062a6c5cbc7a80698aa4b6ebb705b2b49ced4e` | 3,599,498 |
| `copdem-glo-90-n50-e016` | 800 × 1200 | 50–51°N, 16–17°E | `e45349315f8c3d52e55cd80ebf0bebe87c6bf0ae6ca7426bcdc69b090f0577c9` | 3,623,786 |
| `copdem-glo-90-n50-e017` | 800 × 1200 | 50–51°N, 17–18°E | `4ac28bf7d0d68fec057276a6240382128aa7506203368e79636e816eefd95270` | 3,530,755 |
| `copdem-glo-90-n50-e018` | 800 × 1200 | 50–51°N, 18–19°E | `682c1d65693ca274b78be38049591c590492a26b5c784eb4d0bec7bb00782c05` | 3,549,707 |
| `copdem-glo-90-n51-w011` | 800 × 1200 | 51–52°N, 10–11°W | `b0cbc4df9e2116eeaece53fc446cde69a9de412311aef8f1e71a4b51ed1560e0` | 399,066 |
| `copdem-glo-90-n51-w010` | 800 × 1200 | 51–52°N, 9–10°W | `394f3091211507b383472d3c6d240e4abad2ba68f340c4844feee483a4fca022` | 1,780,797 |
| `copdem-glo-90-n51-w009` | 800 × 1200 | 51–52°N, 8–9°W | `f6c51c91fe65a65530fa8cbdc4cfb383a396d92ee87fb3616d7793ec53da9687` | 1,286,576 |
| `copdem-glo-90-n51-w008` | 800 × 1200 | 51–52°N, 7–8°W | `a7e6748b8a576705fb279b1137dbd801e21ffaf6f6369095c09ab9fa41b7f87b` | 171,825 |
| `copdem-glo-90-n51-e002` | 800 × 1200 | 51–52°N, 2–3°E | `ef85067c1e723f4865ad123154dbf65de23361e73a1935bdd18297ad364e2357` | 498,908 |
| `copdem-glo-90-n51-e003` | 800 × 1200 | 51–52°N, 3–4°E | `cf2ab6c647b5f86dbef3577873a0c91871cd3024bc4141464fb20a2451444c0d` | 2,052,137 |
| `copdem-glo-90-n51-e004` | 800 × 1200 | 51–52°N, 4–5°E | `027580d05d1e455d7d950fba4ac444315ff09d352f9ff014a133022e5a5e63a8` | 3,980,818 |
| `copdem-glo-90-n51-e005` | 800 × 1200 | 51–52°N, 5–6°E | `57108d8f54ccf7af137cd36611baa14550f3a10908c25bd3836e8010a55fe95d` | 3,969,495 |
| `copdem-glo-90-n51-e006` | 800 × 1200 | 51–52°N, 6–7°E | `b5ef8b4440f3d1435eb6699e80a6e09af09f42ffb716c5f3ee511779aef3a550` | 3,832,093 |
| `copdem-glo-90-n51-e014` | 800 × 1200 | 51–52°N, 14–15°E | `b1de9bfdac1f37193115f7dae76491c18e8b316a4e807f96ecbce33bfbaf1e7c` | 3,651,420 |
| `copdem-glo-90-n51-e015` | 800 × 1200 | 51–52°N, 15–16°E | `b1c50f5e95bfdd9dd29c0afb91be7866eeeeb83b6521dcc021b7af8ee576a247` | 3,665,239 |
| `copdem-glo-90-n52-w011` | 800 × 1200 | 52–53°N, 10–11°W | `b13da8cc0d9a7100d988464ebb4952191ee8528b0dee4087f45dd6517965a552` | 341,742 |
| `copdem-glo-90-n52-w010` | 800 × 1200 | 52–53°N, 9–10°W | `4b2a4cb9102a19ad4af3a42eda9365333b7a409b7a511875ecae168c045c0b44` | 2,777,133 |
| `copdem-glo-90-n52-w009` | 800 × 1200 | 52–53°N, 8–9°W | `e1c481ea0f68b675bc0742bc435836f27056f281e1b00b846736716840d23622` | 3,771,686 |
| `copdem-glo-90-n52-w008` | 800 × 1200 | 52–53°N, 7–8°W | `d318fe9fa78fbe32b7d1a25d7cdc8c0bd91e9c06c75156f4454e475d7569cf36` | 3,551,552 |
| `copdem-glo-90-n52-w007` | 800 × 1200 | 52–53°N, 6–7°W | `ccbf74f82337d9f3d9af6712eb81abed4ea800df6ff206d49adb8aab7900b15d` | 2,511,795 |
| `copdem-glo-90-n52-w006` | 800 × 1200 | 52–53°N, 5–6°W | `4eea446185fbb5cdf6605d8c6ebeb62d9404c26b0027fa44d9faa553d25605d4` | 61,294 |
| `copdem-glo-90-n52-e004` | 800 × 1200 | 52–53°N, 4–5°E | `9a7bff2e790fe47e47784984e129a2d2116bbd3c08e4a41eb769acc55ede5ed2` | 1,998,003 |
| `copdem-glo-90-n52-e005` | 800 × 1200 | 52–53°N, 5–6°E | `90155d4f8b01522d16580465f544805c5a1e1a96004ea18d99920e76eb39d350` | 3,181,099 |
| `copdem-glo-90-n52-e006` | 800 × 1200 | 52–53°N, 6–7°E | `2bcebbf08a8529563819846705001564938706f1161b3260f1bda2259fa83c83` | 4,027,162 |
| `copdem-glo-90-n52-e007` | 800 × 1200 | 52–53°N, 7–8°E | `2b2702fe091ef2effa804d5226b06392af709a272dd100af3f2caa53af464ce2` | 3,875,963 |
| `copdem-glo-90-n53-w011` | 800 × 1200 | 53–54°N, 10–11°W | `8be4c9e64d64c9cb30e02ddf9c7d7b7f7438865e50b878f1268e9e0b95fa067e` | 241,866 |
| `copdem-glo-90-n53-w010` | 800 × 1200 | 53–54°N, 9–10°W | `74af1b0ec261d5d38eb24fc77799df654dad250e082fd718004254c5cb07a764` | 2,836,844 |
| `copdem-glo-90-n53-w009` | 800 × 1200 | 53–54°N, 8–9°W | `8d8f1eaf14b1f4d1832e359f682b8e550a1fbaefa9c56205f5851344ae219d1d` | 3,725,288 |
| `copdem-glo-90-n53-w008` | 800 × 1200 | 53–54°N, 7–8°W | `7f3643aabd8ce2fef6362c8a1f5b6d5a0524ac20c2f447bd14ad3bcd3ead9eb9` | 3,684,155 |
| `copdem-glo-90-n53-w007` | 800 × 1200 | 53–54°N, 6–7°W | `b49d0e03f875985d1508038bc043ac37887534d01774db170810da9ce7f9c62d` | 3,194,846 |
| `copdem-glo-90-n53-e004` | 800 × 1200 | 53–54°N, 4–5°E | `e4244bb81eafe84293647984a813c34aa80f3c1634612aa199cae1c25288777b` | 167,159 |
| `copdem-glo-90-n53-e005` | 800 × 1200 | 53–54°N, 5–6°E | `5d425671b7e54d700f76a1fc7e0aa6c507053a25159ba845cd229ed801c0619a` | 1,042,414 |
| `copdem-glo-90-n53-e006` | 800 × 1200 | 53–54°N, 6–7°E | `673edb5c2101ef9eddae2e9367266c1750f3ce5a0944f458247b27446da4cbe6` | 1,942,799 |
| `copdem-glo-90-n53-e007` | 800 × 1200 | 53–54°N, 7–8°E | `ebeb7dbbc0eb8e1e6fa81bcdcb7bdc13de62976c19f783a962ed6c4878fc5c44` | 2,863,811 |
| `copdem-glo-90-n53-e023` | 800 × 1200 | 53–54°N, 23–24°E | `e4437fc416a78ed8c5b1491dc82d4c9182369e8db93a8645f19320c6da036d55` | 3,705,282 |
| `copdem-glo-90-n53-e024` | 800 × 1200 | 53–54°N, 24–25°E | `15a029f39dfdb1f57ae2ac9f85bee236c8563762f0778a049cfa45331a45377e` | 3,644,579 |
| `copdem-glo-90-n54-w011` | 800 × 1200 | 54–55°N, 10–11°W | `6c2d61ef35ab30b1365ee921b316998922e2500e29a7770aaf16a1413972bc59` | 132,257 |
| `copdem-glo-90-n54-w010` | 800 × 1200 | 54–55°N, 9–10°W | `261c86ea4f082bed53bc6d6e4aaa671eb66f6531fc9bf66f8564d479128cc310` | 1,143,075 |
| `copdem-glo-90-n54-w009` | 800 × 1200 | 54–55°N, 8–9°W | `2d9675cab5f35ee4b13d6700135e891ddbd0173a130cd151b70f5a6f44690527` | 2,495,986 |
| `copdem-glo-90-n54-w008` | 800 × 1200 | 54–55°N, 7–8°W | `7b9085625d719c25da05f7c7f7474f118b467e9daa2bd29fba73a9335a6c8507` | 3,788,523 |
| `copdem-glo-90-n54-w007` | 800 × 1200 | 54–55°N, 6–7°W | `b7c938a597410131cac303106d29c14cbaf155f3cc705791aed6ad5cd163ac70` | 3,663,040 |
| `copdem-glo-90-n54-e008` | 800 × 1200 | 54–55°N, 8–9°E | `437a71b180706981c9c2a3f6e4f568da86657ebec78b5614d5a35dd2a2db7fee` | 1,252,983 |
| `copdem-glo-90-n54-e009` | 800 × 1200 | 54–55°N, 9–10°E | `391621554ec472aa206110e1414babd551d5de960aaccf095f6c6a4b31c32ba4` | 3,866,484 |
| `copdem-glo-90-n54-e010` | 800 × 1200 | 54–55°N, 10–11°E | `59b2f5ba79c343f1ab3f77cb019b105aee51d2cb288c2e13dd5a66e87b45185f` | 1,790,615 |
| `copdem-glo-90-n54-e011` | 800 × 1200 | 54–55°N, 11–12°E | `52f976748ef1f71f5e491af34f968435bcf6c66bb1b05200b29abe8cc756a3fb` | 1,564,342 |
| `copdem-glo-90-n54-e012` | 800 × 1200 | 54–55°N, 12–13°E | `589930bf87b5f9bddb481abe9ce1e67e73e0f79d85f40d092d5afa64d76ea715` | 1,647,976 |
| `copdem-glo-90-n54-e014` | 800 × 1200 | 54–55°N, 14–15°E | `dcdf3a24771abb0cd4b2d8b406d7a7f0fbfe974f690011b9e4d62c17dfc6c70a` | 126,058 |
| `copdem-glo-90-n54-e015` | 800 × 1200 | 54–55°N, 15–16°E | `4a35ba9dec306f32557816629084bc91f315cbdf2c562bc504c62e5f673f5980` | 763,924 |
| `copdem-glo-90-n54-e022` | 800 × 1200 | 54–55°N, 22–23°E | `111cd00690b7a75df9ae59b9d2c262e4f7fe61fb4ac9947aa9b07fa827a8c488` | 3,692,958 |
| `copdem-glo-90-n54-e023` | 800 × 1200 | 54–55°N, 23–24°E | `779a4f976c82dcaed307ab4b69d0f66c3821dd1d82aaf029217d8160a1ea1906` | 3,690,624 |
| `copdem-glo-90-n54-e024` | 800 × 1200 | 54–55°N, 24–25°E | `6988c0b83c2421e9b2cac2dc30738eceeeba8d7133945178370a7d408801af24` | 3,727,859 |
| `copdem-glo-90-n54-e025` | 800 × 1200 | 54–55°N, 25–26°E | `339486187d5cc472238b14fb43a4ef46d05fcbffe149d3d272dd99c53d6209a4` | 3,655,742 |
| `copdem-glo-90-n54-e026` | 800 × 1200 | 54–55°N, 26–27°E | `0aae85178482de1b6b683f4ce353b27e7d4b50e3505146b642fd37596f9241e6` | 3,539,244 |
| `copdem-glo-90-n55-w009` | 800 × 1200 | 55–56°N, 8–9°W | `cc649e88182bf1185d52c7333bc14a1fdafc584b30bd0a3a77296b5dac2c3265` | 298,323 |
| `copdem-glo-90-n55-w008` | 800 × 1200 | 55–56°N, 7–8°W | `fb9e7ebc903bd8d8322505d128aede561839d312004a06441f1b01f2899390d9` | 1,003,469 |
| `copdem-glo-90-n55-w007` | 800 × 1200 | 55–56°N, 6–7°W | `22fd0ccde5bceb502378fb5a562affd5b81788e5957b2c569c78c0bd466595fe` | 1,284,850 |
| `copdem-glo-90-n55-e008` | 800 × 1200 | 55–56°N, 8–9°E | `06f32a9b20ed0c0c717e5b0c2851480de02f81b7a2c1557f721ca5aecfafe30b` | 2,486,896 |
| `copdem-glo-90-n55-e009` | 800 × 1200 | 55–56°N, 9–10°E | `29eb5d6b9150c94047f4f39ea457125f4793b7626998d9d21637c88d941f096e` | 3,242,005 |
| `copdem-glo-90-n55-e010` | 800 × 1200 | 55–56°N, 10–11°E | `182ff807192c45289676e3565a42c1626d51e91e3ab6e4480110908d9738e2e5` | 1,928,753 |
| `copdem-glo-90-n55-e011` | 800 × 1200 | 55–56°N, 11–12°E | `72f4a294ece767f83c6de08e12c60d23b718104d4f4c4ec5acb1c2ae98528a87` | 2,485,099 |
| `copdem-glo-90-n55-e012` | 800 × 1200 | 55–56°N, 12–13°E | `bf5d7a6ee24092b0a0a36044929185164da8ba8fe523b12bc1b96a1ebf0e2a57` | 1,918,213 |
| `copdem-glo-90-n55-e014` | 800 × 1200 | 55–56°N, 14–15°E | `9e72b11af79d8fc7e3b78d4a2f6ce76a126b9973c79c04a59b1126c779d6acde` | 998,322 |
| `copdem-glo-90-n55-e015` | 800 × 1200 | 55–56°N, 15–16°E | `c6d949cd25462569b57ad6451517a219228a14086a246e7f5dc74e7884c55dcd` | 147,782 |
| `copdem-glo-90-n55-e020` | 800 × 1200 | 55–56°N, 20–21°E | `7c4c9f57a6e1bf4693e7a5b94a3476bf160eebe789360b243d5274773ca7d17e` | 107,578 |
| `copdem-glo-90-n55-e021` | 800 × 1200 | 55–56°N, 21–22°E | `64798c2d6ca4a959d435201e8c623e78132515ffc8c49ee457721ab5a096e1ba` | 3,397,830 |
| `copdem-glo-90-n55-e022` | 800 × 1200 | 55–56°N, 22–23°E | `891b04b894c74c35484a5c84a70ee4bf59b32ed4c205d6d8c57cb0e601da660a` | 3,761,785 |
| `copdem-glo-90-n55-e023` | 800 × 1200 | 55–56°N, 23–24°E | `4f75c8898393c8c60ad934e8976342ffca8c98303c210976c616b7822a13186f` | 3,690,101 |
| `copdem-glo-90-n55-e024` | 800 × 1200 | 55–56°N, 24–25°E | `a1d4c61d0398f536c632d2b877fe85d47e317f29542492e689a3eb8f763416b2` | 3,709,381 |
| `copdem-glo-90-n55-e025` | 800 × 1200 | 55–56°N, 25–26°E | `07308a7e43eddac34a78e9ddfc7291fd073a5a8176a6a9767cb7b2878066b56e` | 3,702,123 |
| `copdem-glo-90-n55-e026` | 800 × 1200 | 55–56°N, 26–27°E | `b27e8db0cae194ce6204789b33a0b00130310fd7a2200faa405889f5c8e1423e` | 3,574,307 |
| `copdem-glo-90-n55-e027` | 800 × 1200 | 55–56°N, 27–28°E | `2da0621774db84b8bbcffc16de6274ed291655d962203d10acdf075772867ff4` | 3,516,971 |
| `copdem-glo-90-n56-e008` | 800 × 1200 | 56–57°N, 8–9°E | `7e53b8ed01ba9b59f489cd4979bb2e33e69f2d2c70aa8110f56449f7d3aaa954` | 2,823,655 |
| `copdem-glo-90-n56-e009` | 800 × 1200 | 56–57°N, 9–10°E | `bbfc836e73c0be076ae78e0cb8b26aad6ca088c419f1d8c26717a4262cf14906` | 3,652,983 |
| `copdem-glo-90-n56-e010` | 800 × 1200 | 56–57°N, 10–11°E | `9957bbcf92978b6f2892e6ac77f7b233419bb5506a28ebc826b92baa6c099544` | 2,021,324 |
| `copdem-glo-90-n56-e011` | 800 × 1200 | 56–57°N, 11–12°E | `fd77f777107963c6e16c1458ace7739a7fb14c399059900b44daa3760b17788f` | 74,953 |
| `copdem-glo-90-n56-e012` | 800 × 1200 | 56–57°N, 12–13°E | `81a6254a5999365b56b01df0330c8ee7d17e740b7fbf808c14b9ed7a31702f25` | 1,623,360 |
| `copdem-glo-90-n56-e020` | 800 × 1200 | 56–57°N, 20–21°E | `d64f880e259bcdd08bf2dda5a17d1a19915d5ed48002a21fac17dfa054630e2d` | 83,022 |
| `copdem-glo-90-n56-e021` | 800 × 1200 | 56–57°N, 21–22°E | `547d3b4b91b4ded50f7edc9e2155b50293dbed6c4d4b68ce8753f37b2caf33fc` | 3,689,890 |
| `copdem-glo-90-n56-e022` | 800 × 1200 | 56–57°N, 22–23°E | `04738988279ddd5569af4666194345626e99b74cc770a95e9f67dc7bf3026933` | 3,753,646 |
| `copdem-glo-90-n56-e023` | 800 × 1200 | 56–57°N, 23–24°E | `c2496c9bd11ac6d975efe11f843c72e8fe3c62c87c6ef5eb9439dd04f62c7636` | 3,805,040 |
| `copdem-glo-90-n56-e024` | 800 × 1200 | 56–57°N, 24–25°E | `f6ca63655d722ce19d066a92b9a99e79f1c213d771d3eb8073525ead32b0538a` | 3,861,386 |
| `copdem-glo-90-n56-e025` | 800 × 1200 | 56–57°N, 25–26°E | `46baf86d5dee55151f387bcad22c1a53282125f6c334f571435cba74d54328a7` | 3,753,322 |
| `copdem-glo-90-n56-e026` | 800 × 1200 | 56–57°N, 26–27°E | `57b06732f20c87f2270fcc9c9fc3d6197e62bee327b14df1879b851f9d0eb38e` | 3,675,985 |
| `copdem-glo-90-n56-e027` | 800 × 1200 | 56–57°N, 27–28°E | `a1cf80395d503afc37eeb09dd1a023f08a4c37e49755f3b12e514182f9adf9f3` | 3,637,113 |
| `copdem-glo-90-n56-e028` | 800 × 1200 | 56–57°N, 28–29°E | `fffe5c435612b351e064325d87da0d4dc397373dd26713e7e2b476906aeba7af` | 3,688,383 |
| `copdem-glo-90-n57-e008` | 800 × 1200 | 57–58°N, 8–9°E | `3b0c920bd4ab008f71393544bec78db85280438bc7e59ab7711fabde52d15585` | 286,996 |
| `copdem-glo-90-n57-e009` | 800 × 1200 | 57–58°N, 9–10°E | `a9786805fb9fd9ddf835b94140dc4f13f0daef5f8b6dd5fb7e17b5b326f7b9c6` | 1,143,280 |
| `copdem-glo-90-n57-e010` | 800 × 1200 | 57–58°N, 10–11°E | `74b66cf96a52971bbeda1d2addc3b0dcff6cb186e303a4c3607e1f66ea6828b0` | 1,304,662 |
| `copdem-glo-90-n57-e011` | 800 × 1200 | 57–58°N, 11–12°E | `738f1364cf10a8912a089e41af34dbf6f31016338577869e32003d3d5c392e34` | 749,927 |
| `copdem-glo-90-n57-e021` | 800 × 1200 | 57–58°N, 21–22°E | `c9525e09cfcae582348655fc74d5eba367f8d64c257e89350a048eed1d74e2a9` | 1,266,848 |
| `copdem-glo-90-n57-e022` | 800 × 1200 | 57–58°N, 22–23°E | `88effa041c469d632c52c5fd524c544305078221e0ef037d5107f4f413fba718` | 2,471,406 |
| `copdem-glo-90-n57-e023` | 800 × 1200 | 57–58°N, 23–24°E | `94b2da56ebd46d40e70138f501eabacabbdf0c9e488ae8ae1dace6cd8f5632bc` | 451,108 |
| `copdem-glo-90-n57-e024` | 800 × 1200 | 57–58°N, 24–25°E | `b1aa8ba2ab3c4912b995095b26fdf308f0461a61c5a9a9361ded4e05de59bf6c` | 2,652,185 |
| `copdem-glo-90-n57-e025` | 800 × 1200 | 57–58°N, 25–26°E | `b96e1f2f6f0a34a20feb5efcc8cc26d0d73ee272c934a5eb64276d810a33ed62` | 3,744,121 |
| `copdem-glo-90-n57-e026` | 800 × 1200 | 57–58°N, 26–27°E | `28d87852682a6720c94a698e1552a4dab1511793240e854d33f5dc9cf37e2379` | 3,790,200 |
| `copdem-glo-90-n57-e027` | 800 × 1200 | 57–58°N, 27–28°E | `1ed6eff52a734f51476e3ea75c595ff36ba636e87c9770c17f0508be65e624a9` | 3,698,384 |
| `copdem-glo-90-n58-e021` | 800 × 1200 | 58–59°N, 21–22°E | `dfea448afba09f4abd52703581079fb58ee8ad1a92f8af57a4eab7992bba2591` | 181,349 |
| `copdem-glo-90-n58-e022` | 800 × 1200 | 58–59°N, 22–23°E | `581ec998d58f4b37d17c62e180d7b9118eb68a5164292c6055e1c51cc7ab70d0` | 2,112,924 |
| `copdem-glo-90-n58-e023` | 800 × 1200 | 58–59°N, 23–24°E | `11d204e39c6d792e08dae5af099bab63ff673142a33553fd31025ad6cf5405c6` | 1,618,734 |
| `copdem-glo-90-n58-e024` | 800 × 1200 | 58–59°N, 24–25°E | `df2604e51263c37899a69d66c050f1df2bffc0acad2b106efff0b2e6e02fbed3` | 3,351,805 |
| `copdem-glo-90-n58-e025` | 800 × 1200 | 58–59°N, 25–26°E | `b9e4cd2926675294bac829c5a71f196e61575e8e8466a050d950f3a3d8e09851` | 3,758,368 |
| `copdem-glo-90-n58-e026` | 800 × 1200 | 58–59°N, 26–27°E | `44d407f55cf81aefe752115b06125b5d7eb32c2857c02c4a4ce7450a36a4c431` | 3,639,322 |
| `copdem-glo-90-n58-e027` | 800 × 1200 | 58–59°N, 27–28°E | `c9a5f5e20a7595dcf8f59f6bcc01c14d47fb53744e522436c40dc6aaf844b1d5` | 2,079,733 |
| `copdem-glo-90-n59-e022` | 800 × 1200 | 59–60°N, 22–23°E | `ea0c10875f5c5e0c05e10965883e673fbf95bb9fe8ecc6d87eae9db06dc8bd33` | 369,886 |
| `copdem-glo-90-n59-e023` | 800 × 1200 | 59–60°N, 23–24°E | `35c302eb50ec9b7197d2b4308a20be4330867f953f994017b90f86ee47fdf47d` | 1,108,371 |
| `copdem-glo-90-n59-e024` | 800 × 1200 | 59–60°N, 24–25°E | `6ba823b2319b044912ea91f7fabf5996dd6b3fcc5514b443b9829940d4ceff03` | 1,790,434 |
| `copdem-glo-90-n59-e025` | 800 × 1200 | 59–60°N, 25–26°E | `50ddade2af4e578ddaad21bb9cc825b922f06f15c527b0a4231fcfa3b211e689` | 2,126,782 |
| `copdem-glo-90-n59-e026` | 800 × 1200 | 59–60°N, 26–27°E | `7400c6eff3a0f1c44a4a66c7581a8168f83c5f4e27756382796215ef88487384` | 2,041,639 |
| `copdem-glo-90-n59-e027` | 800 × 1200 | 59–60°N, 27–28°E | `1cc666f3917f7c28bd5610fc2c9b409ea573b9ae5f8705b12d5771dba0a90328` | 1,668,119 |
| `copdem-glo-90-n59-e028` | 800 × 1200 | 59–60°N, 28–29°E | `89e10c39b841c7bdbfb1b6e67da9532e3fd84896ba74890a8ca67871bcf5c884` | 2,905,234 |
| `copdem-glo-90-n63-w023` | 600 × 1200 | 63–64°N, 22–23°W | `dd683e5a8065d9c231039635693f5246bbc225b0484c9f7542ad8937d11e769f` | 410,009 |
| `copdem-glo-90-n63-w022` | 600 × 1200 | 63–64°N, 21–22°W | `1debb658de555ce6ad1a6530ff2653454107927cb113c178f5972563b4f3c810` | 474,807 |
| `copdem-glo-90-n63-w021` | 600 × 1200 | 63–64°N, 20–21°W | `301241c04e31be01fa53052a15a69c7d0e1e72ae031a934e0cd8a609a823af81` | 1,007,970 |
| `copdem-glo-90-n63-w020` | 600 × 1200 | 63–64°N, 19–20°W | `fe6e62ab33a2a7afbd3366c890e2b5c4bc2f97461ff2759df0d05ad32a8487c8` | 1,471,983 |
| `copdem-glo-90-n63-w019` | 600 × 1200 | 63–64°N, 18–19°W | `a33042a2c0ba4ebe55fe6daa3edc16d941416996deb467dd21d866b29238d9ca` | 1,597,972 |
| `copdem-glo-90-n63-w018` | 600 × 1200 | 63–64°N, 17–18°W | `bacb0149628e4200c217d4cf5e0d5b32c83caddd307935928624c3635d926817` | 796,334 |
| `copdem-glo-90-n63-w017` | 600 × 1200 | 63–64°N, 16–17°W | `47ecc135e9b0a9d17d25f48e660799b0cda6929e500e1dad9ad8306384133208` | 328,633 |
| `copdem-glo-90-n64-w025` | 600 × 1200 | 64–65°N, 24–25°W | `45d9bd46923835e49c7a0b7656969479cbda3b36684a62e73fa1c8cf354ead5f` | 58,754 |
| `copdem-glo-90-n64-w024` | 600 × 1200 | 64–65°N, 23–24°W | `967d411439d71afec58b00484c8b618a0caf9e6623ba6780e353418257f99bfa` | 500,573 |
| `copdem-glo-90-n64-w023` | 600 × 1200 | 64–65°N, 22–23°W | `1dd7d06e17b71316bc4b0836f41ff9a85b05be28e7b1554f80c704677dd3372d` | 1,067,983 |
| `copdem-glo-90-n64-w022` | 600 × 1200 | 64–65°N, 21–22°W | `e9822927ff7a73db209e65541583f18862ee1ecf71269cf06a3f7c6bd16f4c7a` | 2,693,734 |
| `copdem-glo-90-n64-w021` | 600 × 1200 | 64–65°N, 20–21°W | `31758717bf8e569f67a088f840787565966ae83e4a72737338f565df45f62648` | 2,640,430 |
| `copdem-glo-90-n64-w020` | 600 × 1200 | 64–65°N, 19–20°W | `67fe71bd8a59599217229990fafa3e06801c8298a92add0b33ac235125f9d982` | 2,590,983 |
| `copdem-glo-90-n64-w019` | 600 × 1200 | 64–65°N, 18–19°W | `47159e153ca007ec69aba019057837c62b25f78db9ab8863152972ec4dc8c1c7` | 2,483,708 |
| `copdem-glo-90-n64-w018` | 600 × 1200 | 64–65°N, 17–18°W | `f56d50f2100743ccc3c542f5d421f7362fe3b8e3718d4e8e750b3e79aa4b5c07` | 2,436,160 |
| `copdem-glo-90-n64-w017` | 600 × 1200 | 64–65°N, 16–17°W | `a7769c37314e6237475e37f1a8a988b6dc091c4edb0891f7dd93bc0b8bca2c68` | 2,381,648 |
| `copdem-glo-90-n64-w016` | 600 × 1200 | 64–65°N, 15–16°W | `501be86e24618da64b707d3a2881380ad763eb18a23329048daf7e405a473440` | 2,087,310 |
| `copdem-glo-90-n64-w015` | 600 × 1200 | 64–65°N, 14–15°W | `fa951a491113599a6a2ef11bba31923bcc60b3d9b1a008f5be3a06c480c8cac5` | 1,442,206 |
| `copdem-glo-90-n64-w014` | 600 × 1200 | 64–65°N, 13–14°W | `9df70646f83c9e8314515dddebc1794d9773af063bfef0b44d62d824f34ac786` | 186,560 |
| `copdem-glo-90-n65-w025` | 600 × 1200 | 65–66°N, 24–25°W | `49526566c992d665e9f1714fe98e8c017a815708ffe0454be36a29a702a5872e` | 252,092 |
| `copdem-glo-90-n65-w024` | 600 × 1200 | 65–66°N, 23–24°W | `fefee9cfadab4e1671b43eb00ef8b324b901e22dc9ec05204d5a188b323b1d1a` | 1,306,580 |
| `copdem-glo-90-n65-w023` | 600 × 1200 | 65–66°N, 22–23°W | `0cfc3f63588abfe1f92df75df795487a39e260bb0c7577ae63b9234eee2b2151` | 1,846,405 |
| `copdem-glo-90-n65-w022` | 600 × 1200 | 65–66°N, 21–22°W | `9ee3cd6dcab892e7d2b928ae389769062d631c0feed4736dfb6cca7623245c52` | 2,155,548 |
| `copdem-glo-90-n65-w021` | 600 × 1200 | 65–66°N, 20–21°W | `28c163ce118f35bc1408179cbfb1125738a1f5f0cd64c8e78de7eaea0217165e` | 2,058,697 |
| `copdem-glo-90-n65-w020` | 600 × 1200 | 65–66°N, 19–20°W | `808c57a80ad15e3440991b615eb66aef4e06c292a02f06b874d2e79e212aa7c4` | 2,564,204 |
| `copdem-glo-90-n65-w019` | 600 × 1200 | 65–66°N, 18–19°W | `1fc5b66adfba74456b633831a0b45f4eb37e48ae366e3b0b88a55c1e0f2a3c04` | 2,700,834 |
| `copdem-glo-90-n65-w018` | 600 × 1200 | 65–66°N, 17–18°W | `05b7bc614ef8717d5fb89e661ecc2f090fe004ce1f1c2b51da644211505a75c2` | 2,718,532 |
| `copdem-glo-90-n65-w017` | 600 × 1200 | 65–66°N, 16–17°W | `54183e42e7d0fd5f5fe193fb441c0452fbe93c8d6ee226b42e211a3ed8e2289f` | 2,602,798 |
| `copdem-glo-90-n65-w016` | 600 × 1200 | 65–66°N, 15–16°W | `f987360e45cb0d7c300ec133b96b7296b9d8698d882af56c679fa606b42fd350` | 2,658,036 |
| `copdem-glo-90-n65-w015` | 600 × 1200 | 65–66°N, 14–15°W | `cf76670f2ea52d3cf8a7ec7a6f00dac4512c5fb95ff2462c1e5cc618473717c2` | 2,262,430 |
| `copdem-glo-90-n65-w014` | 600 × 1200 | 65–66°N, 13–14°W | `fb57483a2b23c4716df7ae94a77dd669981ee9c9c980821557160f64b1621a94` | 661,380 |
| `copdem-glo-90-n66-w024` | 600 × 1200 | 66–67°N, 23–24°W | `72f0fe6f7544e905d92b7735cff07c5b51f35632e34da99648cc872df1afd362` | 409,385 |
| `copdem-glo-90-n66-w023` | 600 × 1200 | 66–67°N, 22–23°W | `2b5c7a16f4ce1071ae478b6fbc7aa0c4660e637efda3b7da2f296124a44a920d` | 942,839 |
| `copdem-glo-90-n66-w022` | 600 × 1200 | 66–67°N, 21–22°W | `b2a98d4b2a2595d37ff4ce1aacdc51a980d3ac4628b884caa95ae744842610c3` | 277,519 |
| `copdem-glo-90-n66-w021` | 600 × 1200 | 66–67°N, 20–21°W | `38d81b689bbf375630c6b8e22925988ed0cd6c291f3793391b1edac8de4a9fd3` | 174,398 |
| `copdem-glo-90-n66-w020` | 600 × 1200 | 66–67°N, 19–20°W | `c1da7877d0ee305d1f64e41e8aad9db1d43267b6797d5f13241ecb8848c5e866` | 181,538 |
| `copdem-glo-90-n66-w019` | 600 × 1200 | 66–67°N, 18–19°W | `aba32c5f98a0ed0f7b15376defcb80068ff8f7b7914cd01c2b558961461aa503` | 438,378 |
| `copdem-glo-90-n66-w018` | 600 × 1200 | 66–67°N, 17–18°W | `533dbbb67143d988d4be6a58ce1e0acbe7239934db891afa5cb87aa25d88aa7a` | 349,255 |
| `copdem-glo-90-n66-w017` | 600 × 1200 | 66–67°N, 16–17°W | `2f73c685af206b1e3928a6ae7dcfcff64fbfdc64d31d34c459645adc2b9173de` | 1,011,364 |
| `copdem-glo-90-n66-w016` | 600 × 1200 | 66–67°N, 15–16°W | `e4f63d3a40fb3c906a026e886746401cd4ba1fdb6eb8ff55a11a871147979fbb` | 882,513 |
| `copdem-glo-90-n66-w015` | 600 × 1200 | 66–67°N, 14–15°W | `9728de666506a14e2ea7479e15c6f8ca590bbefeead65d3a64c5379b1d26aff7` | 184,403 |

## swisstopo-dhm25-200m

| Property | Value |
| --- | --- |
| Grid | 1 926 × 1 201 (200 m) |
| Extent | LV03 coordinates 479 900–865 100 m E, 61 900–302 100 m N |
| CRS | EPSG:21781 (LV03 / LN02) |
| Pixel type | Float32, single band, ESRI ASCII Grid in the source ZIP |
| Nodata | −9 999 |
| ZIP size | 45 069 784 bytes |
| ZIP SHA-256 | `9d283f198f829202e09859954d54553001f7ff1a79e3614ad47ac9cd98641887` |

This is the [official DHM25 / 200m national product][swisstopo-dhm25] from the Federal Office of
Topography swisstopo, not the Wabern or Matterhorn sample files (whose use in products is
prohibited). The pinned official archive contains `DHM200.asc`; that member is the 18 153 345-byte
grid used by the renderer. The archive metadata identifies EPSG:21781; the grid header supplies its
dimensions, 200 m cells and −9 999 nodata value. swisstopo permits free redistribution, modification
and commercial use under its [OGD terms][swisstopo-terms], with a source credit required. Figures
carry `©swisstopo`.

## natural-earth-10m-lakes-v5.0.0

| Property | Value |
| --- | --- |
| Features | 1 355 |
| CRS | OGC:CRS84 (longitude, latitude) |
| Size | 5 043 554 bytes |
| SHA-256 | `2d036f53dedec578001c5c30c2959ee7d4eebc1306900fa4367c49929ec8f2d9` |

[Natural Earth lakes and reservoirs][ne-lakes], version 5.0.0, are used as contextual map water
and clipped to the selected country. Natural Earth data is public domain; no attribution is
required.

## natural-earth-10m-rivers-lake-centerlines-v5.0.0

| Property | Value |
| --- | --- |
| Features | 1 455 |
| CRS | OGC:CRS84 (longitude, latitude) |
| Size | 7 307 743 bytes |
| SHA-256 | `bb854a900ecbd3b408df46d5e16e3e0f974ba55993f9d8b5c26e855273c0905a` |

[Natural Earth rivers and lake centerlines][ne-rivers], version 5.0.0, are used as contextual water.
The published base file includes its Europe supplemental ranks 9–12, which Natural Earth documents
as public domain with optional European Commission attribution. The separate Europe and North
America supplement downloads are not registered here. No attribution is required.

## natural-earth-10m-roads-v5.0.0

| Property | Value |
| --- | --- |
| Features | 56 600 |
| CRS | OGC:CRS84 (longitude, latitude) |
| Size | 50 474 096 bytes |
| SHA-256 | `66a0c7b438e92fd124822cc5921cfa11042f48c294ade5e0f03f2c6640fd0248` |

[Natural Earth roads][ne-roads], version 5.0.0, provide country-clipped major-road features for the
eight approved countries; no North American country is in that set. The published file contains
North American features from XNR Productions, but those features are outside the eight map extents
and are not retained in any figure. Natural Earth documents its base data as public domain and
requires no attribution. The separate North America supplement download is not registered.

## population-count-2020-30arcsec

| Property | Value |
| --- | --- |
| Grid | 43200 × 21600 (30 arc-second, 1/120°) |
| Extent | whole globe, origin (−180°, 90°) |
| CRS | EPSG:4326 (WGS 84) |
| Pixel type | Float32, single band, LZW compressed |
| Nodata | −3.40282306073709653e+38 (Float32, two ulps above −max) |
| Size | 428 465 215 bytes (409 MiB) |
| SHA-256 | `956993aa500774aed548c8e1af1a3a68fc164577be82ca799d4ae8568d445e9d` |
| Land cells | 222 669 928 of 933 120 000 (182 358 616 populated, 40 311 312 zero) |
| World total | 7 757 982 599.32 persons |
| Largest cell | 602 380.375 persons |

Every value above was measured from the file, not copied from a datasheet. The total is a compensated
(Neumaier) sum: over 933 120 000 additions into a running 7.8e9, where one ulp is 1.9e-6, a naive f64
accumulator lands 0.15 low — it gives 7 757 982 599.17, and the file is not what changed.

**It is [Gridded Population of the World, Version 4.11 (GPWv4.11): Population Count Adjusted to Match
the 2015 Revision of UN WPP Country Totals, Revision 11][gpw-adj]**, year **2020**, 30 arc-second
GeoTIFF — CIESIN, Columbia University, distributed by NASA SEDAC, DOI
[10.7927/H4PN93PB][gpw-doi]. SEDAC's own host is gone and the DOI is the route
that survived it. It is the WPP-adjusted variant rather than the unadjusted [Population Count][gpw-raw]:
the two differ only in values, and their catalogued maxima of 602 380 and 627 597 are what the measured
`Largest cell` identifies this copy by — the file's own `GDALMetadata` is generic and claims none of it.

GPWv4.11 is released under [CC BY 4.0][cc-by]. Reuse, including commercial, requires attribution, so any
published map or figure derived from this raster carries the citation:

> Center for International Earth Science Information Network — CIESIN — Columbia University. 2018.
> *Gridded Population of the World, Version 4 (GPWv4): Population Count Adjusted to Match 2015
> Revision of UN WPP Country Totals, Revision 11.* Palisades, NY: NASA Socioeconomic Data and
> Applications Center (SEDAC). <https://doi.org/10.7927/H4PN93PB>

## population-count-2020-10arcmin

| Property | Value |
| --- | --- |
| Grid | 2160 × 1080 (10 arc-minute, 1/6°) |
| Extent | whole globe, origin (−180°, 90°) |
| CRS | EPSG:4326 (WGS 84) |
| Pixel type | Float32, single band, LZW compressed |
| Nodata | −3.40282306073709653e+38 (Float32, two ulps above −max) |
| Size | 2,332,874 bytes |
| SHA-256 | `cf38289019a887369663ca18003bd4b39aa21e9ed574b0d66a61b3626ce9a767` |
| Measured world total | 7 757 982 599.77 persons |

This project derives the raster directly from [`population-count-2020-30arcsec`](#population-count-2020-30arcsec), whose exact SHA-256 is recorded beside the derived identity in [`registry.toml`][registry]. Each output cell sums its 20 × 20 source-cell block, retaining partial blocks at grid edges. Nodata and negative source values contribute zero. Aggregates are stored as Float32; the measured total differs from the source by 0.446 persons, within the conservative bound for per-cell Float32 rounding and f64 block summation.

The source is GPWv4.11's adjusted 2020 population count described above, with the same CC BY 4.0 terms and attribution. The derived GeoTIFF is published as [`population-count-2020-10arcmin.tif`][coarse-population-asset]. It is not resampled from another derived raster.

This is the built-in world-calculation default. Select the native 30 arc-second source explicitly with
`--dataset population-count-2020-30arcsec`. On an Apple M2 Pro, the GH-223 comparison built equal
2160 × 1080 tables from the coarse raster and from the native source at decimation 20. The coarse
table build plus three circle searches averaged 1.872 seconds, compared with 16.305 seconds from the
native source; all three measured radii and centres matched at 10%, 50% and 90% population shares.
This result describes those measured shares; the [issue records the timing method and full
comparison][coarse-comparison].

The measurements pass the pre-recorded default-adoption bar in #223; no answer-change condition
remains open for this default.

## coastline-1to110m

| Property | Value |
| --- | --- |
| Geometry | 134 LineStrings, 5128 vertices |
| Extent | whole globe, −180° to 180°, −85.609038° to 83.645130° |
| CRS | `urn:ogc:def:crs:OGC:1.3:CRS84`, the file's own declaration (EPSG:4326, axes as longitude then latitude) |
| Size | 136.6 KiB, one line |
| SHA-256 | `851f581ff5ffb844deed8ae1a9ce22e3c4bb3d74fa342cadb5d8e39b41ae7c3c` |

Measured from the file. The declared `bbox` reads `180.00000044181` at its eastern edge, 5 cm past the
antimeridian, and the vertices themselves stop at 180, so nothing relies on the declaration.

**It is [Natural Earth][ne]'s 1:110m physical coastline**, from the vector distribution repository at tag
**[v5.1.2][ne-tag]**, path `geojson/ne_110m_coastline.geojson` — stored here under the heading above,
which is the only difference between the two. Committed byte-for-byte as that tag serves it, which is
what makes the checksum above something a reader can check rather than a record of one download:

```sh
curl -sL https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_110m_coastline.geojson \
  | shasum -a 256
```

GeoJSON rather than the shapefile the same data is published as: `json` needs no reader beyond the
standard library, where a shapefile would put a driver in the dependency tree to draw a coastline with.

**Natural Earth is in the public domain.** Its [terms of use][ne-terms] ask for no permission, fee or
attribution, so a figure over this basemap carries the raster's citation alone. Crediting Natural Earth
too is welcomed by the project and is not done, for the reason the citation's wording is checked at all:
what a figure says about its sources should be what those sources ask for and nothing else.

## populated-places-1to10m

| Property | Value |
| --- | --- |
| Geometry | 7,342 Point features |
| Coordinates | longitude, latitude (WGS 84) |
| Size | 19,359,003 bytes |
| SHA-256 | `9b8e3de09048ef00dfc70357dbb9fa324493f214b5e0ae4daf1aa79a8d10116b` |

**Natural Earth 10m populated places**, in GeoJSON from its vector distribution repository at tag
**[v5.1.2][ne-tag]**, file `geojson/ne_10m_populated_places.geojson`. It is the publisher's 10m populated
places layer, preserved byte-for-byte; it has point coordinates and no raster grid, nodata value or cell
size. The `NAME` and `ADM0NAME` properties supply the settlement and country labels.

The upstream file is available anonymously, but its 19 MB size is above the repository's committed-file
limit, so this exact copy is published in a data-only release and fetched by `mise run data:get`. The
release asset is [`populated-places-1to10m.geojson`][populated-places-asset].

Natural Earth is in the public domain. Its [terms of use][ne-terms] require no attribution, so settlement
names add no credit to a figure.

## gisco-countries-2024-1m

| Property | Value |
| --- | --- |
| Geometry | 263 MultiPolygon country features |
| Coordinates | longitude, latitude (WGS 84) |
| CRS | EPSG:4326 (GeoJSON declares `urn:ogc:def:crs:EPSG::4326`) |
| Feature identifier | `CNTR_ID`, unique for all 263 features in this snapshot |
| Size | 58,878,461 bytes |
| SHA-256 | `556a81995108778335b8551f0d8929e86dcbcaab206ba512ba12928eafce505f` |

**GISCO Countries 2024, 1:1,000,000**, from the [publisher's country product][gisco-countries].
The publisher serves one worldwide GeoJSON file. The 2024 product has received later corrections;
the checksum above identifies the exact bytes selected here, and `CNTR_ID` identifies each feature
within that snapshot. The publisher offers no DOI or validated persistent identifier for these exact
bytes. The territorial interpretation follows [ADR 0015][adr-gisco]:
each published feature is retained as one identity, including multipart countries, separate
dependencies and the publisher's disputed-area treatment. The nine invalid source geometries remain
unchanged; country-mask work must decide their behavior explicitly.

GISCO's terms permit use and distribution only for non-commercial purposes, require acknowledgment
of EuroGeographics and UN-FAO, and require the source geometry to remain at 1:1,000,000 or smaller.
This input is unchanged at 1:1,000,000. It is not covered by this repository's MIT software licence.
The required English publication notice is **© EuroGeographics for the administrative boundaries**.

## CNTR_BN_01M_2024_4326_COASTL

| Property | Value |
| --- | --- |
| Geometry | 212 MultiLineStrings, 1,103,873 vertices |
| Extent | −180° to 180.0000000000001°, −90° to 83.66551412700005° |
| CRS | `urn:ogc:def:crs:EPSG::4326` (longitude, latitude) |
| Size | 45,779,839 bytes |
| SHA-256 | `64c6c77885ef52384a3a9f2f8228d092eaf10068e00ac223b4000007b2b5af1f` |

**GISCO Countries 2024, 1:1,000,000 coastline**, the publisher's
[`CNTR_BN_01M_2024_4326_COASTL.geojson`][gisco-coastline-source] snapshot. Its revision is pinned
by the SHA-256 above; the source catalogue and exact downloaded date are recorded in
[`registry.toml`][registry]. It remains at 1:1,000,000 and is fetched from the publisher through
`mise run data:get -- --dataset CNTR_BN_01M_2024_4326_COASTL`, which verifies the byte count and
checksum before placing it. Maps using this data must show **© EuroGeographics for the administrative
boundaries**. GISCO permits use only for non-commercial purposes; the data is not covered by this
repository's MIT software licence.

### Obtaining it

`mise run data:get` downloads the verified snapshot without an account from this project's
[data-only release][gisco-asset]. The release copy preserves the publisher's bytes unchanged because
the publisher's direct URL can serve corrected content under the same filename. To compare the
current publisher file with this snapshot:

```sh
curl -fL https://gisco-services.ec.europa.eu/distribution/v2/countries/geojson/CNTR_RG_01M_2024_4326.geojson \
  -o /tmp/CNTR_RG_01M_2024_4326.geojson
shasum -a 256 /tmp/CNTR_RG_01M_2024_4326.geojson
```

The published file is renamed to `gisco-countries-2024-1m.geojson` in `data/boundaries/`; no
extraction, simplification or reprojection is performed. Compare the resulting digest with the row in
[`registry.toml`][registry]. The registry's `fetch_url` is the stable project release asset;
the publisher URL above is provenance and a way to reproduce the selected bytes while it still
serves them.

## Prepared country combinations

The built-in combinations pair the 30 arc-second GPW population source with either GISCO Countries
2024 (`gpw-2020-gisco-2024`, the default) or GADM 4.1 (`gpw-2020-gadm-4.1`). Each pairing provides
factor 1 (30 arc-seconds) and factor 2 (1 arcminute); factor 2 is the default. `country fetch` installs
the selected prepared bundle. The manifests retain all 263 source feature IDs, including Vatican City,
which has no owned population cells in these bundles.

| Pairing and factor | Release assets | Bundle directory bytes |
| --- | --- | ---: |
| GISCO, factor 1 | [`data-v5`](https://github.com/turboBasic/PopCircles/releases/tag/data-v5) | 7,596,273,923 |
| GISCO, factor 2 | [`data-v6`](https://github.com/turboBasic/PopCircles/releases/tag/data-v6) | 1,901,567,393 |
| GADM, factor 1 | [`data-v7`](https://github.com/turboBasic/PopCircles/releases/tag/data-v7) | 7,177,194,888 |
| GADM, factor 2 | [`data-v8`](https://github.com/turboBasic/PopCircles/releases/tag/data-v8) | 1,796,924,439 |

The #213 release comparison found matching world radii at factors 1 and 2 for the tested 20%, 50%,
and 90% shares, and matching radii for the required GISCO country cases. Centres and achieved shares
shift slightly with resolution. GADM has more vertices for several microstates, but that geometry
count is not evidence of greater accuracy. The full case results, timings, input digests, and bundle
integrity evidence are recorded in the [#213 acceptance record](https://github.com/turboBasic/PopCircles/issues/213#issuecomment-6413726620).

## gadm41-adm0

| Property | Value |
| --- | --- |
| Geometry | 263 ADM0 MultiPolygon features |
| Coordinates | longitude, latitude (WGS 84) |
| CRS | EPSG:4326 |
| Feature identifier | `GID_0`, unique and retained from the GADM 4.1 source |
| Size | 1,155,550,194 bytes |
| SHA-256 | `4d005953e25108f475bcedb00f2ad4409f5d61d3c8d4ae7fd6e523d593c6c4b9` |

This is layer `ADM_0` from the [GADM 4.1 global levels archive][gadm-source], exported to GeoJSON
with only `GID_0` and `COUNTRY` properties. Coordinates are kept at 15 decimal places; no geometry
simplification or reprojection is performed. `GID_0` remains the source identity; it is not mapped to
GISCO identifiers.

GADM's [terms][gadm-license] permit academic and other non-commercial use; redistribution or
commercial use requires prior permission. The maintainer confirmed permission for this project's
redistribution of the source snapshot and derived prepared bundles on 2026-10-26. Austria's source
data is under CC BY-SA 2.0. The required attribution is **GADM database, version 4.1; Austria data
© Government of Austria, CC BY-SA 2.0.** This data is not covered by the repository's MIT software
licence.

### Obtaining and reproducing it

`mise run data:get` downloads the exact GeoJSON snapshot from this project's [data-only release][gadm-asset].
To reproduce the export from the publisher's global archive, extract `gadm_410-levels.gpkg` from
the ZIP, then run:

```sh
ogr2ogr -f GeoJSON -select GID_0,COUNTRY -lco COORDINATE_PRECISION=15 \
  /tmp/gadm41-adm0.geojson /tmp/gadm_410-levels.gpkg ADM_0
shasum -a 256 /tmp/gadm41-adm0.geojson
```

The digest above identifies the exact export. The registry's `fetch_url` is the stable project
asset; the publisher URL is the source archive used to reproduce it.

<!-- Links -->

[adr-gisco]: ../docs/decisions/0015-country-identities-follow-gisco-country-features.md
[cc-by]: https://creativecommons.org/licenses/by/4.0/
[coarse-population-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v4/population-count-2020-10arcmin.tif
[coarse-comparison]: https://github.com/turboBasic/PopCircles/issues/223#issuecomment-6404921257
[contributing-data]: ../CONTRIBUTING.md#verifying-a-published-dataset
[gadm-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v7/gadm41-adm0.geojson
[gadm-license]: https://gadm.org/license.html
[gadm-source]: https://geodata.ucdavis.edu/gadm/gadm4.1/gadm_410-levels.zip
[gisco-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v3/gisco-countries-2024-1m.geojson
[gisco-coastline-source]: https://gisco-services.ec.europa.eu/distribution/v2/countries/geojson/CNTR_BN_01M_2024_4326_COASTL.geojson
[gisco-countries]: https://gisco-services.ec.europa.eu/distribution/v2/countries/countries-2024-files.html
[gpw-adj]: https://www.earthdata.nasa.gov/data/catalog/sedac-ciesin-sedac-gpwv4-apct-wpp-2015-r11-4.11
[gpw-doi]: https://doi.org/10.7927/H4PN93PB
[gpw-raw]: https://doi.org/10.7927/H4JW8BX5
[ne]: https://www.naturalearthdata.com/downloads/110m-physical-vectors/
[ne-tag]: https://github.com/nvkelso/natural-earth-vector/releases/tag/v5.1.2
[ne-terms]: https://www.naturalearthdata.com/about/terms-of-use/
[etopo]: https://www.ncei.noaa.gov/products/etopo-global-relief-model
[etopo-file]: https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif
[etopo-15s-file]: https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/15s/15s_surface_elev_gtif/ETOPO_2022_v1_15s_N45E000_surface.tif
[copdem-glo-90-file]: https://copernicus-dem-90m.s3.amazonaws.com/Copernicus_DSM_COG_30_N42_00_E001_00_DEM/Copernicus_DSM_COG_30_N42_00_E001_00_DEM.tif
[swisstopo-dhm25]: https://www.swisstopo.admin.ch/en/height-model-dhm25-200m
[swisstopo-terms]: https://www.swisstopo.admin.ch/en/terms-of-use-free-geodata-and-geoservices
[ne-lakes]: https://www.naturalearthdata.com/downloads/10m-physical-vectors/10m-lakes/
[ne-rivers]: https://www.naturalearthdata.com/downloads/10m-physical-vectors/10m-rivers-lake-centerlines/
[ne-roads]: https://www.naturalearthdata.com/downloads/10m-cultural-vectors/roads/
[populated-places-asset]: https://github.com/turboBasic/PopCircles/releases/download/data-v2/populated-places-1to10m.geojson
[registry]: registry.toml

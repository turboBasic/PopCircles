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

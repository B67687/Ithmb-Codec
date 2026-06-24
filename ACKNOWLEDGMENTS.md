# Acknowledgments

Every known open-source `.ithmb` implementation (30+ total) was surveyed across GitHub, GitLab, Codeberg, SourceHut, Bitbucket, Gitee, and SourceForge during development.

---

## Open-Source Implementations

### Directly incorporated (MIT-licensed)

These projects' source code directly informed our decoders. All are MIT-licensed.

| Project      | Author  | URL                                     | Contribution                                                                              |
| ------------ | ------- | --------------------------------------- | ----------------------------------------------------------------------------------------- |
| **iOpenPod** | Savi    | https://github.com/TheRealSavi/iOpenPod | Most complete modern codec. 50+ format entries, encode + decode for all iPod generations. |
| **ithmbrdr** | cyianor | https://github.com/cyianor/ithmbrdr     | F1067 YCbCr 4:2:0 with correct BT.601 coefficients; padded frame structure.               |

### Format references (clean-room — no code copied)

Format behavior was studied from these implementations; no code was copied.

| Project                           | Author             | URL                                                                            | Language   | License | What it contributed                              |
| --------------------------------- | ------------------ | ------------------------------------------------------------------------------ | ---------- | ------- | ------------------------------------------------ |
| Keith's iPod Photo Reader         | Keith Wiley        | https://github.com/kebwi/Keiths_iPod_Photo_Reader                              | C++        | —       | Original 2005 RE, 13 decode methods              |
| andrewmalta/ithmb                 | Andrew Malta       | https://github.com/andrewmalta/ithmb                                           | Python     | —       | F1019 CLCL packed-chroma layout                  |
| Gaurav-Phogat/F1007               | Gaurav Phogat      | https://github.com/Gaurav-Phogat/ithmb-extractor-F1007                         | Python     | —       | F1007 480×864 MSB-replication scaling            |
| keyj.emphy.de blog                | Jeff Luyten (KeyJ) | https://web.archive.org/web/2024*/https://keyj.emphy.de/an-ipod-hackers-diary/ | —          | —       | ArtworkDB RE, RGB565 byte-swapped artwork        |
| repear                            | worstje            | https://github.com/worstje/repear                                              | Python     | —       | Complete format → dimension encoder table        |
| podsyncr                          | tbutter            | https://github.com/tbutter/podsyncr                                            | Java       | —       | iPod Nano 2G syncer, configurable endianness     |
| libgpod/gtkpod                    | gtkpod team        | https://github.com/gtkpod/libgpod                                              | C          | LGPL    | 22 format variants, ArtworkDB/PhotoDB parser     |
| shinyquagsire23 gist              | shinyquagsire23    | https://gist.github.com/shinyquagsire23/5ac38487b4c8f9252e78e0275814c90b       | C          | —       | F1093 = 512×512 RGB565 decode                    |
| Steee29/ithmb_converter           | Steee29            | https://github.com/Steee29/ithmb_converter                                     | Python     | —       | iOS 1.x format table (3004=55×55, etc.)          |
| wrinklykong/pyithmb               | wrinklykong        | https://github.com/wrinklykong/pyithmb                                         | Python     | —       | CLCL nibble-chroma decoder                       |
| thomas-alrek/iPod-photo-database  | thomas-alrek       | https://github.com/thomas-alrek/iPod-photo-database                            | JavaScript | —       | Node.js Photo Database + ithmb→JPEG              |
| epireyn/ithmb-rs                  | epireyn            | https://gitlab.com/epireyn/ithmb-rs                                            | Rust       | GPLv3   | Not incorporated; format reference only          |
| MasterCard007/ithmb2jpg-converter | MasterCard007      | https://github.com/MasterCard007/ithmb2jpg-converter                           | Python     | —       | Batch converter                                  |
| devm18426/mhfd_extractor          | devm18426          | https://github.com/devm18426/mhfd_extractor                                    | Python     | —       | MHFD chunk → UYVY interlaced format              |
| pygpod                            | Bionded            | https://github.com/Bionded/pygpod                                              | Python     | —       | Pure-Python libgpod port                         |
| Keipydesu/ipod-convert            | Keipydesu          | https://github.com/Keipydesu/ipod-convert                                      | Python     | MIT     | F1067 padded YCbCr support                       |
| **clickwheel**                    | dstaley            | https://github.com/dstaley/clickwheel                                          | C#         | MIT     | 40+ format ID table, per-generation device capabilities, full ArtworkDB read/write |
| **OrgZ**                          | FoxCouncil         | https://github.com/FoxCouncil/OrgZ                                             | C#         | MIT     | IPodCapabilities.cs device tables, ArtworkDB+ithmb read/write, album art sync |
| **podkit**                        | jvgomg             | https://github.com/jvgomg/podkit                                               | TypeScript | MIT     | ArtworkDB CLI diagnostics, offset validation, MHII/MHNI linking integrity checks |
| **gnupod**                        | richvdh            | https://github.com/richvdh/gnupod                                              | Perl       | GPLv2   | Per-device format tables (nano_4g, nano_3g, classic, video, nano, mini), ImageMagick pipeline |
| cardner/playlist-generator        | cardner            | https://github.com/cardner/playlist-generator                                  | TypeScript | —       | Pure-TS iPod DB with detailed iTunesDB dbversion comparison |
| moerdowo/Minpod                   | moerdowo           | https://github.com/moerdowo/Minpod                                             | Swift      | MIT     | iPod sync tool, ArtworkDB ithmb creation         |
| atimevil/Ithmb-Converter          | atimevil           | https://github.com/atimevil/Ithmb-Converter                                    | Python     | MIT     | Korean converter with AI upscaling               |
| yosoyemi/ithmb-converter-a-jpg    | yosoyemi           | https://github.com/yosoyemi/ithmb-converter-a-jpg                              | Python     | MIT     | UYVY big-endian decode, BT.601-like coefficients |
| **Reuhno** (iPod Classic 6G) | Renaud             | https://reddit.com/u/Reuhno                                                     | Python     | —       | First known real F-prefix samples from iPod Classic 6G (F1055, F1060, F1061). Confirmed decoder correctness on real hardware. ArtworkDB analysis with mhni field offsets, stored_width formula (size/2/height), UTF-16LE .ithmb filenames, and iPod Classic 6G generation-specific format table (F1061=56×55, F1055=128×128, F1060=320×320). |

### Lost / unrecoverable

- **iThmbConv** (C, 2007) — Windows CLI tool by "Anywho" (Whirlpool). First working F3008 (iPhone RGB565) and F1067 (Nano YCbCr 4:2:0) decoder. Source lost behind Captcha. [Whirlpool forum thread](https://forums.whirlpool.net.au/archive/661720)

### Commercial / closed-source (reference only)

- [iThmb Converter](https://www.ithmbconverter.com/) — Windows GUI tool
- [File Juicer](https://echoone.com/filejuicer/formats/ithmb) — macOS tool, also provides a non-standard sample file

---

## Firmware Decryption

The iPhone OS 1.x–3.x IPSW DMG encryption keys used for firmware extraction are publicly documented:

- **The iPhone Wiki** — [theiphonewiki.com](https://www.theiphonewiki.com/) — VFDecrypt keys for all iPhone OS 1.x–3.x builds, publicly documented since 2009.
- **xpwn** — [GitHub](https://github.com/planetbeing/xpwn) — Cross-platform IPSW extraction toolkit (dmg, img3, VFDecrypt).

These keys were used solely for the purpose of extracting `.ithmb` files from 2007–2009 firmware for decoder validation. Only archived firmware images from Apple's public CDN were processed.

---

## Format Documentation Sources

| Source                 | URL                                                                                                             | Content                                                                                                                                                                                         |
| ---------------------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| iLounge "Gory Details" | https://web.archive.org/web/20090120040252/http://forums.ilounge.com/showthread.php?t=66435                     | Per-device format ID table (2005)                                                                                                                                                               |
| iLounge hacking thread | https://web.archive.org/web/20191225184817/https://forums.ilounge.com/threads/hacking-ithmb-file-format.110066/ | Original YUV 4:2:2 RE with working code                                                                                                                                                         |
| keyj.emphy.de blog     | https://web.archive.org/web/2024*/https://keyj.emphy.de/an-ipod-hackers-diary/                                  | ArtworkDB reverse engineering                                                                                                                                                                   |
| iPodLinux Wiki         | http://www.ipodlinux.org/ITunesDB/                                                                              | iTunesDB/ArtworkDB binary spec                                                                                                                                                                  |
| Just Solve wiki        | http://fileformats.archiveteam.org/wiki/IThmb                                                                   | Format overview, sample links                                                                                                                                                                   |
| ithmb.org              | https://ithmb.org                                                                                               | Browser-based decoder                                                                                                                                                                           |
| MyBroadband forum      | https://mybroadband.co.za/forum/threads/iphone-ipad-recover-deleted-images-through-ithmb-files.1298535/         | 3 sample files + reference JPEGs                                                                                                                                                                |
| XnView forum           | https://newsgroup.xnview.com/viewtopic.php?t=32698                                                              | Sample discussion                                                                                                                                                                               |
| Whirlpool forum thread | https://forums.whirlpool.net.au/archive/661720                                                                  | F3008 (iPhone RGB565) and F1067 (Nano YCbCr 4:2:0) first decoders, 4G vs 5G size differences, 500 MB file-split threshold. Contributors: Anywho (iThmbConv), stringpower, machstem, vaz, Dash-2 |

---

## Public Sample File Sources

Live .ithmb file sources used for testing:

- **Jakarade.com** (F00–F08) — [https://www.jakarade.com/inc/Picture/yugioh/iPod Photo Cache/](https://www.jakarade.com/inc/Picture/yugioh/iPod%20Photo%20Cache/) — ✅ 227 files, 210–346 KB each, T-prefix with embedded JPEG+EXIF
- **Florida Atlantic University** (F00–F50) — [https://home.fau.edu/schilitj/web/iPod Photo Cache/](https://home.fau.edu/schilitj/web/iPod%20Photo%20Cache/) — ⚠️ ~500 files, ~672 KB each, directories live but downloads return 404
- **Hungrypoint.com** — [Photo Database](https://hungrypoint.com/Niki/iPod%20Photo%20Cache/Photo%20Database) — ✅ 30 KB index file (no .ithmb)
- **File Juicer sample** (T117.ithmb.zip) — [sample-files/T117.ithmb.zip](https://echoone.com/filejuicer/sample-files/T117.ithmb.zip) — Non-standard file (67,696 bytes `80 10` pattern + 726,344 bytes unknown); correctly rejected by our codec

Dead sources (listed for completeness): `dragonnorth.com` (empty directories), `vhromanov.com` (domain gone), `home.fau.edu/schilitj/` (redirected).

---

## Academic References

No formal academic papers describe the `.ithmb` format itself. The most relevant publication:

> Piccinelli M., Gubian P. (2011). _"Exploring the iPhone Backup Made by iTunes."_ Journal of Digital Forensics, Security & Law, 6(3). [DOI: 10.15394/jdfsl.2011.1099](https://doi.org/10.15394/jdfsl.2011.1099) — Open Access

This paper describes the iTunes backup structure containing .ithmb files.

---

## Color Conversion

- YCbCr → RGB uses **ITU-R BT.601** matrix (JPEG full-range variant), per [Recommendation ITU-R BT.601-7](https://www.itu.int/rec/R-REC-BT.601).
- RGB565 → RGB888 uses **MSB replication** (standard in FFmpeg, libpng, Skia).

---

## Privacy Note

Public sample files are hosted on personal and university servers and may contain personal photographs. **Do not redistribute the .ithmb files themselves.** The synthetic encoder generates test files without privacy concerns.

---

## License Note

The original IthmbDecoder reference implementation (ImageGlass PR [#2316](https://github.com/d2phap/ImageGlass/pull/2316)) was GPL-3.0. This plugin is a clean-room implementation for the v10 SDK ABI, informed by format behavior described in that PR but using no GPL code. The codebase is MIT-licensed — see [LICENSE](LICENSE).

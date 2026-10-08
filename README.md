# Rust Gerber Parser
[![Build status][build-status-badge]][build-status]
[![Crates.io][crates-io-badge]][crates-io]
[![MakerPnP Discord][discord-badge]][discord]

- [Docs (released)](https://docs.rs/gerber-parser/)

This crate implements a `gerber` parser written in rust to be used with the `gerber-types` crate.

## Example

See the `examples` folder of the repository, run it as follows:

    $ cargo run --example example1

## Spec compliance

The gerber specification 2024.05 is the latest version of the Gerber file format that is used for reference.

### Supported gerber features

| Supported | Feature                                        | Notes                                           |
|-----------|------------------------------------------------|-------------------------------------------------|
| ✅         | Comments (G04)                                 |                                                 |
| ✅         | Units (MO)                                     |                                                 |
| ✅         | Format specification (FS)                      |                                                 |
| ✅         | Aperture definition (AD)                       |                                                 |
| ✅         | Standard aperture templates                    |                                                 |
| ✅         | Aperture macros (AM)                           |                                                 |
| ✅         | Select aperture (Dnn)                          |                                                 |
| ✅         | Plot state (G01, G02, G03, G75)                |                                                 |
| ✅         | Operations (D01, D02, D03)                     |                                                 |
| ✅         | Transformations (LP, LM, LR, LS)               |                                                 |
| ✅         | Regions (G36/G37)                              |                                                 |
| ✅         | Block aperture (AB)                            |                                                 |
| ✅         | Step repeat (SR)                               |                                                 |
| ✅         | End-of-file (M02)                              |                                                 |
| ✅         | File attributes (TF)                           |                                                 |
| ✅         | Aperture attributes (TA)                       |                                                 |
| ✅         | Object attributes (TO)                         |                                                 |
| ✅         | Delete attribute (TD)                          |                                                 |
| ✅         | Standard attributes                            | Full support, including .N, .P, .C, .CXxxx, etc |
| ✅         | User defined attributes                        |                                                 |
| ✅         | Comment attributes                             | See spec 2024.5 - 5.1.1, 'G04 #@! ...*          |
| ✅         | Image Transformations (IP, MI, SF, OF, IR, AS) | Yes, see below for details                      |
| ✅         | Legacy/deprecated attributes                   | Partial                                         |

Contributions to improve support welcomed!

### Unicode escapes

String and attribute field values are decoded from Gerber `\uXXXX` and `\UXXXXXXXX`
escapes into Unicode characters. Invalid escapes produce content errors with source context.
Escaped delimiters are decoded after splitting commands and fields.

### Image Transformations

Image transformations have been deprecated since December 2012 (I1 revision).
The intention of these commands is not entirely clear when it comes to rendering, since the spec talks about
step and repeat distances not being coordinate data; However, it doesn't make any sense to have a command called
Mirror Image (MI) where the intention /appears/ to be to mirror the /entire/ image, which cannot be achieved unless you
also mirror step and repeat distances and apertures.  Thus it is advised for downstream crates to apply image
transformations to step and repeat distances and apertures when applying image transformations via (MI, SF, OF, IR).

The related crate 'gerber-viewer' (see below), creates an image transform matrix based on the image transformation
commands and applies them before rendering, but after primitive processing, resulting in a perfectly transformed image
which is always consistent.  However, depending on the intention of the author of the gerber file and the tools they were
using at the time that might not be "correct".  Thus, as per the spec, it's best to avoid these commands entirely to
avoid misinterpretation and/or manufacturing issues.

Refer to gerber spec 2024.05 sections 8.1.5 (IR), 8.1.7 (MI), 8.1.8 (OF), 8.1.9 (SF)


## Related crates

### Gerber Types

A rust crate for definition of gerber types in test.

Crates.io: https://crates.io/crates/gerber-types
Github: https://github.com/MakerPnP/gerber-types

### Gerber Viewer

A rust crate for rendering gerber layers, also uses this crate as a dependency:

Github: https://github.com/MakerPnP/gerber-viewer

## Related projects

For a list of other projects that use this crate you can check the github 'dependents' page.

https://github.com/MakerPnP/gerber-parser/network/dependents

## Timeline

- 2022/06/27 - Initial version by Nemo Adrea, licensed on AGPL.
  The original author used f360, the initial implementation was written to support files it generated.
- 2024/04/23 - Crate went under heavy development to broaden the support for Gerber files, e.g. ones generated by KiCad, DipTrace,
  LibrePcb, etc.
- 2025/05/05 - gerber-types crate transferred to the MakerPnP organization, primary maintainer is now Dominic Clifton
- 2025/06/04 - gerber-parser crate transferred the repository to the MakerPnP organization, primary maintainer is now Dominic Clifton.
- 2025/06/13 - All non-deprecated gerber commands are now supported. License changed to MIT or APACHE.
- 2025/07/11 - Support for deprecated commands added.


## Authors

* Nemo Andrea (Original author)
* Dominic Clifton (Current maintainer)

## License

Available under APACHE *or* MIT licenses, the same as the gerber-types crate.

* [APACHE](LICENSE-APACHE)
* [MIT](LICENSE-MIT)

<!-- Badges -->

[build-status]: https://github.com/makerpnp/gerber-parser/actions/workflows/ci.yml
[build-status-badge]: https://github.com/makerpnp/gerber-parser/workflows/CI/badge.svg
[crates-io]: https://crates.io/crates/gerber-parser
[crates-io-badge]: https://img.shields.io/crates/v/gerber-parser.svg
[discord]: https://discord.gg/ffwj5rKZuf
[discord-badge]: https://img.shields.io/discord/1255867192503832688?label=MakerPnP%20discord&color=%2332c955

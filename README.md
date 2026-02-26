# Pilot Garden

Pilot Garden is a 3D minigame built with Rust and Bevy.

> [!CAUTION]
> The source code and the file names of the source code may contain major spoilers.

![screenshot of pilot garden](readme_screenshot.png)

## System requirements

- GPU: `GeForce GTX 1050 Mobile` (2 GiB VRAM recommended)
- OS: Windows, macOS, or Linux

## Installation

### Precompiled

1. Go to [itch.io](https://carrascomj.itch.io/pilot-garden) or
[Github Releases](https://github.com/carrascomj/pilot_garden/releases/latest)
2. Download the file for your platform.
3. If it's a zip, unzip it and run the binary:
   - Windows: `pilot_garden.exe`
   - Unix/macOS: `pilot_garden`

### From source

1. Instal Rust: https://rust-lang.org/tools/install/

```bash
# Unix-like OS, click on the link above for windows
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Clone this repo and run the game with [Cargo](https://doc.rust-lang.org/cargo/guide/creating-a-new-project.html):

```bash
git clone https://github.com/carrascomj/pilot_garden.git
cd pilot_garden
cargo run --release
```

> This may require additional dependencies. Check the [Bevy setup guide](https://bevy.org/learn/quick-start/getting-started/setup/) if needed.

## License

### Source Code

Copyright 2026 Jorge Carrasco Muriel.

All source code (including shaders in `assets/`) is dual-licensed under:

- [Apache 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

### Assets

Copyright 2026 Jorge Carrasco Muriel.

All none code assets in `assets/` (except shaders and fonts) are licensed under [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/)
> Non-commercial use only.

### Contribution

Any contribution intentionally submitted for inclusion in this work is also dual-licensed under MIT or Apache 2.0, unless you state otherwise.

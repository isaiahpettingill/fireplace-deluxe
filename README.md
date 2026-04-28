# Fireplace Deluxe
A cozy fireplace in your terminal, now ported to Rust

![A gif of what to expect](demo.gif?raw=true "Cozy")

## Install

```bash
cargo install fireplace-deluxe
```

## Build and Run

```bash
cargo build --release
./target/release/fireplace-deluxe
```

## Options
```
Usage: fireplace-deluxe [options]
        -c character    Character to draw the flames. Default is '@'.
        -h              Print this message.
        -f framerate    Set the framerate in frames/sec. Default is 20.
        -t temp         Set the maximum temperature of the flames. Default is 10.
        -w wolfram      Wolfram rule for flicker. Default is 60.
        -r              Print random characters.
        --no-background Disable black background.
        -b, --background-flame  Use background colors for solid flame effect.
        -u              Use decorative unicode (╬, ╳, ░, ▞, 🮿, 𜵯, 🮋, 𜺏).
        -n NUM          Unicode character number (1-8). Default is 1.

Press ^C or q to exit. Use up/down arrows or j/k to change temperature.
```

## Docker
```bash
docker build . -t fireplace:latest
docker run -it --rm fireplace
docker run -it --rm fireplace -t 7
```
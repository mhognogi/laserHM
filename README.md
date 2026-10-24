# LaserHM

Desktop app to design, cut and engrave with GRBL diode lasers, inspired by LightBurn.
Built with Tauri, Rust and Vue. Windows and Linux.

## ⚠️ Safety

Lasers are dangerous. This software moves a machine and turns a laser on.

- Laser light can permanently damage eyes and skin, even when reflected, and it may be invisible.
- Everyone in the room wears safety glasses certified (EN 207) for your laser's wavelength
  (about 450 nm for blue diode lasers).
- Never leave the laser running unattended.
- Engraving and cutting make smoke and fumes: ventilate the room or use an exhaust.
- Never cut PVC or vinyl: they release chlorine gas, harmful to you and to the machine.
- Materials can catch fire: keep a fire extinguisher at hand.
- To stop a job at once: the Stop button, or the Pause key.
- Your laser's manufacturer instructions come first.
- LaserHM is provided as is, without any warranty; you use it at your own risk.

The app shows these warnings at the first start; they are also in Help > Safety.

## Features

- Drawing: rectangles, ellipses, polygons, lines and curves, text with installed fonts;
  move, resize, rotate, edit nodes, Weld / Union / Subtract / Intersection, undo / redo
- Cutting with power, speed and passes; simulation with the laser off; progress and time
- Image engraving: Threshold, Floyd-Steinberg, Jarvis, Stucki or Grayscale, with
  brightness, contrast and gamma
- Material Test: a grid of power and speed combinations
- Camera overlay with calibration
- Projects (.lhm) with images included; the last session comes back at startup
- Claude Desktop can draw on the canvas (MCP); the laser itself stays in your hands

## Requirements

- A laser with a GRBL 1.1 controller, connected over USB (115200 baud), in laser mode (`$32=1`)
- Windows 10 or 11, or Linux (packages built on Ubuntu 22.04; newer versions work too)

### Linux: access to the laser's USB port

The user must be in the `dialout` group, otherwise the app sees the laser but cannot connect:

```sh
sudo usermod -aG dialout $USER
```

then log out and log in again.

### Tested hardware

LaserHM has been tested only on a **Mecpow X3 Pro** with a diode laser, on Windows.
Other GRBL lasers should work, but may differ (homing, origin, baud rate, firmware messages).
The Linux version is built but not tested yet.

If you try it on another machine, your observations and suggestions are welcome: please open an
[issue](https://github.com/mhognogi/laserHM/issues) with your laser's model, what worked and
what did not.

## Building

Needs [Node.js](https://nodejs.org/) and [Rust](https://www.rust-lang.org/tools/install).

```sh
npm install
npm run tauri dev     # run in development
npm run tauri build   # installer in src-tauri/target/release/bundle
```

## License

[MIT](LICENSE) © 2026 Marius Hognogi. Provided as is, without any warranty.

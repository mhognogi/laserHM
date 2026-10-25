# LaserHM

LaserHM is a program to design, cut and engrave with a laser cutter.
It controls lasers with a GRBL controller. It works on Windows. It is free and open source.

## Why LaserHM

### Simple and precise camera calibration

Cut one square. Then click 9 points of the square in the camera image: the 4 corners,
the middles of the 4 sides and the center.
LaserHM corrects each quarter of the square separately.
This also corrects the distortion of the camera lens near the edges.
The camera image then matches the work area. Make the square large: the calibration is exact inside it.

### See the job on the canvas

During a job, the path that the laser cut becomes green on the canvas.
A red dot shows where the laser head is.
LaserHM gets this position from the laser, not from an estimate.
At each new pass, the green path starts again.
A progress bar shows the time of the job and the total time.

### Material Test on the canvas

LaserHM puts the Material Test grid on the canvas. You move or rotate it to the correct place on the material.
Each square of the grid has its own power and speed.
During the job, the laser panel shows the power and the speed of the current square.

### Engrave images with a live preview

The "Adjust Image" window shows the original image and the engraving side by side.
The preview changes immediately when you change a setting.
The canvas shows the image as the laser will engrave it.

### Fast editing

- Turn the mouse wheel over a value to change it: position, size, rotation, power, speed, text size.
- Browse the fonts with the arrow keys. The text on the canvas changes immediately.

### Your work is kept

- When you start LaserHM again, the canvas shows your last work.
- A project file contains the shapes and the images. You can open it on a different computer.

### Draw with Claude Desktop

You can ask Claude Desktop to draw shapes and texts on the canvas.
Claude cannot start the laser: you always start the job.

## What LaserHM can do

- Draw rectangles, ellipses, polygons, lines and texts.
- Cut shapes with a power, a speed and a number of passes that you set.
- Engrave images.
- Burn a Material Test grid to find the best power and speed for a material.
- Show a camera image of the work area.
- Save projects and open them again.

## Download and install

1. Go to the [Releases page](https://github.com/mhognogi/laserHM/releases).
2. Download the file that ends in `x64-setup.exe`.
3. Install LaserHM.

A version for Linux is not available yet.

For all the steps, see the [user guide](docs/user-guide.md#1-install-laserhm).

## Requirements

- A laser with a GRBL 1.1 controller. Connect it with a USB cable.
- The laser mode of the controller must be on (GRBL setting `$32=1`).
- Windows 10 or Windows 11.

## How to use LaserHM

Read the [user guide](docs/user-guide.md).

## Tested lasers

LaserHM is tested only on a **Mecpow X3 Pro** diode laser, on Windows.
Other GRBL lasers can operate differently.

Did you use LaserHM with a different laser? Please tell us in an [issue](https://github.com/mhognogi/laserHM/issues).
Write the model of your laser, what worked and what did not work.

## ⚠️ Safety

> **WARNING:** Laser light can permanently damage your eyes and skin. Reflected light is also dangerous.

- Wear safety glasses for the wavelength of your laser. The glasses must have the EN 207 certification.
  Blue diode lasers have a wavelength of about 450 nm.
- Do not leave the laser alone while it operates.
- Open a window or use an exhaust. Cutting makes smoke.
- Do not cut PVC or vinyl. These materials release chlorine gas.
- Keep a fire extinguisher near the laser.
- To stop a job immediately, click **Stop** or push the **Pause** key on the keyboard.
- Obey the instructions of your laser manufacturer first.
- LaserHM has no warranty. You use it at your own risk.

LaserHM shows these warnings when you start it for the first time. To read them again, click **Help** > **Safety**.

## For programmers

To build LaserHM from the source code, see [docs/building.md](docs/building.md).

## License

[MIT](LICENSE) © 2026 Marius Hognogi.

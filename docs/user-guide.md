# LaserHM user guide

This guide tells you how to use LaserHM with a laser cutter.
LaserHM controls lasers with a GRBL controller.

## Words used in this guide

- **Laser**: the laser cutter machine.
- **Laser head**: the part of the laser that moves and emits the light.
- **Work area**: the surface where the laser can cut. LaserHM shows it as a white rectangle.
- **Canvas**: the large area in the middle of the LaserHM window. It shows the work area.
- **Shape**: a rectangle, an ellipse, a polygon, a line, a text or an image on the canvas.
- **Job**: one cut or engraving of the selected shapes.
- **Top bar**: the bar under the menu. It shows the position and size of the selected shapes.
- **Tool bar**: the column of buttons on the left side.
- **Laser panel**: the column on the right side. It has the Start button.

## 1. Install LaserHM

### Before you start

- A computer with Windows 10 or Windows 11.
- An internet connection.

### Procedure

1. Go to the [Releases page](https://github.com/mhognogi/laserHM/releases).
2. Under **Assets**, download the file that ends in `x64-setup.exe`.
3. Open the file.
4. If Windows shows "Windows protected your PC", click **More info**. Then click **Run anyway**.
   Windows shows this message because the installer has no digital signature.
5. Follow the steps of the installer.

**Result:** LaserHM is in the Start menu.

---

## 2. Start LaserHM for the first time

1. Open LaserHM.

   **Result:** The "Laser safety" window opens.

2. Read the warnings.
3. Select **I understand**.
4. Click **OK**.

**Result:** LaserHM opens. The safety window does not open again.
To read the warnings again, click **Help** > **Safety**.

---

## 3. Connect the laser and set the work area

### Before you start

- The laser has a GRBL 1.1 controller.
- The laser mode of the controller is on (GRBL setting `$32=1`).

### Procedure

1. Connect the laser to the computer with the USB cable.
2. Turn on the laser.
3. Open LaserHM.

   **Result:** The status box at the top of the laser panel shows "Searching for laser…".
   LaserHM tries the USB ports every 2 seconds.

   **Result:** The status box shows "Connected to laser". The canvas shows the work area.
   LaserHM reads the size of the work area from the laser.

### Set the size of the work area yourself

Do this procedure if the work area on the canvas has the wrong size, or if no laser is connected.

1. Right-click on an empty part of the canvas.
2. Click **Set dimension canvas**.
3. Type the width and the height of the work area, in mm.
4. Click **Set**.

**Result:** The canvas shows the new work area. LaserHM keeps this size, also when a laser is connected.

If no laser is connected and no size is set, the canvas shows "Connect laser or set dimension in mm".
Type the width and the height. Then click **Set dimensions**.

### If an error occurs

- The status box shows "Searching for laser…" and does not change:
  - Make sure that the laser is on.
  - Use a different USB cable or USB port.
  - Make sure that the laser has a GRBL controller at 115200 baud.
- The status box shows "Laser disconnected. Searching again…": the USB connection stopped.
  Examine the USB cable.

---

## 4. Draw and change shapes

### Draw a shape

1. In the tool bar, click a tool:
   - **Rectangle**, **Ellipse** or **Polygon**: drag on the canvas.
     Hold Ctrl for a square, a circle or a polygon with equal sides.
   - **Draw Lines**: click to add points. Click and drag to make a curve.
     Click the first point to close the shape. Double-click to stop an open line.
   - **Text**: click on the canvas and type. Push Esc when the text is complete.

**Result:** The new shape is on the canvas. LaserHM selects it.

### Change the font and the size of a text

1. Select the text.
2. In the top bar, click the font name.
3. Push the up arrow or the down arrow to see other fonts. Push Enter to keep a font.
4. In the **Size** field, type the height of the capital letters, in mm. The default size is 19 mm.

### Edit a text

1. Double-click the text.
2. Use the left and right arrows to move the cursor. Type or delete characters.
3. Push Esc.

### Select shapes

- Click **Select** in the tool bar. Then click a shape.
- To select more shapes, drag a rectangle around them.
- To select all shapes, push Ctrl+A.

**Result:** A dashed rectangle is around the selected shapes.

### Move, resize and rotate

- **Move:** drag the selected shapes.
- **Resize:** drag a small square on the dashed rectangle.
- **Rotate:** drag one of the round arrows near the corners.
- **Exact values:** in the top bar, type values in **xCtr**, **yCtr**, **Width**, **Height** or **Rotate**.
  Then push Enter. You can also turn the mouse wheel over a field.
- **Mirror:** in the top bar, click **Flip horizontally** or **Flip vertically**.

### Undo a change

- Push Ctrl+Z to undo the last change.
- Push Ctrl+Shift+Z to do the change again.

---

## 5. Set the job and examine the path

### Set power, speed and passes

In the laser panel, set these values:

- **Power**: the laser power, in % of the maximum. The default value is 50.
- **Speed**: the speed of the laser head, in mm/min. The default value is 2000.
- **Passes**: how many times the laser cuts each shape. The default value is 1.

LaserHM keeps these values when you close it.

### Examine the path with the laser off

1. Select the shapes.
2. In the laser panel, click **Simulate**.

**Result:** The laser head goes to the origin. Then it follows the shapes with the laser off.
The status box shows "Simulating (laser off)…".

### Find a point on the material

1. In the tool bar, click **Position Laser**.
2. Click on the canvas.

   **Result:** The laser head moves to this point with the laser off.

3. Put on your safety glasses.
4. Hold the **Fire** button. The Fire button is below the Position Laser button.

   **Result:** The laser is on at low power. It does not move. The default power is 10%.

5. Release the **Fire** button.

   **Result:** The laser is off.

To change the Fire power, use the **Fire** field in the top bar.

---

## 6. Start, pause and stop a job

### Before you start

- Put on your safety glasses.
- Adjust the focus of the laser.
- Turn on the air assist, if the laser has one.

### Procedure

1. Select the shapes.

   **Result:** The status box shows "Click to start".

2. Click **Start**.

   **Result:** The laser head goes to the origin. Then the laser cuts the selected shapes.
   The status box shows "Cutting…". The path that the laser cut becomes green.
   A red dot shows the position of the laser head.
   The progress bar shows the time of the job and the total time.

3. Wait until the status box shows "Done. Connected to laser".

### Pause the job

1. Click **Pause**.

   **Result:** The laser head stops and the laser turns off. The status box shows "Paused. Press Start to resume".

2. To continue, click **Start**.

### Stop the job

- Click **Stop** or push the **Pause** key.

**Result:** The laser stops immediately. The status box shows "Stopped. Connected to laser".

### Show reminders before each job

1. Select **Verify** in the laser panel.

**Result:** Before each job, LaserHM asks you to check the focus, the position of the laser head and the air assist.
Close the reminder window. Then click **Start** again.

### If an error occurs

- The status box shows "No object selected": select the shapes first.
- The status box shows "The shape must be entirely inside the work area": move the shapes into the work area.
- The status box shows "Laser replied "error:5"…": homing is off on your laser. *(Not verified on a laser without limit switches.)*
- The status box shows "Laser did not respond to command…": examine the USB cable. Then turn the laser off and on.

---

## 7. Engrave an image

### Before you start

- An image file: PNG, JPG or BMP.

### Procedure

1. Click **File** > **Import…** (or push Ctrl+I).
2. Select the image file. Click **Open**.

   **Result:** The image is in the middle of the work area. The "Adjust Image" window opens.
   The left side shows the original image. The right side shows the engraving.

3. In **Image Mode**, select how LaserHM changes the gray parts of the image:
   - **Threshold**: each dot is black or white. Use it for logos.
   - **Floyd-Steinberg**, **Jarvis** or **Stucki**: dots make the gray parts. Use them for photos.
   - **Grayscale**: the power changes with the gray.
4. Set **Line Interval**: the distance between the engraved lines, in mm. The default value is 0.1 mm.
5. Set **Contrast**, **Brightness** and **Gamma** if necessary.
6. Set **Speed**, **Power max** and **Power min**. LaserHM uses Power min only in Grayscale mode.
7. Click **OK**.

   **Result:** The canvas shows the image as it will be engraved.

8. Move or resize the image.
9. Select the image. Click **Start**.

**Result:** The laser engraves the image line by line.

To change the settings again, double-click the image.

---

## 8. Make a Material Test

A Material Test burns a grid of squares. Each square has a different power and speed.
Use it to find the best values for a new material.

1. Click **Tools** > **Material Test**.
2. Set the values:
   - **Power %**: number of columns (**Count**), first power (**Min**) and last power (**Max**).
   - **Speed**: number of rows (**Count**), first speed (**Min**) and last speed (**Max**).
   - **Mode**: **Fill** engraves each square full. **Line** cuts only the outline.
   - **Labels**: select it to engrave the values next to the grid.
3. Click **Push to canvas**.

   **Result:** The grid is on the canvas and LaserHM selects it.

4. Move the grid to the correct position. You cannot resize the grid.
5. Click **Start**.

**Result:** The laser burns each square with its own power and speed.
During the job, the laser panel shows the power and speed of the current square.

---

## 9. Calibrate the camera

Do this procedure if you use a camera above the laser.
After the calibration, the camera image is in the correct position on the canvas.

### Before you start

- A camera is connected to the computer and points at the work area.

### Procedure

1. Draw a large square. Cut it in a sheet of material. Do not move the material after the cut.
2. In the top bar, click the camera button.

   **Result:** The canvas shows the camera image.

3. Select the square that you cut.
4. Click **Tools** > **Calibrate camera**.

   **Result:** The "Camera calibration" window opens.

5. In the camera image, click 9 points of the cut square in this order:
   1. The 4 corners: bottom-left, bottom-right, top-right, top-left.
   2. The middles of the 4 sides: bottom, right, top, left.
   3. The center.

**Result:** LaserHM aligns the camera image with the canvas and keeps the calibration.

### If an error occurs

- **Calibrate camera** is not available: turn on the camera, then select the cut square.
- To cancel the calibration, push Esc.

---

## 10. Save and open projects

LaserHM saves projects as `.lhm` files. A project file contains the shapes and the images.

- **Save:** click **File** > **Save** (Ctrl+S). The first time, type a file name.
- **Save with a new name:** click **File** > **Save As…** (Ctrl+Shift+S).
- **Open:** click **File** > **Open…** (Ctrl+O).
- **New empty project:** click **File** > **New** (Ctrl+N).

The title of the window shows the name of the project. A `*` after the name means that the project has changes that are not saved.

When you close LaserHM, it keeps the canvas. The next time, LaserHM opens with the same shapes.

---

## 11. Connect Claude Desktop (optional)

Claude Desktop can draw shapes on the LaserHM canvas. Claude cannot start the laser.

### Before you start

- Claude Desktop is installed.
- LaserHM is installed.

### Procedure

1. Find the LaserHM program file. Right-click the LaserHM shortcut in the Start menu.
   Click **Open file location**. *(Not verified: the folder is usually
   `C:\Users\<your name>\AppData\Local\LaserHM`, and the file name can be `laserhm.exe` or `LaserHM.exe`.)*
2. Open the Claude Desktop configuration file `claude_desktop_config.json`.
   In Claude Desktop, click **Settings** > **Developer** > **Edit Config**. *(Not verified in all Claude Desktop versions.)*
3. Add these lines. Use the path of the program file from step 1. Write each `\` as `\\`.

   ```json
   "mcpServers": {
     "laserhm": {
       "command": "C:\\Users\\<your name>\\AppData\\Local\\LaserHM\\laserhm.exe",
       "args": ["--mcp"]
     }
   }
   ```

4. Save the file.
5. Close Claude Desktop completely. Then open it again.
6. Open LaserHM.

**Result:** In Claude Desktop, you can write, for example, "Draw a circle of 50 mm in LaserHM".

**If an error occurs:** If Claude says "LaserHM is not running", open LaserHM and try again.

---

## Safety

> **WARNING:** Laser light can permanently damage your eyes and skin. Reflected light is also dangerous.

- Wear safety glasses for the wavelength of your laser. The glasses must have the EN 207 certification.
- Do not leave the laser alone while it operates.
- Open a window or use an exhaust. Cutting makes smoke.
- Do not cut PVC or vinyl. These materials release chlorine gas.
- Keep a fire extinguisher near the laser.
- To stop a job immediately, click **Stop** or push the **Pause** key on the keyboard.
- Obey the instructions of your laser manufacturer first.

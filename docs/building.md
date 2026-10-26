# Building LaserHM from the source code

This page is for programmers. To use LaserHM, see the [user guide](user-guide.md).

## Before you start

- [Node.js](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/tools/install)

## Procedure

1. Install the dependencies:

   ```sh
   npm install
   ```

2. Run LaserHM in development mode:

   ```sh
   npm run tauri dev
   ```

3. Make the installers:

   ```sh
   npm run tauri build
   ```

   **Result:** The installers are in `src-tauri/target/release/bundle`.

## Releases

When you push a version tag (for example `v0.1.2`), GitHub Actions makes the installers for Windows.
It puts them in a draft release. Publish the draft on the Releases page.
See `.github/workflows/release.yml`.

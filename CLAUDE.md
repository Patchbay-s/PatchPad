# Project guidelines

- Keep the public product name PatchPad and the Rust/Tauri implementation.
- `patchpad-core` must stay free of UI dependencies.
- DS4 report offsets must be checked against `hid-playstation.c` (Linux) and
  `SDL_hidapi_ps4.c` (SDL) and cited in comments; never guess them.
- Fixtures are synthetic: regenerate them with `crates/patchpad-core/tests/fixtures/generate.py`.
  Never commit captures that contain a real controller's serial or MAC address.
- The controller schematic is an original drawing: do not reproduce Sony or Microsoft artwork.
- Run `cargo fmt --check`, workspace Clippy and tests, and the frontend build before
  submitting changes.

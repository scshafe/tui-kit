# ImageBox Visual Test

Small manual test app for `tui-kit::widgets::image_box::ImageBox`.

Run it in a Kitty-graphics-compatible terminal such as WezTerm, kitty, or Ghostty:

```bash
cd /Users/coleshaffer/Projects/tui-kit/visual-tests
cargo run --offline
```

Controls:

- `+` / `=` zoom in
- `-` / `_` zoom out
- arrows or `h/j/k/l` pan while zoomed
- mouse wheel zooms at the cursor
- `0` or `f` reset zoom
- `b` toggle the `ImageBox` border
- `c` cycle the border color
- `q` or `Esc` quit

What to check:

- Initial render shows the generated image centered in the box.
- Zoom grows the theoretical image and crops the colored extremities.
- Pan moves the cropped source window.
- Border/title update without affecting image state.

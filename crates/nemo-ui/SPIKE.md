# nemo-ui glass spike

Goal: check whether iced 0.14 can carry the Liquid Glass player bar (design.md sections 6, 7, 14).

## Run

```
cargo run -p nemo-ui --example glass_spike --features gui
```

Use `--release` for numbers that mean anything. The `gui` feature is off by default, so
`cargo clippy --workspace` does not build iced. iced is pinned `=0.14.0` with
`default-features = false, features = ["wgpu", "thread-pool"]`.

## What the example is

Tier Low look only: generated four-stop gradient plus a translucent cross gradient as the
"cover", and a 88 px bar inset 12 px with `shape.xl` corners, tint, 1 px edge highlight,
shadow, a Play/Pause button, a seek slider and a time label. There is NO live blur and NO
lens. A readout (top left, on an opaque `surface-container-high` plate) shows the average
and worst interval between redraws over the last second. The bar tint alpha is raised from
0.55 in 0.05 steps until `on-surface` reaches 4.5:1 over the lightest cover color (rule from
design.md section 6; computed from the known gradient stops, not from pixels).

## What the owner should read from the readout

- avg ms and fps: at 60 Hz expect about 16.7 ms. Lower fps with a low avg means the monitor
  is the limit (vsync), not the app.
- worst ms: should stay under 16.6 ms (budget, design.md section 7). Spikes above that
  count as dropped frames. Note any value over 33 ms.
- frames in last 1 s: about 60 at 60 Hz. Far fewer means redraws are being skipped or the
  frame is too slow.
- Run in release, with the window at 1280x800, on the owner's real GPU, and write down
  the GPU (integrated or discrete).

The readout is the time between redraws from `iced::window::frames()`. It is NOT GPU
time. Frame interval can hide GPU work that the present queue absorbs. Use a GPU profiler
for glass cost (budget: glass <= 3 ms at High).

## Not verified

- I could not run the window or measure 60 fps here. No frame time was measured.
  The example was only compiled and linted.
- The window, gradient look, text legibility and the slider and button rendering have not
  been seen.
- Whether the readout updates while the window is occluded or minimized.

## Tier High investigation (live blur): desk research only

Sources read (docs.rs, iced 0.14.0):
- Shader widget `Program`: https://docs.rs/iced/0.14.0/iced/widget/shader/trait.Program.html
- `Primitive`: https://docs.rs/iced/0.14.0/iced/widget/shader/trait.Primitive.html
- Application: https://docs.rs/iced/0.14.0/iced/application/index.html

Findings from the docs:
- A `Program` returns a `Primitive` from `draw`. The `Primitive` has a shared `Pipeline`
  (wgpu state), `prepare(pipeline, device, queue, bounds, viewport)`, and two ways to draw:
  `draw(pipeline, render_pass)` into the existing render pass (viewport and scissor are
  set to the widget bounds), or `render(pipeline, encoder, target, clip_bounds)` with
  its own `CommandEncoder` and the target `TextureView`. `draw` returns `false` to ask
  iced to call `render` instead, "for complex composition needs".
- So a custom pipeline can run a full-screen or bar-sized pass and has the encoder and the
  target view. That is the hook a blur would need.

Unverified (do not rely on these):
- That `target` in `render` holds everything drawn before the bar (backdrop pixels), is
  readable as a texture (needs `TextureUsages::TEXTURE_BINDING` on the swapchain surface or
  a copy), and is not a per-layer or MSAA target. Not in the docs page; need the iced_wgpu
  0.14 source.
- That sampling the frame while rendering into it is allowed. Normally it needs a copy
  (`copy_texture_to_texture`) into a scratch texture first, which costs bandwidth each
  frame.
- Draw ordering: whether the bar's primitive runs after the layers below it, and whether
  text and widgets drawn above the shader layer render after it.
- Cost of dual-Kawase 24 px blur at 1280x800 on integrated GPUs.

Fallback paths that need no framebuffer access (matches design.md section 14):
1. Tier Low (shipped first): pre-blur the cover on a worker thread, draw as an image behind
   the bar, add tint and edge (what the example imitates without the image).
2. Render the cover plus static panels to your own offscreen texture, blur that in a custom
   `Primitive`, and draw the result clipped to the bar bounds. This reproduces the
   backdrop from known content and avoids reading iced's frame.

Suggested next step: read `iced_wgpu` 0.14 source for how `Primitive::render` receives its
target, then prototype the copy-and-blur in a shader widget. Measure with a GPU timer.

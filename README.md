# WIP Nanite-like LOD Renderer

Requires a mesh shader capable GPU, which all raytracing capable GPUs are (plus a few more). The master branch has been tested on:
* Windows Nvidia 30XX series
* Linux AMD 680M iGPU, very similar to a SteamDeck

Currently, I am still using the vulkano library for vulkan bindings, which unfortunately is causing problems with
barriers, layout transitions and general synchronization. For this reason the current master does not pass validation
layers, though it does seem to run on most platforms. But the Nanite-like LOD generation (branch `lod_tree_gen`),
as seen in [this video](https://www.youtube.com/watch?v=g002AhbOUOM), is currently blocked from merging as it only seems
to work on Linux on my AMD iGPU. I'm working towards replacing the library with my own systems build on top of raw
vulkan bindings provided by the `ash` crate in the `ash` branch, which will hopefully be ready in a week or two.

## Building and Running

* have [Rust](https://rustup.rs/) and the [Vulkan SDK](https://vulkan.lunarg.com/) installed
* Build and run with `cargo run --release`

Once started, you may use the windows key to undock your mouse and maximize the window.

## Controls

* WASD - to move
* Space / Shift - Up and down
* Scrollwheel - adjust Speed
* Home - reset camera
* T/G - cycle through all the available scenes
* R/F - adjust the LOD level down or up (`lod_tree_gen` branch only)
* Q/E - cycle through rendering modes: PBR, PBR mixed with meshlet coloring, just meshlet coloring, baseColor, normals with normal maps, vertex normals, texture coordinates and position reconstruction from depth

## Scenes

The Lantern scene is always available, for additional scenes checkout the `scenes` branch with ~1GiB of additional
scenes available. You may add scenes in the `gltf` or `glb` format yourself by putting them in
`/models/models/local/`, restarting and cycling to them using the T/G keys.

Showcase scenes:
* Amazon Lumberyard Bistro, CC-BY 4.0, 2017 Amazon Lumberyard
* Sponza: CRYENGINE Limited License Agreement
* San Miguel: Outdoors Restaurant from Mexico, CC BY 3.0, Guillermo M. Leal Llaguno

Minecraft scenes:
* Rungholt: CC BY 3.0, kescha

Object scenes:
* Damaged Helmet: CC BY 4.0, ctxwing, theblueturtle_
* Lantern: CC0 1.0 Universal, Microsoft, Frank Galligan

All downloaded scenes can be found either on https://casual-effects.com/data/ or https://github.com/KhronosGroup/glTF-Sample-Assets

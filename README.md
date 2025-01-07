<!-- Allow this file to not have a first line heading -->
<!-- markdownlint-disable-file MD041 -->

<!-- inline html -->
<!-- markdownlint-disable-file MD033 -->

<div align="center">

# Nanite-like LOD Renderer

**Nanite made from scratch in Rust as my master thesis**

[<img src="docs/nanite_bistro.jpg" alt="Meshlet Bistro" width="700"/>](https://www.youtube.com/watch?v=g002AhbOUOM)
</div>

[LOD generation showcase video](https://www.youtube.com/watch?v=g002AhbOUOM)

This is the current state of my Master thesis about replicating UE5’s Nanite. It's a GPU-driven renderer using meshlets and mesh shaders that can render glTF scenes as complicated as bistro. I have a custom baker to preprocess all the meshes offline, allowing me to offload the generation of Nanite's LOD tree and enabling blazing fast load times of under a second for all scenes. In the [LOD generation showcase video](https://www.youtube.com/watch?v=g002AhbOUOM), one can see the output of the LOD tree generator at its various LOD levels. The only missing part is the runtime LOD selection, so currently one can only select a specific LOD level for all meshes, instead of having models automatically use a lower LOD as they get further away from the camera.

## Requirements

Requires a mesh shader capable GPU, which all raytracing capable GPUs are (plus a few more). The master branch has been tested on:
* Windows Nvidia 30XX series
* Windows AMD 680M iGPU, very similar to a SteamDeck
* Linux AMD is known to sometimes randomly crash with "device lost", especially when switching scenes.

## Building and Running

* have [Rust](https://rustup.rs/) and the [Vulkan SDK](https://vulkan.lunarg.com/) installed
* Build and run with `cargo run --release`

Once started, you can resize or maximize the window, before having it grab the cursor with Tab.

## Controls

For the complex configurations, see the console output for what has been selected.

* Tab - grab / ungrab the cursor
* WASD - to move
* Space / Shift - Up and down
* Scrollwheel - adjust speed
* Home - reset camera
* T/G - cycle through all the available scenes
* R/F - adjust the LOD selection, default is dynamic "Nanite" selection, increasing it with F selects LOD 0, 1, 2, etc. or the lowest available
* Q/E - cycle through different rendering modes, see console output on what they are
* X/C - adjust the target triangle density for Nanite selection, can yield some broken results in extreme settings
* M - disable mouse rotating the camera

## Scenes

The Lantern scene is always available, for additional scenes checkout the `scenes` branch with ~1GiB of additional scenes available. You may also add scenes in the `gltf` or `glb` format yourself by putting them in `/models/models/local/`, restarting and cycling to them using the T/G keys.

Available scenes:
* [Lantern](https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/Lantern): CC0 1.0 Universal, Microsoft, Frank Galligan
* [Not a Stanford Bunny](https://jocelyndaprato.artstation.com/projects/g8PKBm), no right to sell, Feel free to share, use, modify, by Jocelyn Da Prato
* [Amazon Lumberyard Bistro](https://developer.nvidia.com/orca/amazon-lumberyard-bistro), CC-BY 4.0, 2017 Amazon Lumberyard
* [Sponza](https://casual-effects.com/data/): CC BY 3.0, 2010 Frank Meinl, Crytek
* [San Miguel](https://casual-effects.com/data/): CC BY 3.0, Guillermo M. Leal Llaguno
* [Rungholt](https://casual-effects.com/data/): CC BY 3.0, kescha
* [Damaged Helmet](https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/DamagedHelmet): CC BY 4.0, ctxwing, theblueturtle_

## Technical Details

This Project is written in [Rust](https://rustup.rs/), which should be fairly readable by C++ Programmers, and the shaders are also written in Rust thanks to the [rust-gpu](https://github.com/Rust-GPU/rust-gpu/) shader compiler. This allows me to easily share datastructures and algorithms between the CPU and GPU, and enables the use of rust tooling such as formatters, linters and tests in the shaders.

The Renderer uses a simple G-Buffer, as I do not have enough time to implement a visibility buffer-based renderer. In the first 3D pass, I render out all meshlets using this [mesh and fragment shader](space-engine-shader/src/renderer/meshlet/mesh_shader.rs) to the G-Buffer and the deferred pass uses a [lighting CS](space-engine-shader/src/renderer/lighting/lighting_compute.rs) with most of the [PBR evaluation here](space-engine-shader/src/material/pbr.rs). The background is written in a following [sky CS](space-engine-shader/src/renderer/lighting/lighting_compute.rs) which only writes to fragment of
`alpha = 0.0`.

The Nanite data structure is split up into the [disk format](space-asset-disk/src/meshlet) and the [shader format](space-asset-shader/src/meshlet), as the disk format, serialized with [rkyv](https://github.com/rkyv/rkyv), should be focused on compression with [zstd](https://github.com/gyscos/zstd-rs) whereas the runtime format should focus on the access patterns of the GPU. A few basic shared structs can be found in [disk shader](space-asset-disk-shader/src/meshlet). The [preprocessor](space-asset-preprocess/src/meshlet/build_script.rs) searches for glTF files, processes them in parallel using [rayon](https://github.com/rayon-rs/rayon) and writes them out in my internal disk format. The [runtime](space-asset-rt/src/meshlet/scene.rs) then decompresses and converts it into the shader format.

To represent indirections to other Buffers, Images or other Resources I have build my own [bindless library](vulkano-bindless) specifically to be used with rust-gpu. The sharing of code allows me to declare GPU structs with "Descriptors" pointing at other resources that I can upload and validate from the CPU directly. I'm currently in the process of reworking this system as I transition away from vulkano to ash, and will likely add more detail on it at a later date.

# Debugging

In `meshlet-renderer/src/main_loop.rs` around line 35 there is the constant `const DEBUGGER: Debuggers` which can be set to a variety of debuggers, like `RenderDoc`, `Validation` or `DebugPrintf`. While `GpuAssistedValidation` is also available, it may report many false positives, due to validation layers not having proper timeline semaphore support yet. 

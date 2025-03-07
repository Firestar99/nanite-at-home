<!-- Allow this file to not have a first line heading -->
<!-- markdownlint-disable-file MD041 -->

<!-- inline html -->
<!-- markdownlint-disable-file MD033 -->

<div align="center">

# Nanite at home

**Nanite made from scratch in Rust as my master thesis**

[<img src="docs/nanite_bistro.jpg" alt="Meshlet Bistro" width="700"/>](https://www.youtube.com/watch?v=K0du8jCp42I)
</div>

[Showcase video](https://www.youtube.com/watch?v=K0du8jCp42I)


In my master thesis I've replicated UE5's Nanite, which is able to draw an object in multiple LODs, without introducing any holes or seams at LOD transitions. It's a GPU-driven renderer using meshlets and mesh shaders that can render glTF scenes as complicated as bistro. I have a custom baker to preprocess all the meshes offline, allowing me to offload the generation of Nanite's LOD tree and enabling blazing fast load times of under a second for all scenes. In the [LOD generation showcase video](https://www.youtube.com/watch?v=g002AhbOUOM), one can see the output of the LOD tree generator at its various LOD levels. At runtime, these LODs are automatically selected based on their distance to the camera, and due to the unique data structure proposed by Nanite, allow the LODs to transition mid-model.

## Requirements

Requires a mesh shader capable GPU, which all raytracing capable GPUs are (plus a few more). The master branch has been tested on:
* Windows Nvidia 3070ti mobile
* Windows AMD 680M iGPU, very similar to a SteamDeck
* Linux AMD RADV is known to crash with "device lost", haven't had time to debug further

## Building and Running

* have [Rust](https://rustup.rs/) and the [Vulkan SDK](https://vulkan.lunarg.com/) installed
* Build and run with `cargo run --release`

Once started, you can resize or maximize the window as you wish. The UI has the controls spelled out at the very top, most importantly using Tab to switch between UI and game focus. Feel free to play around with the settings!

## Scenes

The Lantern scene is always available, for additional scenes checkout the `scenes` branch with ~1GiB of additional scenes available. You may also add scenes in the `gltf` or `glb` format yourself by putting them in `/models/models/local/` and restarting, which should list them in the scenes ComboBox.

Available scenes:
* [Lantern](https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/Lantern): CC0 1.0 Universal, Microsoft, Frank Galligan
* [Not a Stanford Bunny](https://jocelyndaprato.artstation.com/projects/g8PKBm), no right to sell, Feel free to share, use, modify, by Jocelyn Da Prato
* [Amazon Lumberyard Bistro](https://developer.nvidia.com/orca/amazon-lumberyard-bistro), CC-BY 4.0, 2017 Amazon Lumberyard
* [Sponza](https://casual-effects.com/data/): CC BY 3.0, 2010 Frank Meinl, Crytek
* [San Miguel](https://casual-effects.com/data/): CC BY 3.0, Guillermo M. Leal Llaguno
* [Rungholt](https://casual-effects.com/data/): CC BY 3.0, kescha
* [Damaged Helmet](https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/DamagedHelmet): CC BY 4.0, ctxwing, theblueturtle_

## Technical Details

This Project is written in [Rust](https://rustup.rs/), which should be fairly readable by C++ Programmers, and the shaders are also written in Rust thanks to the [rust-gpu](https://github.com/Rust-GPU/rust-gpu/) shader compiler, of which I have been made a maintainer. This allows me to easily share datastructures and algorithms between the CPU and GPU, and enables the use of rust tooling such as formatters, linters and tests in the shaders.

To represent indirections from one buffer to other buffers, images or samplers I have built my own [bindless library](rust-gpu-bindless), specifically to be used with rust-gpu. The sharing of code allows me to declare GPU structs with "Descriptors" pointing at other resources, that can easily be uploaded directly from the CPU. Some simple examples are available as [integration tests](rust-gpu-bindless/tests/integration-test) with their shader counterparts [here](rust-gpu-bindless/tests/integration-test-shader). These indirections allow me to jump from a [scene struct](space-asset-shader/src/meshlet/scene.rs) to [instance](space-asset-shader/src/meshlet/instance.rs) and [model structs](space-asset-shader/src/meshlet/mesh.rs), and from those model structs to [vertex](space-asset-disk-shader/src/material/pbr.rs), [index](space-asset-disk-shader/src/meshlet/indices.rs) buffers and [material textures](space-asset-shader/src/material/pbr.rs). 

To select which meshlets at their various LODs to render, I use two compute passes supplied with a reference to the scene struct. I spawn one workgroup of the [instance cull CS](space-engine-shader/src/renderer/meshlet/instance_cull.rs) for each model instance, cull the instance, and use all 32 invocations to write out all [meshlet instance groups](space-engine-shader/src/renderer/meshlet/intermediate.rs) of up to 32 meshlets each. Due to the sheer amount of meshlets each model contains, this proved to be much more performant than spawning one invocation per instance, as is typically done. A second [meshlet select CS](space-engine-shader/src/renderer/meshlet/meshlet_select.rs) is launched indirectly with one workgroup per meshlet group emitted previously, so that each invocation culls one meshlet, and writes all passing meshlet instances into a buffer.

The Renderer uses a simple G-Buffer, as I have not had the time to implement a visibility buffer-based renderer. In the 3D pass, I render out all meshlets from the previously generated meshlet buffer using this [mesh and fragment shader](space-engine-shader/src/renderer/meshlet/mesh_shader.rs) to the G-Buffer, which is by far the slowest step. The deferred pass uses a [lighting CS](space-engine-shader/src/renderer/lighting/lighting_compute.rs) with most of the [PBR evaluation here](space-engine-shader/src/material/pbr.rs), and the background is written in a following [sky CS](space-engine-shader/src/renderer/lighting/lighting_compute.rs), which only writes to fragment of `alpha = 0.0`.

The Nanite data structure is split up into the [disk format](space-asset-disk/src/meshlet) and the [shader format](space-asset-shader/src/meshlet), as the disk format, serialized with [rkyv](https://github.com/rkyv/rkyv), should be focused on compression with [zstd](https://github.com/gyscos/zstd-rs) whereas the runtime format should focus on the access patterns of the GPU. A few basic shared structs can be found in [disk shader](space-asset-disk-shader/src/meshlet). The [preprocessor](space-asset-preprocess/src/meshlet/build_script.rs) searches for glTF files, processes them in parallel using [rayon](https://github.com/rayon-rs/rayon) and writes them out in my internal disk format. The [runtime](space-asset-rt/src/meshlet/scene.rs) then decompresses and converts it into the shader format.

The UI is using [egui](https://github.com/emilk/egui), an [ImGui](https://github.com/ocornut/imgui)-like UI framework written in rust. I've integrated it into my bindless renderer only after submitting the thesis.

### Debugging

In `meshlet-renderer/src/main_loop.rs` around line 38 there is the constant `const DEBUGGER: Debuggers` which can be set to a variety of debuggers, like `RenderDoc`, `Validation` or `DebugPrintf`. While `GpuAssistedValidation` is also available, it is known to report many [false positives](https://github.com/KhronosGroup/Vulkan-ValidationLayers/issues/9289) for this project. 

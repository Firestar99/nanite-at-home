# Meshlet Renderer - not quite Nanite yet

Requires a mesh shader capable GPU, which are all raytracing capable GPUs plus a few more. Specifically an Nvidia RTX 16XX or 20XX or greater. Unfortunately, it currently does not run on AMD Windows due to a recently introduced driver incompatibility I haven't had the time to fix.

Once started, use the windows key to undock your mouse and maximize the window. Then use T/G to cycle through all available scenes.

## Building

* have [Rust](https://rustup.rs/) and the [Vulkan SDK](https://vulkan.lunarg.com/) installed
* Build and run with `cargo run --bin meshlet-renderer`

## Controls

* WASD - to move
* Space / Shift - Up and down
* Scrollwheel - adjust Speed
* Home - reset camera
* E - cycle through rendering modes: With light, light mixed with meshlet coloring, just meshlet coloring, just color, normals, texture coordinates
* T/G - cycle through all the available scenes

## Scenes

Showcase scenes:
* Amazon Lumberyard Bistro, CC-BY 4.0, 2017 Amazon Lumberyard
* Sponza: CRYENGINE Limited License Agreement
* San Miguel: Outdoors Restaurant from Mexico, CC BY 3.0, Guillermo M. Leal Llaguno

Minecraft scenes:
* Rungholt: CC BY 3.0, kescha
* Lost Empire: CC BY 3.0, 2011 Morgan McGuire
* Vokselia spawn: CC BY 3.0, 2011 Vokselia

Object scenes:
* Damaged Helmet: CC BY 4.0, ctxwing, theblueturtle_
* Lantern: CC0 1.0 Universal, Microsoft, Frank Galligan
* Head: CC BY 3.0, Lee Perry-Smith, www.triplegangers.com, Morgan McGuire
* Sibenik: Marko Dabrovic, Kenzie Lamar, Morgan McGuire

All downloaded scenes can be found either on https://casual-effects.com/data/ or https://github.com/KhronosGroup/glTF-Sample-Assets

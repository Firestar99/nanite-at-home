# Meshlet Renderer - not quite Nanite yet

Requires a mesh shader capable GPU, which are all raytracing capable GPUs plus a few more. Specifically requires an Nvidia RTX 16XX or 20XX or greater. Unfortunately, it currently does not run on AMD Windows due to a recently introduced driver incompatibility I haven't had the time to fix.

## Controls

* Tab / E - switch between cursor and camera movement
* WASD - to move
* Space / Shift - Up and down
* Scrollwheel - adjust Speed
* Home - reset camera
* Settings panel has additional unimportant keybinds listed

## Scenes

Good-looking scenes:

* conference: A conference room, "Credit required", Anat Grynberg and Greg Ward
* salle de bain: A Bathroom, CC BY 3.0, Nacimus Ait Cherif
* san miguel: Outdoors Restaurant from Mexico, CC BY 3.0, Guillermo M. Leal Llaguno

Technical and testing scenes:

* Cornell Box: CC BY 3.0, 2009 Morgan McGuire. Original scene has an "area light", but accumulating light with an area light is hard. So we also have a simpler scene with a "point light" instead.
* Testbox: A custom-made scene for testing.

All downloaded scenes can be found on https://casual-effects.com/data/

## Settings

* Mode:
    * Indirect trace direct trace: Uses path tracing for everything, result can be quite noisy.
    * indirect ic direct trace (default): Uses an irradiance cache to accumulate indirect light, combined with path tracing for direct illumination for the best result.
    * indirect ic: Shows only the indirect light accumulated via irradiance cache.
    * direct trace: Shows only direct lighting via path tracing.
    * direct ic: Direct lighting via the irradiance cache.
    * debug pattern: A debug pattern to judge the size of individual accumulation cells.
* Clear Irradiance Cache: clears the irradiance cache so that light accumulation has to start from scratch
* Traces per Frame: how many traces should be sent out per frame and probe, higher values trade worse performance for faster accumulation
* Accumulation Quality: adjusts accumulation factor to match traces per frame so the image quality remains mostly constant, higher values trade worse accumulation speed for better final image quality
* Accumulation Factor: the percentage by which the newly sampled values should be mixed with the previous values, higher values trade accumulation speed for better final image quality
* Normal Offset Factor: When sampling some geometry, the light will be sampled from the irradiance cache at the position the geometry was hit plus the normal of the geometry times this factor

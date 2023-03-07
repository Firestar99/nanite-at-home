use clap::Parser;

use space_client::APPLICATION_CONFIG;
use space_client::cli_args::Cli;
use space_client::vulkan::{Queues, SpaceQueueAllocator};
use space_engine::{reinit, reinit_no_restart};
use space_engine::reinit::{ReinitRef, Target};
use space_engine::reinit::State::Initialized;
use space_engine::vulkan::init::{init, Init, Plugin};
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;

reinit_no_restart!(CLI: Cli = Cli::parse());
reinit!(RENDERDOC_ENABLE: bool = (CLI: Cli) => |cli, _| cli.renderdoc);
reinit!(VALIDATION_LAYER: bool = (CLI: Cli) => |cli, _| cli.validation_layer);
reinit!(VULKAN_INIT: Init<Queues> = (VALIDATION_LAYER: bool, RENDERDOC_ENABLE: bool) => |validation_layer, renderdoc_enable, _| {
		let mut plugins: Vec<&mut dyn Plugin> = vec![];

		let mut standard_validation_plugin = StandardValidationLayerPlugin {};
		if **validation_layer {
			plugins.push(&mut standard_validation_plugin);
		}
		let mut renderdoc_plugin = RenderdocLayerPlugin {};
		if **renderdoc_enable {
			plugins.push(&mut renderdoc_plugin);
		}

		let init = init(APPLICATION_CONFIG, plugins, SpaceQueueAllocator::new());
		println!("{}", init.device.physical_device().properties().device_name);
		init
});

struct Main {
	init: ReinitRef<Init<Queues>>,
}

impl Target for Main {}

reinit!(MAIN: Main = (VULKAN_INIT: Init<Queues>) => |init, _| Main {init: init.clone()});

fn main() {
	let _need = MAIN.need();
	MAIN.assert_state(Initialized);
	println!("exiting...");
}

use clap::Parser;

use space_engine::{reinit, reinit_no_restart};
use space_engine::vulkan::init::{init, Init, Plugin};
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;

use crate::APPLICATION_CONFIG;
use crate::cli_args::Cli;
use crate::vulkan::{Queues, SpaceQueueAllocator};

reinit_no_restart!(pub CLI: Cli = Cli::parse());
reinit!(pub RENDERDOC_ENABLE: bool = (CLI: Cli) => |cli, _| cli.renderdoc);
reinit!(pub VALIDATION_LAYER: bool = (CLI: Cli) => |cli, _| cli.validation_layer);
reinit!(pub VULKAN_INIT: Init<Queues> = (VALIDATION_LAYER: bool, RENDERDOC_ENABLE: bool) => |validation_layer, renderdoc_enable, _| {
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

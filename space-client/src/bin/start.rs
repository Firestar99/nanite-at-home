use clap::Parser;
use space_client::APPLICATION_CONFIG;

use space_client::cli_args::Cli;
use space_client::vulkan::ClientQueueAllocator;
use space_engine::reinit::Reinit;
use space_engine::vulkan::init::{init, Plugin};
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;

fn main() {
	let cli = Reinit::new_no_restart(Cli::parse);
	let vulkan = Reinit::new1(&cli, |cli, _| {
		let mut plugins: Vec<&mut dyn Plugin> = vec![];

		let mut standard_validation_plugin = StandardValidationLayerPlugin {};
		if cli.validation_layer {
			plugins.push(&mut standard_validation_plugin);
		}
		let mut renderdoc_plugin = RenderdocLayerPlugin {};
		if cli.renderdoc {
			plugins.push(&mut renderdoc_plugin);
		}

		let init = init(APPLICATION_CONFIG, plugins, ClientQueueAllocator::new());
		println!("{}", init.device.physical_device().properties().device_name);
		init
	});
}

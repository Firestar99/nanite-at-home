use clap::Parser;
use space_client::APPLICATION_CONFIG;

use space_client::cli_args::Cli;
use space_client::vulkan::create_vulkan_instance_and_device;

fn main() {
	let cli = Cli::parse();
	let init = create_vulkan_instance_and_device(APPLICATION_CONFIG, &cli);
	println!("{}", init.device.physical_device().properties().device_name);
}

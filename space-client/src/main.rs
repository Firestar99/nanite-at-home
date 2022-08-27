use clap::Parser;
use space_client::APPLICATION_CONFIG;

use space_client::cli_args::Cli;
use space_client::device_selection::create_vulkan_instance_and_device;

fn main() {
	let cli = Cli::parse();
	let (instance, device) = create_vulkan_instance_and_device(APPLICATION_CONFIG, &cli);
	println!("{}", device.physical_device().properties().device_name);
}

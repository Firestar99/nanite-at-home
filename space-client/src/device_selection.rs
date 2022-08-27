use std::sync::Arc;

use vulkano::device::{Device, Queue, QueueCreateInfo};
use vulkano::device::physical::PhysicalDevice;
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions, layers_list};

use space_engine::application_config::ApplicationConfig;
use space_engine::vulkan::init::{init, Plugin, QueueAllocator};
use space_engine::vulkan::plugins::renderdoc_layer_plugin::RenderdocLayerPlugin;
use space_engine::vulkan::plugins::standard_validation_layer_plugin::StandardValidationLayerPlugin;

use crate::cli_args::Cli;

pub fn create_vulkan_instance_and_device(application_config: ApplicationConfig, cli: &Cli) -> (Arc<Instance>, Arc<Device>) {
	let mut plugins: Vec<&mut dyn Plugin> = vec![];

	let mut standard_validation_plugin = StandardValidationLayerPlugin {};
	if cli.validation_layer {
		plugins.push(&mut standard_validation_plugin);
	}
	let mut renderdoc_plugin = RenderdocLayerPlugin {};
	if cli.renderdoc {
		plugins.push(&mut renderdoc_plugin);
	}

	let (instance, device, queues) = init(application_config, plugins, Bla {});

	(instance, device)
}

struct Bla {}

impl QueueAllocator for Bla {
	fn alloc<'a>(&self, _instance: &Arc<Instance>, _physical_device: &PhysicalDevice<'a>) -> Vec<QueueCreateInfo<'a>> {
		vec!(QueueCreateInfo {
			family: _physical_device.queue_families().next().unwrap(),
			queues: vec![0.],
			_ne: Default::default(),
		})
	}
}

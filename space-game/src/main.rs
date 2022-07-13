use space_engine::application_config::{ApplicationConfig, ApplicationVersion};
use space_engine::render::vulkan::vk_instance::{VkInstanceConfig, VkInstanceService};

static APPLICATION_CONFIG: ApplicationConfig = ApplicationConfig {
    name: "space-rust",
    version: ApplicationVersion {
        major: 1,
        minor: 0,
        patch: 0,
    },
};

fn main() {
    let mut vk_instance_config = VkInstanceConfig::new(APPLICATION_CONFIG);
    vk_instance_config.requested_extensions = vk_instance_config.requested_extensions.union(&vulkano_win::required_extensions());

    let vk_instance = VkInstanceService::new(vk_instance_config);
    println!("Vulkan {}", vk_instance.instance().api_version());
    println!("Extensions: {:#?}", vk_instance.instance().enabled_extensions());
}

use crate::reinit;
use crate::space::bootup::VULKAN_INIT;
use crate::space::Init;
use crate::space::renderer::lodobj::opaque::OpaquePipeline;

reinit!(pub OPAQUE: OpaquePipeline = (VULKAN_INIT: Init) => |init, _| {
	OpaquePipeline::new(&init.device)
});

use crate::descriptor::Bindless;
use crate::platform::BindlessPipelinePlatform;
use rust_gpu_bindless_shaders::buffer_content::Metadata;
use rust_gpu_bindless_shaders::frame_in_flight::FrameInFlight;
use std::sync::Arc;

pub struct ExecutionContext<'a, P: BindlessPipelinePlatform> {
	bindless: Arc<Bindless<P>>,
	fif: FrameInFlight<'a>,
	metadata: Metadata,
	pub cmd: P::RecordingCommandBuffer,
}

impl<'a, P: BindlessPipelinePlatform> ExecutionContext<'a, P> {
	pub fn bindless(&self) -> &Arc<Bindless<P>> {
		&self.bindless
	}

	pub fn fif(&self) -> FrameInFlight<'a> {
		self.fif
	}

	pub fn metadata(&self) -> Metadata {
		self.metadata
	}
}

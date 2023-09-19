use std::sync::Arc;

use smallvec::SmallVec;
use vulkano::device::Queue;

use crate::application_config::ApplicationConfig;
use crate::generate_application_config;

pub mod platform;
pub mod debug;
pub mod queue_allocation_helper;
pub mod init;
pub mod plugins;
pub mod window;

pub const ENGINE_APPLICATION_CONFIG: ApplicationConfig = generate_application_config!();

/// create a `SmallVec` of **unique** queue families from the supplied queues
///
/// impl-note: queue families are made unique by searching the resulting `SmallVec` linearly instead of using a `HashSet` or the line,
/// as for small sizes of typically 2-3 it's not worth creating one.
pub fn unique_queue_families<const N: usize>(queues: &[&Arc<Queue>]) -> SmallVec<[u32; N]>
{
	let mut ret = SmallVec::<[u32; N]>::new();
	for x in queues.into_iter().map(|q| q.queue_family_index()) {
		if !ret.contains(&x) {
			ret.push(x);
		}
	}
	ret
}

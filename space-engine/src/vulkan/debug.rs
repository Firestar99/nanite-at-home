use std::sync::Arc;

use vulkano::instance::debug::{DebugCallback, MessageSeverity, MessageType};
use vulkano::instance::Instance;

pub struct Debug {
	_debug_callback: DebugCallback,
}

impl Debug {
	pub fn new(instance: &Arc<Instance>) -> Debug {
		Debug {
			_debug_callback: DebugCallback::new(&instance, MessageSeverity::all(), MessageType::all(),
												|m| println!("[{}] {}: {}", debug_severity_string(m.severity), debug_type_string(m.ty), m.description),
			).unwrap()
		}
	}
}

fn debug_severity_string(a: MessageSeverity) -> &'static str {
	if a.error {
		return "error";
	}
	if a.warning {
		return "warning";
	}
	if a.information {
		return "information";
	}
	if a.verbose {
		return "verbose";
	}
	return "unknown";
}

fn debug_type_string(a: MessageType) -> &'static str {
	if a.validation {
		return "validation";
	}
	if a.performance {
		return "performance";
	}
	if a.general {
		return "general";
	}
	return "unknown";
}
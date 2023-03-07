use std::sync::Arc;

use vulkano::instance::debug::{DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessenger, DebugUtilsMessengerCreateInfo, Message};
use vulkano::instance::Instance;

pub struct Debug {
	_debug_callback: DebugUtilsMessenger,
}

impl Debug {
	pub fn new(instance: &Arc<Instance>) -> Debug {
		// SAFETY: the user_callback may not make any vulkan calls
		unsafe {
			Debug {
				_debug_callback: DebugUtilsMessenger::new(instance.clone(), DebugUtilsMessengerCreateInfo {
					message_type: DebugUtilsMessageType {
						general: true,
						performance: true,
						validation: true,
						..DebugUtilsMessageType::empty()
					},
					message_severity: DebugUtilsMessageSeverity {
						error: true,
						warning: true,
						information: true,
						verbose: false,
						..DebugUtilsMessageSeverity::empty()
					},
					..DebugUtilsMessengerCreateInfo::user_callback(Arc::new(Self::debug_message))
				}).unwrap()
			}
		}
	}

	/// SAFETY: the user_callback may not make any vulkan calls
	fn debug_message(m: &Message) {
		let error = format!("[{}] {}{}: {}",
							Self::debug_severity_string(m.severity),
							Self::debug_type_string(m.ty),
							m.layer_prefix.map(|s| format!(" by {}", s)).unwrap_or(String::new()),
							m.description
		);
		if m.severity.error {
			panic!("{}", error);
		} else {
			println!("{}", error);
		}
	}

	fn debug_severity_string(a: DebugUtilsMessageSeverity) -> &'static str {
		if a.error {
			"error"
		} else if a.warning {
			"warning"
		} else if a.information {
			"information"
		} else if a.verbose {
			"verbose"
		} else {
			unreachable!();
		}
	}

	fn debug_type_string(a: DebugUtilsMessageType) -> &'static str {
		if a.validation {
			"validation"
		} else if a.performance {
			"performance"
		} else if a.general {
			"general"
		} else {
			unreachable!()
		}
	}
}
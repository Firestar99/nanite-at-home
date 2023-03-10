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
							m.layer_prefix.unwrap_or("Unknown"),
							if m.ty.validation {
								" (Validation)"
							} else if m.ty.performance {
								" (Performance)"
							} else if m.ty.general {
								""
							} else {
								unreachable!()
							},
							m.description
		);
		if m.severity.error {
			panic!("{}", error);
		} else {
			println!("{}", error);
		}
	}

	pub fn debug_severity_string(a: DebugUtilsMessageSeverity) -> &'static str {
		if a.error {
			"Error"
		} else if a.warning {
			"Warn"
		} else if a.information {
			"Info"
		} else if a.verbose {
			"Verbose"
		} else {
			unreachable!();
		}
	}

	pub fn debug_type_string(a: DebugUtilsMessageType) -> &'static str {
		if a.validation {
			"Validation"
		} else if a.performance {
			"Performance"
		} else if a.general {
			"General"
		} else {
			unreachable!()
		}
	}
}
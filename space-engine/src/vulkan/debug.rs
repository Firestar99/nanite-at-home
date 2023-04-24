use std::sync::Arc;

use vulkano::instance::debug::DebugUtilsMessageSeverity;
use vulkano::instance::debug::DebugUtilsMessageType;
use vulkano::instance::debug::DebugUtilsMessenger;
use vulkano::instance::debug::DebugUtilsMessengerCreateInfo;
use vulkano::instance::debug::Message;
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
					message_type: DebugUtilsMessageType::GENERAL | DebugUtilsMessageType::PERFORMANCE | DebugUtilsMessageType::VALIDATION,
					message_severity: DebugUtilsMessageSeverity::ERROR | DebugUtilsMessageSeverity::WARNING | DebugUtilsMessageSeverity::INFO | DebugUtilsMessageSeverity::VERBOSE,
					..DebugUtilsMessengerCreateInfo::user_callback(Arc::new(Self::debug_message))
				}).unwrap()
			}
		}
	}

	/// SAFETY: the user_callback may not make any vulkan calls
	fn debug_message(m: &Message) {
		let error = format!(
			"[{}] {}{}: {}",
			Self::debug_severity_string(m.severity),
			m.layer_prefix.unwrap_or("Unknown"),
			if m.ty.contains(DebugUtilsMessageType::VALIDATION) {
				" (Validation)"
			} else if m.ty.contains(DebugUtilsMessageType::PERFORMANCE) {
				" (Performance)"
			} else if m.ty.contains(DebugUtilsMessageType::GENERAL) {
				""
			} else {
				unreachable!()
			},
			m.description
		);
		if m.severity.contains(DebugUtilsMessageSeverity::ERROR) {
			panic!("{}", error);
		} else {
			println!("{}", error);
		}
	}

	pub fn debug_severity_string(a: DebugUtilsMessageSeverity) -> &'static str {
		if a.contains(DebugUtilsMessageSeverity::ERROR) {
			"Error"
		} else if a.contains(DebugUtilsMessageSeverity::WARNING) {
			"Warn"
		} else if a.contains(DebugUtilsMessageSeverity::INFO) {
			"Info"
		} else if a.contains(DebugUtilsMessageSeverity::VERBOSE) {
			"Verbose"
		} else {
			unreachable!();
		}
	}

	pub fn debug_type_string(a: DebugUtilsMessageType) -> &'static str {
		if a.contains(DebugUtilsMessageType::VALIDATION) {
			"Validation"
		} else if a.contains(DebugUtilsMessageType::PERFORMANCE) {
			"Performance"
		} else if a.contains(DebugUtilsMessageType::GENERAL) {
			"General"
		} else {
			unreachable!()
		}
	}
}
use vulkano::Version;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ApplicationVersion {
	pub major: u32,
	pub minor: u32,
	pub patch: u32,
}

impl From<ApplicationVersion> for Version {
	fn from(a: ApplicationVersion) -> Self {
		Version {
			major: a.major,
			minor: a.minor,
			patch: a.patch,
		}
	}
}

#[derive(Copy, Clone)]
pub struct ApplicationConfig {
	pub name: &'static str,
	pub version: ApplicationVersion,
}

pub const fn compile_time_parse(input: &'static str) -> u32 {
	match konst::primitive::parse_u32(input) {
		Ok(e) => e,
		Err(_) => unreachable!()
	}
}

#[macro_export]
macro_rules! generate_application_config {
    () => {
        $crate::application_config::ApplicationConfig {
            name: env!("CARGO_PKG_NAME"),
            version: $crate::application_config::ApplicationVersion {
                major: $crate::application_config::compile_time_parse(env!("CARGO_PKG_VERSION_MAJOR")),
                minor: $crate::application_config::compile_time_parse(env!("CARGO_PKG_VERSION_MINOR")),
                patch: $crate::application_config::compile_time_parse(env!("CARGO_PKG_VERSION_PATCH")),
            }
        }
    };
}

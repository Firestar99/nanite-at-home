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

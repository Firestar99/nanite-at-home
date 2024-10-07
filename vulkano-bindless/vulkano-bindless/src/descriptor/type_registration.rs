use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::OnceLock;
use vulkano_bindless_shaders::descriptor::DescriptorType;

pub const RESERVED_TABLES: u32 = 3;
static TABLE_ID_CNT: AtomicU32 = AtomicU32::new(RESERVED_TABLES);

pub struct DescriptorTypeRegistration(OnceLock<DescriptorType>);

impl DescriptorTypeRegistration {
	pub const fn new() -> Self {
		Self(OnceLock::new())
	}

	pub(crate) const fn new_reserved(id: u32) -> Self {
		assert!(id < RESERVED_TABLES, "not a reserved id");
		unsafe {
			Self(OnceLock::from(match DescriptorType::new(id) {
				None => panic!(),
				Some(e) => e,
			}))
		}
	}

	pub fn get(&self) -> DescriptorType {
		*self.0.get_or_init(|| {
			let id = TABLE_ID_CNT.fetch_add(1, Relaxed);
			unsafe { DescriptorType::new(id) }.expect("DescriptorType allocation failed due to running out of ids")
		})
	}
}

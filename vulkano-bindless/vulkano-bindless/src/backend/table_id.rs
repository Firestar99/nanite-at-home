use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::OnceLock;
use vulkano_bindless_shaders::descriptor::ID_TYPE_BITS;

pub const TABLE_COUNT: u32 = 1 << ID_TYPE_BITS;
pub const RESERVED_TABLES: u32 = 3;
pub const BUFFER_TABLE_ID: TableId = unsafe { TableId::new_unchecked(0) };
pub const STORAGE_IMAGE_TABLE_ID: TableId = unsafe { TableId::new_unchecked(1) };
pub const SAMPLED_IMAGE_TABLE_ID: TableId = unsafe { TableId::new_unchecked(2) };
pub const SAMPLER_TABLE_ID: TableId = unsafe { TableId::new_unchecked(3) };

#[derive(Clone, Copy, Debug)]
pub struct TableId(u32);

impl TableId {
	const unsafe fn new_unchecked(id: u32) -> Self {
		Self(id)
	}

	pub const fn to_u32(&self) -> u32 {
		self.0
	}

	pub const fn to_usize(&self) -> usize {
		self.0 as usize
	}
}

static TABLE_ID_CNT: AtomicU32 = AtomicU32::new(RESERVED_TABLES);

#[derive(Debug)]
pub struct TableIdRegistration {
	once: OnceLock<TableId>,
}

impl TableIdRegistration {
	pub fn new() -> Self {
		Self { once: OnceLock::new() }
	}

	pub fn get(&self) -> TableId {
		*self.once.get_or_init(|| {
			let id = TABLE_ID_CNT.fetch_add(1, Relaxed);
			if id < TABLE_COUNT {
				TableId(id)
			} else {
				panic!("ran out of table ids")
			}
		})
	}
}

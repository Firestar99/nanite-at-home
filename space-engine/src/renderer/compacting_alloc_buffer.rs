use rust_gpu_bindless::buffer_content::BufferStructPlain;
use rust_gpu_bindless::descriptor::{
	Bindless, BindlessAllocationScheme, BindlessBufferCreateInfo, BindlessBufferUsage, Buffer, MutBuffer, MutDesc,
	RCDesc,
};
use rust_gpu_bindless::pipeline::{
	AccessError, GeneralRead, MutBufferAccess, MutBufferAccessExt, Recording, RecordingError, ShaderRead,
	ShaderReadWrite, TransferWrite,
};
use space_engine_shader::renderer::compacting_alloc_buffer::{
	CompactingAllocBufferReader, CompactingAllocBufferWriter,
};
use std::sync::Arc;

pub struct CompactingAllocBuffer<T: BufferStructPlain> {
	buffer: MutDesc<MutBuffer<[T]>>,
	indirect_args: MutDesc<MutBuffer<[u32; 3]>>,
	indirect_args_default: RCDesc<Buffer<[u32; 3]>>,
}

impl<T: BufferStructPlain> CompactingAllocBuffer<T> {
	pub fn new(
		bindless: &Arc<Bindless>,
		capacity: usize,
		indirect_args_default: [u32; 3],
		name: &str,
	) -> anyhow::Result<Self> {
		let buffer = bindless.buffer().alloc_slice(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::STORAGE_BUFFER,
				name: &format!("CompactingAllocBuffer {} buffer", name),
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			},
			capacity,
		)?;
		let indirect_args = bindless.buffer().alloc_sized(&BindlessBufferCreateInfo {
			usage: BindlessBufferUsage::STORAGE_BUFFER
				| BindlessBufferUsage::INDIRECT_BUFFER
				| BindlessBufferUsage::TRANSFER_DST,
			name: &format!("CompactingAllocBuffer {} indirect args", name),
			allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
		})?;
		let indirect_args_default = bindless.buffer().alloc_shared_from_data(
			&BindlessBufferCreateInfo {
				usage: BindlessBufferUsage::STORAGE_BUFFER | BindlessBufferUsage::TRANSFER_SRC,
				name: &format!("CompactingAllocBuffer {} indirect args", name),
				allocation_scheme: BindlessAllocationScheme::AllocatorManaged,
			},
			indirect_args_default,
		)?;
		Ok(Self {
			buffer,
			indirect_args,
			indirect_args_default,
		})
	}

	pub fn transition_writing<'a>(
		self,
		cmd: &mut Recording<'a>,
	) -> Result<CompactingAllocBufferWriting<'a, T>, RecordingError> {
		let indirect_args = self.indirect_args.access::<TransferWrite>(cmd)?;
		cmd.copy_buffer_to_buffer(&self.indirect_args_default, &indirect_args)?;
		Ok(CompactingAllocBufferWriting {
			buffer: self.buffer.access(cmd)?,
			indirect_args: indirect_args.transition()?,
			indirect_args_default: self.indirect_args_default,
		})
	}
}

pub struct CompactingAllocBufferWriting<'a, T: BufferStructPlain> {
	buffer: MutBufferAccess<'a, [T], ShaderReadWrite>,
	indirect_args: MutBufferAccess<'a, [u32; 3], ShaderReadWrite>,
	indirect_args_default: RCDesc<Buffer<[u32; 3]>>,
}

impl<'a, T: BufferStructPlain> CompactingAllocBufferWriting<'a, T> {
	pub fn to_writer(&self) -> Result<CompactingAllocBufferWriter<'_, T>, AccessError> {
		Ok(CompactingAllocBufferWriter {
			buffer: self.buffer.to_mut_transient()?,
			indirect_args: self.indirect_args.to_mut_transient()?,
		})
	}

	pub fn transition_reading(self) -> anyhow::Result<CompactingAllocBufferReading<'a, T>> {
		Ok(CompactingAllocBufferReading {
			buffer: self.buffer.transition()?,
			indirect_args: self.indirect_args.transition()?,
			indirect_args_default: self.indirect_args_default,
		})
	}
}

pub struct CompactingAllocBufferReading<'a, T: BufferStructPlain> {
	buffer: MutBufferAccess<'a, [T], ShaderRead>,
	indirect_args: MutBufferAccess<'a, [u32; 3], GeneralRead>,
	indirect_args_default: RCDesc<Buffer<[u32; 3]>>,
}

impl<'a, T: BufferStructPlain> CompactingAllocBufferReading<'a, T> {
	pub fn to_reader(&self) -> Result<CompactingAllocBufferReader<'_, T>, AccessError> {
		Ok(CompactingAllocBufferReader {
			buffer: self.buffer.to_transient()?,
			indirect_args: self.indirect_args.to_transient()?,
		})
	}

	pub fn indirect_args(&self) -> &MutBufferAccess<'a, [u32; 3], GeneralRead> {
		&self.indirect_args
	}

	pub fn transition_reset(self) -> CompactingAllocBuffer<T> {
		CompactingAllocBuffer {
			buffer: self.buffer.into_desc(),
			indirect_args: self.indirect_args.into_desc(),
			indirect_args_default: self.indirect_args_default,
		}
	}
}

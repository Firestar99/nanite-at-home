use crate::material::pbr::process_pbr_material;
use crate::meshlet::error::{Error, MeshletError};
use crate::uri::Scheme;
use glam::{Affine3A, Mat3, Quat, Vec3};
use gltf::buffer::Data;
use gltf::image::Source;
use gltf::mesh::Mode;
use gltf::{Buffer, Document, Image, Node, Primitive, Scene};
use meshopt::VertexDataAdapter;
use rayon::prelude::*;
use smallvec::SmallVec;
use space_asset::image::{DiskImageCompression, Image2DDisk, Image2DMetadata, Size};
use space_asset::meshlet::indices::triangle_indices_write_vec;
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
use space_asset::meshlet::offset::MeshletOffset;
use space_asset::meshlet::scene::MeshletSceneDisk;
use space_asset::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{io, mem};
use zune_image::codecs::png::zune_core::bytestream::ZCursor;
use zune_image::codecs::png::zune_core::options::DecoderOptions;
use zune_image::codecs::ImageFormat;
use zune_image::errors::ImageErrors;

pub struct Gltf {
	pub document: Document,
	pub base: PathBuf,
	pub buffers: SmallVec<[Data; 1]>,
}

impl Gltf {
	#[profiling::function]
	pub fn open(path: &Path) -> crate::meshlet::error::Result<Arc<Self>> {
		let base = path
			.parent()
			.map(Path::to_path_buf)
			.unwrap_or_else(|| PathBuf::from("./"));
		let gltf::Gltf { document, mut blob } = gltf::Gltf::open(&path).map_err(Error::from)?;
		let buffers = document
			.buffers()
			.map(|buffer| {
				Data::from_source_and_blob(buffer.source(), Some(base.as_path()), &mut blob).map_err(Error::from)
			})
			.collect::<crate::meshlet::error::Result<_>>()?;
		Ok(Arc::new(Self {
			document,
			base,
			buffers,
		}))
	}

	pub fn base(&self) -> &Path {
		self.base.as_path()
	}

	pub fn buffer(&self, buffer: Buffer) -> Option<&[u8]> {
		self.buffers.get(buffer.index()).map(|b| &b.0[..])
	}
}

#[derive(Debug)]
pub enum GltfImageError {
	MissingBuffer,
	BufferViewOutOfBounds,
	UnsupportedUri,
	UnknownImageFormat,
	ImageErrors(ImageErrors),
	IoError(io::Error),
}

impl Display for GltfImageError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			GltfImageError::MissingBuffer => f.write_str("Invalid buffer index"),
			GltfImageError::BufferViewOutOfBounds => f.write_str("Buffer view is out of bounds"),
			GltfImageError::UnsupportedUri => f.write_str("Image URI is unsupported or invalid"),
			GltfImageError::UnknownImageFormat => f.write_str("Image format is unknown"),
			GltfImageError::ImageErrors(err) => Display::fmt(err, f),
			GltfImageError::IoError(err) => Display::fmt(err, f),
		}
	}
}

impl std::error::Error for GltfImageError {}

impl From<ImageErrors> for GltfImageError {
	fn from(value: ImageErrors) -> Self {
		Self::ImageErrors(value)
	}
}

impl From<io::Error> for GltfImageError {
	fn from(value: io::Error) -> Self {
		Self::IoError(value)
	}
}

impl Gltf {
	#[profiling::function]
	pub fn image<const DATA_TYPE: u32>(&self, image: Image) -> Result<Image2DDisk<DATA_TYPE>, GltfImageError> {
		let scheme = match image.source() {
			Source::View { view, .. } => {
				let buffer = self.buffer(view.buffer()).ok_or(GltfImageError::MissingBuffer)?;
				Scheme::Slice(
					&buffer
						.get(view.offset()..view.length())
						.ok_or(GltfImageError::BufferViewOutOfBounds)?,
				)
			}
			Source::Uri { uri, .. } => Scheme::parse(uri).ok_or(GltfImageError::UnsupportedUri)?,
		};

		let src = {
			profiling::scope!("read into memory");
			scheme.read(self.base())?
		};
		let (format, _) = ImageFormat::guess_format(ZCursor::new(&src)).ok_or(GltfImageError::UnknownImageFormat)?;
		let metadata = {
			profiling::scope!("decode metadata");
			format
				.decoder_with_options(ZCursor::new(&src), DecoderOptions::new_fast())?
				.read_headers()
				.map_err(ImageErrors::from)?
				.expect("Image decoder reads metadata")
		};
		let size = Size::new(metadata.dimensions().0 as u32, metadata.dimensions().1 as u32);

		Ok(Image2DDisk {
			metadata: Image2DMetadata {
				size,
				disk_compression: DiskImageCompression::Embedded,
			},
			bytes: src.into(),
		})
	}
}

impl Deref for Gltf {
	type Target = Document;

	fn deref(&self) -> &Self::Target {
		&self.document
	}
}

impl Gltf {
	pub fn process(self: &Arc<Self>) -> crate::meshlet::error::Result<MeshletSceneDisk> {
		profiling::scope!("Gltf::process");

		let meshes_primitives = {
			profiling::scope!("process meshes");
			self.meshes()
				.collect::<Vec<_>>()
				.into_par_iter()
				.map(|mesh| {
					let vec = mesh.primitives().collect::<SmallVec<[_; 4]>>();
					vec.into_par_iter()
						.map(|primitive| self.process_mesh_primitive(primitive.clone()))
						.collect::<crate::meshlet::error::Result<Vec<_>>>()
				})
				.collect::<crate::meshlet::error::Result<Vec<_>>>()
		}?;

		let mesh2instance = {
			profiling::scope!("instance transformations");
			let scene = self.default_scene().ok_or(Error::from(MeshletError::NoDefaultScene))?;
			let node_transforms = self.compute_transformations(
				&scene,
				Affine3A::from_mat3(Mat3 {
					y_axis: Vec3::new(0., -1., 0.),
					..Mat3::default()
				}),
			);
			let mut mesh2instance = (0..self.meshes().len()).map(|_| Vec::new()).collect::<Vec<_>>();
			for node in self.nodes() {
				if let Some(mesh) = node.mesh() {
					mesh2instance[mesh.index()].push(MeshletInstance::new(node_transforms[node.index()]));
				}
			}
			mesh2instance
		};

		Ok(MeshletSceneDisk {
			mesh2instances: meshes_primitives
				.into_iter()
				.zip(mesh2instance.into_iter())
				.flat_map(|(mesh_primitives, instances)| {
					mesh_primitives
						.into_iter()
						.map(move |primitive| MeshletMesh2InstanceDisk {
							mesh: primitive,
							instances: instances.clone(),
						})
				})
				.collect(),
		})
	}

	fn compute_transformations(&self, scene: &Scene, base: Affine3A) -> Vec<Affine3A> {
		fn walk(out: &mut Vec<Affine3A>, node: Node, parent: Affine3A) {
			let (translation, rotation, scale) = node.transform().decomposed();
			let node_absolute = parent
				* Affine3A::from_scale_rotation_translation(
					Vec3::from(scale),
					Quat::from_array(rotation),
					Vec3::from(translation),
				);
			out[node.index()] = node_absolute;
			for node in node.children() {
				walk(out, node, node_absolute);
			}
		}

		let mut out = vec![Affine3A::IDENTITY; self.nodes().len()];
		for node in scene.nodes() {
			walk(&mut out, node, base);
		}
		out
	}

	#[profiling::function]
	fn process_mesh_primitive(
		self: &Arc<Gltf>,
		primitive: Primitive,
	) -> crate::meshlet::error::Result<MeshletMeshDisk> {
		if primitive.mode() != Mode::Triangles {
			return Err(MeshletError::PrimitiveMustBeTriangleList.into());
		}

		let reader = primitive.reader(|b| self.buffer(b));
		let vertex_positions: Vec<_> = reader
			.read_positions()
			.ok_or(Error::from(MeshletError::NoVertexPositions))?
			.map(|pos| Vec3::from(pos))
			.collect();

		let mut indices: Vec<_> = if let Some(indices) = reader.read_indices() {
			indices.into_u32().collect()
		} else {
			(0..vertex_positions.len() as u32).collect()
		};

		{
			profiling::scope!("meshopt::optimize_vertex_cache");
			meshopt::optimize_vertex_cache_in_place(&mut indices, vertex_positions.len());
		}

		let out = {
			let adapter =
				VertexDataAdapter::new(bytemuck::cast_slice(&*vertex_positions), mem::size_of::<Vec3>(), 0).unwrap();
			let mut out = {
				profiling::scope!("meshopt::build_meshlets");
				meshopt::build_meshlets(
					&indices,
					&adapter,
					MESHLET_MAX_VERTICES as usize,
					MESHLET_MAX_TRIANGLES as usize,
					0.,
				)
			};
			// resize vertex buffer appropriately
			out.vertices.truncate(
				out.meshlets
					.last()
					.map(|m| m.vertex_offset as usize + m.vertex_count as usize)
					.unwrap_or(0),
			);
			out
		};

		let indices = out.iter().flat_map(|m| m.triangles).copied().collect::<Vec<_>>();
		let triangles = triangle_indices_write_vec(indices.iter().copied().map(u32::from));

		let draw_vertices = out
			.vertices
			.into_iter()
			.map(|i| {
				DrawVertex {
					position: vertex_positions[i as usize],
					material_vertex_id: MaterialVertexId(i),
				}
				.encode()
			})
			.collect();

		let mut triangle_start = 0;
		let meshlets = out
			.meshlets
			.into_iter()
			.map(|m| {
				let data = MeshletData {
					draw_vertex_offset: MeshletOffset::new(m.vertex_offset as usize, m.vertex_count as usize),
					triangle_offset: MeshletOffset::new(triangle_start, m.triangle_count as usize),
				};
				triangle_start += m.triangle_count as usize;
				data
			})
			.collect();

		Ok(MeshletMeshDisk {
			draw_vertices,
			meshlets,
			triangles,
			pbr_material: process_pbr_material(self, primitive)?,
		})
	}
}

use crate::material::pbr::process_pbr_material;
use crate::meshlet::error::{Error, MeshletError, Result};
use glam::{Affine3A, Mat3, Quat, Vec3};
use gltf::buffer::Data;
use gltf::image::Format;
use gltf::mesh::Mode;
use gltf::{Buffer, Document, Image, Node, Primitive, Scene};
use image::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use meshopt::VertexDataAdapter;
use rayon::prelude::*;
use smallvec::SmallVec;
use space_asset::meshlet::indices::triangle_indices_write_vec;
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
use space_asset::meshlet::offset::MeshletOffset;
use space_asset::meshlet::scene::MeshletSceneDisk;
use space_asset::meshlet::vertex::{DrawVertex, MaterialVertexId};
use space_asset::meshlet::{MESHLET_MAX_TRIANGLES, MESHLET_MAX_VERTICES};
use std::mem;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Gltf {
	pub document: Document,
	pub base: Option<PathBuf>,
	pub buffers: SmallVec<[Data; 1]>,
}

impl Gltf {
	#[profiling::function]
	pub fn open(path: PathBuf) -> Result<Arc<Self>> {
		let base = Some(
			path.parent()
				.map(Path::to_path_buf)
				.unwrap_or_else(|| PathBuf::from("./")),
		);
		let gltf::Gltf { document, mut blob } = gltf::Gltf::open(&path).map_err(Error::from)?;
		let buffers = document
			.buffers()
			.map(|buffer| {
				Data::from_source_and_blob(buffer.source(), base.as_ref().map(PathBuf::as_path), &mut blob)
					.map_err(Error::from)
			})
			.collect::<Result<_>>()?;
		Ok(Arc::new(Self {
			document,
			base,
			buffers,
		}))
	}

	pub fn base(&self) -> Option<&Path> {
		self.base.as_ref().map(PathBuf::as_path)
	}

	pub fn buffer(&self, buffer: Buffer) -> Option<&[u8]> {
		self.buffers.get(buffer.index()).map(|b| &b.0[..])
	}

	pub fn image(&self, image: Image) -> gltf::Result<DynamicImage> {
		let data =
			gltf::image::Data::from_source(image.source(), self.base.as_ref().map(PathBuf::as_path), &self.buffers)?;

		// gltf converts image to its own format, we convert it back
		Ok(match data.format {
			Format::R8 => DynamicImage::ImageLuma8(GrayImage::from_vec(data.width, data.height, data.pixels).unwrap()),
			Format::R8G8 => {
				DynamicImage::ImageLumaA8(GrayAlphaImage::from_vec(data.width, data.height, data.pixels).unwrap())
			}
			Format::R8G8B8 => {
				DynamicImage::ImageRgb8(RgbImage::from_vec(data.width, data.height, data.pixels).unwrap())
			}
			Format::R8G8B8A8 => {
				DynamicImage::ImageRgba8(RgbaImage::from_vec(data.width, data.height, data.pixels).unwrap())
			}
			_ => return Err(gltf::Error::UnsupportedImageEncoding),
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
	pub fn process(self: &Arc<Self>) -> Result<MeshletSceneDisk> {
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
						.collect::<Result<Vec<_>>>()
				})
				.collect::<Result<Vec<_>>>()
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
	fn process_mesh_primitive(self: &Arc<Gltf>, primitive: Primitive) -> Result<MeshletMeshDisk> {
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

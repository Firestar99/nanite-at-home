use crate::meshlet::error::{Error, MeshletError, Result};
use glam::{Affine3A, Quat, Vec3};
use gltf::buffer::Data;
use gltf::mesh::Mode;
use gltf::{Buffer, Document, Node, Scene};
use memoffset::offset_of;
use meshopt::VertexDataAdapter;
use smallvec::SmallVec;
use space_asset::meshlet::indices::triangle_indices_write_vec;
use space_asset::meshlet::instance::MeshletInstance;
use space_asset::meshlet::mesh::{MeshletData, MeshletMeshDisk};
use space_asset::meshlet::mesh2instance::MeshletMesh2InstanceDisk;
use space_asset::meshlet::offset::MeshletOffset;
use space_asset::meshlet::scene::MeshletSceneDisk;
use space_asset::meshlet::vertex::MeshletDrawVertex;
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
}

impl Deref for Gltf {
	type Target = Document;

	fn deref(&self) -> &Self::Target {
		&self.document
	}
}

impl Gltf {
	pub async fn process(self: &Arc<Self>) -> Result<MeshletSceneDisk> {
		profiling::scope!("Gltf::process");
		let meshes_primitives = futures::future::join_all(self.meshes().map(|mesh| {
			futures::future::join_all(mesh.primitives().map(|primitive| {
				let gltf = self.clone();
				let mesh_id = mesh.index();
				let primitive_id = primitive.index();
				smol::spawn(async move { gltf.process_mesh_primitive(mesh_id, primitive_id) })
			}))
		}));

		let mesh2instance = {
			profiling::scope!("instance transformations");
			let scene = self.default_scene().ok_or(Error::from(MeshletError::NoDefaultScene))?;
			let node_transforms = self.compute_transformations(&scene);
			let mut mesh2instance = (0..self.meshes().len()).map(|_| Vec::new()).collect::<Vec<_>>();
			for node in self.nodes() {
				if let Some(mesh) = node.mesh() {
					mesh2instance[mesh.index()].push(MeshletInstance::new(node_transforms[node.index()]));
				}
			}
			mesh2instance
		};

		let meshes_primitives = {
			profiling::scope!("await meshes");
			meshes_primitives
				.await
				.into_iter()
				.map(|v| v.into_iter().collect::<Result<Vec<_>>>())
				.collect::<Result<Vec<_>>>()?
		};

		Ok(MeshletSceneDisk {
			mesh2instance: meshes_primitives
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

	fn compute_transformations(&self, scene: &Scene) -> Vec<Affine3A> {
		fn walk(out: &mut Vec<Affine3A>, node: Node, parent: Affine3A) {
			let (translation, rotation, scale) = node.transform().decomposed();
			let node_absolute = parent
				* Affine3A::from_scale_rotation_translation(
					Vec3::from(translation),
					Quat::from_array(rotation),
					Vec3::from(scale),
				);
			out[node.index()] = node_absolute;
			for node in node.children() {
				walk(out, node, node_absolute);
			}
		}

		let mut out = vec![Affine3A::IDENTITY; self.nodes().len()];
		for node in scene.nodes() {
			walk(&mut out, node, Affine3A::IDENTITY);
		}
		out
	}

	#[profiling::function]
	fn process_mesh_primitive(self: &Arc<Gltf>, mesh_id: usize, primitive_id: usize) -> Result<MeshletMeshDisk> {
		let mesh = self.meshes().skip(mesh_id).next().unwrap();
		let primitive = mesh.primitives().nth(primitive_id).unwrap();
		if primitive.mode() != Mode::Triangles {
			return Err(MeshletError::PrimitiveMustBeTriangleList.into());
		}

		let reader = primitive.reader(|b| self.buffer(b));
		let vertices: Vec<_> = reader
			.read_positions()
			.ok_or(Error::from(MeshletError::NoVertexPositions))?
			.map(|pos| MeshletDrawVertex::new(Vec3::from(pos)))
			.collect();
		let indices: Vec<_> = if let Some(indices) = reader.read_indices() {
			indices.into_u32().collect()
		} else {
			(0..vertices.len() as u32).collect()
		};

		let out = {
			let adapter = VertexDataAdapter::new(
				bytemuck::cast_slice(&*vertices),
				mem::size_of::<MeshletDrawVertex>(),
				offset_of!(MeshletDrawVertex, position),
			)
			.unwrap();
			let mut out = meshopt::build_meshlets(
				&indices,
				&adapter,
				MESHLET_MAX_VERTICES as usize,
				MESHLET_MAX_TRIANGLES as usize,
				0.,
			);
			// convert meshopt triangle offset from #N of indices to #N of triangles
			for meshlet in &mut out.meshlets {
				meshlet.triangle_offset /= 3;
			}
			// resize vertex and index buffers appropriately
			let (max_vertices, max_triangles) = out
				.meshlets
				.last()
				.map(|m| {
					(
						m.vertex_offset as usize + m.vertex_count as usize,
						m.triangle_offset as usize + m.triangle_count as usize,
					)
				})
				.unwrap_or((0, 0));
			out.vertices.truncate(max_vertices);
			out.triangles.truncate(max_triangles * 3);
			out
		};

		Ok(MeshletMeshDisk {
			draw_vertices: out.vertices.into_iter().map(|i| vertices[i as usize]).collect(),
			meshlets: out
				.meshlets
				.into_iter()
				.map(|m| MeshletData {
					draw_vertex_offset: MeshletOffset::new(m.vertex_offset as usize, m.vertex_count as usize),
					triangle_offset: MeshletOffset::new(m.triangle_offset as usize, m.triangle_count as usize),
				})
				.collect(),
			triangle_indices: triangle_indices_write_vec(out.triangles.into_iter().map(|i| i as u32)),
		})
	}
}

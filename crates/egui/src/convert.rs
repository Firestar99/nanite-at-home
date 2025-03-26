use egui::{TextureFilter, TextureWrapMode};
use rust_gpu_bindless_core::descriptor::{AddressMode, Filter};

pub trait Egui2Bindless {
	type Output;
	fn to_bindless(&self) -> Self::Output;
}

impl Egui2Bindless for TextureFilter {
	type Output = Filter;

	fn to_bindless(&self) -> Self::Output {
		match self {
			TextureFilter::Nearest => Filter::Nearest,
			TextureFilter::Linear => Filter::Linear,
		}
	}
}

impl Egui2Bindless for TextureWrapMode {
	type Output = AddressMode;

	fn to_bindless(&self) -> Self::Output {
		match self {
			TextureWrapMode::ClampToEdge => AddressMode::ClampToEdge,
			TextureWrapMode::Repeat => AddressMode::Repeat,
			TextureWrapMode::MirroredRepeat => AddressMode::MirrorRepeat,
		}
	}
}

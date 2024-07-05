use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::iter::Peekable;

/// ModNode is a helper for writing a mod hierarchy for various symbols
pub enum ModNode<'a, T: ModNodeToTokens> {
	Children(HashMap<Cow<'a, str>, ModNode<'a, T>>),
	Object(T),
}

impl<'a, T: ModNodeToTokens> ModNode<'a, T> {
	pub fn root() -> Self {
		Self::Children(HashMap::new())
	}

	pub fn insert(&mut self, path: impl Iterator<Item = Cow<'a, str>>, t: T) -> Result<(), ModNodeError> {
		self.insert_inner(path.peekable(), t)
	}

	pub fn insert_inner(
		&mut self,
		mut path: Peekable<impl Iterator<Item = Cow<'a, str>>>,
		t: T,
	) -> Result<(), ModNodeError> {
		if let Some(seg) = path.next() {
			match self {
				ModNode::Children(children) => {
					if let None = path.peek() {
						match children.insert(seg, Self::Object(t)) {
							Some(ModNode::Object(_)) => Err(ModNodeError::ObjectsNameCollision),
							Some(ModNode::Children(_)) => Err(ModNodeError::ModuleAndObjectNameCollision),
							None => Ok(()),
						}
					} else {
						children
							.entry(seg)
							.or_insert(Self::Children(HashMap::new()))
							.insert_inner(path, t)
					}
				}
				ModNode::Object(_) => {
					if let None = path.peek() {
						Err(ModNodeError::ObjectsNameCollision)
					} else {
						Err(ModNodeError::ModuleAndObjectNameCollision)
					}
				}
			}
		} else {
			return Err(ModNodeError::NoName);
		}
	}

	pub fn to_tokens(&self) -> TokenStream {
		match self {
			ModNode::Children(children) => {
				let mut content = quote!();
				for (name, node) in children {
					let append = node.to_tokens_with_ident(format_ident!("{}", name));
					content = quote! {
						#content
						#append
					};
				}
				content
			}
			ModNode::Object(_) => unreachable!(),
		}
	}
}

pub trait ModNodeToTokens {
	fn to_tokens_with_ident(&self, name: Ident) -> TokenStream;
}

impl<'a, T: ModNodeToTokens> ModNodeToTokens for ModNode<'a, T> {
	fn to_tokens_with_ident(&self, name: Ident) -> TokenStream {
		match self {
			ModNode::Children(_) => {
				let content = self.to_tokens();
				quote! {
					pub mod #name {
						#content
					}
				}
			}
			ModNode::Object(t) => t.to_tokens_with_ident(name),
		}
	}
}

#[derive(Debug)]
pub enum ModNodeError {
	NoName,
	ObjectsNameCollision,
	ModuleAndObjectNameCollision,
}

impl Error for ModNodeError {}

impl Display for ModNodeError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			ModNodeError::NoName => f.write_str("An object had no name!"),
			ModNodeError::ObjectsNameCollision => f.write_str("Two objects have the same name!"),
			ModNodeError::ModuleAndObjectNameCollision => f.write_str("An object has the same name as a module!"),
		}
	}
}

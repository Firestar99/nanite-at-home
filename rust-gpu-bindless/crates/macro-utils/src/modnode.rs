use proc_macro2::{Ident, TokenStream};
use quote::{TokenStreamExt, format_ident, quote};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::iter::Peekable;

/// ModNode is a helper for writing a mod hierarchy for various symbols
pub enum ModNode<'a, T> {
	Children(HashMap<Cow<'a, str>, ModNode<'a, T>>),
	Object(T),
}

impl<'a, T> ModNode<'a, T> {
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
					if path.peek().is_none() {
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
					if path.peek().is_none() {
						Err(ModNodeError::ObjectsNameCollision)
					} else {
						Err(ModNodeError::ModuleAndObjectNameCollision)
					}
				}
			}
		} else {
			Err(ModNodeError::NoName)
		}
	}

	pub fn to_tokens(&self, mut f: impl FnMut(Ident, &T) -> TokenStream) -> TokenStream {
		match self {
			ModNode::Children(children) => self.to_tokens_children(children, &mut f),
			ModNode::Object(_) => unreachable!(),
		}
	}

	fn to_tokens_loop(&self, name: Ident, f: &mut impl FnMut(Ident, &T) -> TokenStream) -> TokenStream {
		match self {
			ModNode::Children(children) => {
				let content = self.to_tokens_children(children, f);
				quote! {
					pub mod #name {
						#content
					}
				}
			}
			ModNode::Object(t) => f(name, t),
		}
	}

	fn to_tokens_children(
		&self,
		children: &HashMap<Cow<'a, str>, ModNode<'a, T>>,
		f: &mut impl FnMut(Ident, &T) -> TokenStream,
	) -> TokenStream {
		let mut content = quote!();
		for (name, node) in children {
			content.append_all(node.to_tokens_loop(format_ident!("{}", name), f));
		}
		content
	}

	pub fn iter(&'a self, mut f: impl FnMut(&[&'a str], &T)) {
		fn inner<'a, T>(path: &[&'a str], node: &'a ModNode<'a, T>, f: &mut impl FnMut(&[&'a str], &T)) {
			match node {
				ModNode::Children(children) => {
					let mut path = SmallVec::<[&str; 6]>::from(path);
					for (name, node) in children {
						path.push(name);
						inner(&path, node, f);
						path.pop();
					}
				}
				ModNode::Object(t) => f(path, t),
			};
		}
		inner(&[], self, &mut f);
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

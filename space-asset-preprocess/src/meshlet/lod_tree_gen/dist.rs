use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::hash::Hash;

#[derive(Clone)]
pub struct Dist<K: Copy + Debug + Ord + Hash>(pub Vec<(K, usize)>);

impl<K: Copy + Debug + Ord + Hash> Dist<K> {
	pub fn new(iter: impl Iterator<Item = K>) -> Self {
		let mut dist = iter
			.fold(HashMap::new(), |mut h, v| {
				*h.entry(v).or_insert(0usize) += 1;
				h
			})
			.into_iter()
			.collect::<Vec<_>>();
		dist.sort_by_key(|(k, _)| *k);
		Self(dist)
	}
}

impl<K: Copy + Debug + Ord + Hash> Debug for Dist<K> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let mut debug = f.debug_map();
		for (k, cnt) in &self.0 {
			debug.entry(k, cnt);
		}
		debug.finish()
	}
}

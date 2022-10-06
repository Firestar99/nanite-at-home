use std::pin::Pin;
use std::sync::Arc;

fn main() {
	let arc = Arc::new(42);
	let pin = Pin::new(arc.clone());
	Arc::pin()
}
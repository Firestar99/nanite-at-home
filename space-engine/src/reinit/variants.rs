use std::sync::Arc;
use crate::reinit::{Dependency, Reinit, ReinitDetails, ReinitRef, Restart};
use paste::paste;

/// T: Reinit type
/// X: constructor
/// A..P: dependent Reinit types
macro_rules! struct_decl {
    (
		$num:literal,
		<$($x:ident),+>
	) => (paste!{
		struct [<Reinit $num>]<T: 'static, X, $($x: 'static,)+>
			where
				X: Fn($(ReinitRef<$x>,)+ Restart<T>) -> T + 'static
		{
			parent: WeakReinit<T>,
			constructor: X,
			$([<$x:lower>]: Dependency<$x>),+
		}

		impl<T: 'static> Reinit<T>
		{
			pub fn [<new $num>]<X, $($x: 'static,)+>($([<$x:lower>]: &Reinit<$x>,)+ constructor: X) -> Reinit<T>
				where
					X: Fn($(ReinitRef<$x>,)+ Restart<T>) -> T + 'static
			{
				Reinit::new($num, |weak| {
					Arc::new([<Reinit $num>] {
						parent: WeakReinit::new(weak),
						$([<$x:lower>]: Dependency::new([<$x:lower>].clone()),)+
						constructor,
					})
				}, |arc| {
					$(arc.[<$x:lower>].reinit.add_callback(arc, [<Reinit $num>]::[<accept_ $x:lower>], [<Reinit $num>]::[<request_drop_ $x:lower>]);)+
				})
			}
		}

		impl<T: 'static, X, $($x: 'static,)+> ReinitDetails<T> for [<Reinit $num>]<T, X, $($x,)+>
			where
				X: Fn($(ReinitRef<$x>,)+ Restart<T>) -> T + 'static
		{
			fn request_construction(&self, parent: &Reinit<T>) {
				parent.constructed((self.constructor)($(self.[<$x:lower>].value_get().clone(),)+ Restart::new(&self.parent)));
			}
		}

		impl<T: 'static, X, $($x: 'static,)+> [<Reinit $num>]<T, X, $($x,)+>
			where
				X: Fn($(ReinitRef<$x>,)+ Restart<T>) -> T + 'static
		{
			$(
			fn [<accept_ $x:lower>](self: Arc<Self>, t: ReinitRef<$x>) {
				if let Some(parent) = self.parent.upgrade() {
					self.[<$x:lower>].value_set(t);
					parent.construct_countdown();
				}
			}
			)+

			$(
			fn [<request_drop_ $x:lower>](self: Arc<Self>) {
				if let Some(parent) = self.parent.upgrade() {
					self.[<$x:lower>].value_clear();
					parent.construct_countup();
				}
			}
			)+
		}
    })
}

// struct_decl!(1, <A>);
// struct_decl!(2, <A, B>);
// struct_decl!(3, <A, B, C>);
// struct_decl!(4, <A, B, C, D>);
// struct_decl!(5, <A, B, C, D, E>);
// struct_decl!(6, <A, B, C, D, E, F>);
// struct_decl!(7, <A, B, C, D, E, F, G>);
// struct_decl!(8, <A, B, C, D, E, F, G, H>);
// struct_decl!(9, <A, B, C, D, E, F, G, H, I>);
// struct_decl!(10, <A, B, C, D, E, F, G, H, I, J>);
// struct_decl!(11, <A, B, C, D, E, F, G, H, I, J, K>);
// struct_decl!(12, <A, B, C, D, E, F, G, H, I, J, K, L>);
// struct_decl!(13, <A, B, C, D, E, F, G, H, I, J, K, L, M>);
// struct_decl!(14, <A, B, C, D, E, F, G, H, I, J, K, L, M, N>);
// struct_decl!(15, <A, B, C, D, E, F, G, H, I, J, K, L, M, N, O>);
// struct_decl!(16, <A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P>);

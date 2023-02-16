#![allow(clippy::too_many_arguments)]

use std::mem::transmute;
use std::ptr::null_mut;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::{Relaxed, Release};

use paste::paste;

use crate::reinit::{Dependency, Reinit, ReinitDetails, ReinitRef, Restart};

/// T: Reinit type
/// A..P: dependent Reinit types
macro_rules! struct_decl {
    (
		$num:literal,
		<$($x:ident),+>
	) => (paste!{
		pub struct [<Reinit $num>]<T: 'static, $($x: 'static,)+>
		{
			$([<$x:lower>]: Dependency<$x>),+,
			constructor: fn($(ReinitRef<$x>,)+ Restart<T>) -> T,
			parent: AtomicPtr<Reinit<T>>,
		}

		impl<T: 'static, $($x: 'static,)+> [<Reinit $num>]<T, $($x,)+>
		{
			pub const fn new($([<$x:lower>]: &'static Reinit<$x>,)+ constructor: fn($(ReinitRef<$x>,)+ Restart<T>) -> T) -> Self
			{
				Self {
					$([<$x:lower>]: Dependency::new([<$x:lower>]),)+
					constructor,
					parent: AtomicPtr::new(null_mut()),
				}
			}

			pub const fn create_reinit(&'static self) -> Reinit<T> {
				Reinit::new($num, self)
			}

			fn parent(&'static self) -> &'static Reinit<T> {
				// Relaxed is fine as any call to Reinit does it's own sync
				let ptr = self.parent.load(Relaxed);
				assert_ne!(ptr, null_mut(), "parent is null, was init() never called by Reinit?");
				// SAFETY: stored pointer is either null and fails above, or a valid &'static so can never dangle
				unsafe { &*ptr }
			}

			$(
			fn [<accept_ $x:lower>](&'static self, t: ReinitRef<$x>) {
				self.[<$x:lower>].value_set(t);
				self.parent().construct_dec();
			}
			)+

			$(
			fn [<request_drop_ $x:lower>](&'static self) {
				self.[<$x:lower>].value_clear();
				self.parent().construct_inc();
			}
			)+
		}

		impl<T: 'static, $($x: 'static,)+> ReinitDetails<T> for [<Reinit $num>]<T, $($x,)+>
		{
			fn init(&'static self, parent: &'static Reinit<T>) {
				unsafe {
					self.parent.compare_exchange(null_mut(), transmute(&parent), Release, Relaxed)
						.expect("Multiple Reinits initialized this ReinitDetails! There should only be a 1:1 relationship between them, which the macros enforce.");
				}
				$(self.[<$x:lower>].reinit.add_callback(self, Self::[<accept_ $x:lower>], Self::[<request_drop_ $x:lower>]);)+
			}

			unsafe fn on_need_inc(&'static self, _: &'static Reinit<T>) {
				$(self.[<$x:lower>].reinit.need_inc();)+
			}

			unsafe fn on_need_dec(&'static self, _: &'static Reinit<T>) {
				$(self.[<$x:lower>].reinit.need_dec();)+
			}

			fn request_construction(&'static self, parent: &'static Reinit<T>) {
				parent.constructed((self.constructor)($(self.[<$x:lower>].value_get().clone(),)+ Restart::new(&self.parent())));
			}
		}
    })
}

struct_decl!(1, <A>);
struct_decl!(2, <A, B>);
struct_decl!(3, <A, B, C>);
struct_decl!(4, <A, B, C, D>);
struct_decl!(5, <A, B, C, D, E>);
struct_decl!(6, <A, B, C, D, E, F>);
struct_decl!(7, <A, B, C, D, E, F, G>);
struct_decl!(8, <A, B, C, D, E, F, G, H>);
struct_decl!(9, <A, B, C, D, E, F, G, H, I>);
struct_decl!(10, <A, B, C, D, E, F, G, H, I, J>);
struct_decl!(11, <A, B, C, D, E, F, G, H, I, J, K>);
struct_decl!(12, <A, B, C, D, E, F, G, H, I, J, K, L>);
struct_decl!(13, <A, B, C, D, E, F, G, H, I, J, K, L, M>);
struct_decl!(14, <A, B, C, D, E, F, G, H, I, J, K, L, M, N>);
struct_decl!(15, <A, B, C, D, E, F, G, H, I, J, K, L, M, N, O>);
struct_decl!(16, <A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P>);

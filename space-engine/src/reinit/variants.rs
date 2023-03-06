#![allow(clippy::too_many_arguments)]

use std::ptr::null_mut;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::{Relaxed, Release};

use paste::paste;

use crate::reinit::{Dependency, Reinit, ReinitDetails, ReinitRef, Restart};

/// T: Reinit type
/// A..P: dependent Reinit types
macro_rules! reinit_variant_struct {
    (
		$num:literal,
		<$($x:ident),+>
	) => (paste!{
		pub struct [<Reinit $num>]<T: 'static, $($x: 'static,)+>
		{
			$([<$x:lower>]: Dependency<$x>),+,
			constructor: fn($(&ReinitRef<$x>,)+ Restart<T>) -> T,
			parent: AtomicPtr<Reinit<T>>,
		}

		unsafe impl<T: 'static, $($x: 'static,)+> Sync for [<Reinit $num>]<T, $($x,)+> {}

		impl<T: 'static, $($x: 'static,)+> [<Reinit $num>]<T, $($x,)+>
		{
			pub const fn new($([<$x:lower>]: &'static Reinit<$x>,)+ constructor: fn($(&ReinitRef<$x>,)+ Restart<T>) -> T) -> Self
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
			fn [<accept_ $x:lower>](&'static self, t: &ReinitRef<$x>) {
				self.[<$x:lower>].value_set(t.clone());
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
				self.parent.compare_exchange(null_mut(), parent as *const _ as *mut _, Release, Relaxed)
					.expect("Multiple Reinits initialized this ReinitDetails! There should only be a 1:1 relationship between them, which the macros enforce.");
				$(self.[<$x:lower>].reinit.add_callback(self, Self::[<accept_ $x:lower>], Self::[<request_drop_ $x:lower>]);)+
			}

			unsafe fn on_need_inc(&'static self, _: &'static Reinit<T>) {
				$(self.[<$x:lower>].reinit.need_inc();)+
			}

			unsafe fn on_need_dec(&'static self, _: &'static Reinit<T>) {
				$(self.[<$x:lower>].reinit.need_dec();)+
			}

			fn request_construction(&'static self, parent: &'static Reinit<T>) {
				parent.constructed((self.constructor)($(self.[<$x:lower>].value_ref(),)+ Restart::new(&self.parent())));
			}
		}
    })
}

macro_rules! reinit_variant_process {
    (
		$((
			$num:literal,
			<$($x:ident),+>
		),)+
	) => {
		$(reinit_variant_struct!($num, <$($x),+>);)+

		// you cannot declare macro_rules! within macro_rules! as you cannot escape the $ character
		// see https://github.com/rust-lang/rust/issues/83527
		// so to generate reinit! atm just expand the macro below and replace a few things:
		// remove the "macro_rules! reinit_generator {" and the last "}"
		// ' ' -> ''
		// _ -> $
		// paste -> paste!
		// \((.*?)\)=>\{(.*?;)\n\s*}; -> ($1) => {\n\t\t$2\n\t};\n\t

		// paste!(macro_rules! reinit_generator {
		// 	$(
		// 		(_name:ident: _t:ty = ($([<_r $x:lower>]:ident: [<_f $x:lower>]:ty),+) => _f:expr) => {
		// 			reinit(_name: _t = ($([<_r $x:lower>]: [<_f $x:lower>]),+) => _f; $num);
		// 		};
		// 	)+
		// });
	};
}

reinit_variant_process!(
	(1, <A>),
	(2, <A, B>),
	(3, <A, B, C>),
	(4, <A, B, C, D>),
	(5, <A, B, C, D, E>),
	(6, <A, B, C, D, E, F>),
	(7, <A, B, C, D, E, F, G>),
	(8, <A, B, C, D, E, F, G, H>),
	(9, <A, B, C, D, E, F, G, H, I>),
	(10, <A, B, C, D, E, F, G, H, I, J>),
	(11, <A, B, C, D, E, F, G, H, I, J, K>),
	(12, <A, B, C, D, E, F, G, H, I, J, K, L>),
	(13, <A, B, C, D, E, F, G, H, I, J, K, L, M>),
	(14, <A, B, C, D, E, F, G, H, I, J, K, L, M, N>),
	(15, <A, B, C, D, E, F, G, H, I, J, K, L, M, N, O>),
	(16, <A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P>),
);

#[macro_export]
macro_rules! reinit {
	// generic case generating code
	($name:ident: $t:ty = ($($from:ident: $from_type:ty),*) => $f:expr; $num:literal) => ($crate::paste::paste!{
		static [<$name _DETAILS>]: $crate::reinit::[<Reinit $num>]<$t, $($from_type),*> = $crate::reinit::[<Reinit $num>]::new($(&$from,)* $f);
		static $name: $crate::reinit::Reinit<$t> = [<$name _DETAILS>].create_reinit();
	});

	// all lengths redirecting to generic case with length appended at the end
	($name:ident:$t:ty=($ra:ident:$fa:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa)=>$f;1);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb)=>$f;2);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc)=>$f;3);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd)=>$f;4);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe)=>$f;5);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff)=>$f;6);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg)=>$f;7);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh)=>$f;8);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi)=>$f;9);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj)=>$f;10);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk)=>$f;11);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl)=>$f;12);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm)=>$f;13);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty,$rn:ident:$fn:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm,$rn:$fn)=>$f;14);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty,$rn:ident:$fn:ty,$ro:ident:$fo:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm,$rn:$fn,$ro:$fo)=>$f;15);
	};
	($name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty,$rn:ident:$fn:ty,$ro:ident:$fo:ty,$rp:ident:$fp:ty)=>$f:expr) => {
		reinit!($name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm,$rn:$fn,$ro:$fo,$rp:$fp)=>$f;16);
	};

	// special case 0, needs to be at the end
	($name:ident: $t:ty = () => $f:expr) => {
		reinit!($name: $t = () => $f; 0);
	};
	($name:ident: $t:ty = $f:expr) => {
		reinit!($name: $t = () => |_| $f; 0);
	};
}

#[macro_export]
macro_rules! reinit_internal {
	// generic case generating code
	($vis:vis $name:ident: $t:ty = ($($from:ident: $from_type:ty),*) => $f:expr; $num:literal) => ($crate::paste::paste!{
		static [<$name _DETAILS>]: $crate::reinit::[<Reinit $num>]<$t, $($from_type),*> = $crate::reinit::[<Reinit $num>]::new($(&$from,)* $f);
		$vis static $name: $crate::reinit::Reinit<$t> = [<$name _DETAILS>].create_reinit();
	});

	// special case 0, manually written
	($vis:vis $name:ident: $t:ty = () => $f:expr) => {
		$crate::reinit_internal!($vis $name: $t = () => $f; 0);
	};

	// all lengths redirecting to generic case with length appended at the end, automatically generated, see variants.rs
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa)=>$f;1);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb)=>$f;2);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc)=>$f;3);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd)=>$f;4);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe)=>$f;5);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff)=>$f;6);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg)=>$f;7);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh)=>$f;8);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi)=>$f;9);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj)=>$f;10);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk)=>$f;11);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl)=>$f;12);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm)=>$f;13);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty,$rn:ident:$fn:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm,$rn:$fn)=>$f;14);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty,$rn:ident:$fn:ty,$ro:ident:$fo:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm,$rn:$fn,$ro:$fo)=>$f;15);
	};
	($vis:vis $name:ident:$t:ty=($ra:ident:$fa:ty,$rb:ident:$fb:ty,$rc:ident:$fc:ty,$rd:ident:$fd:ty,$re:ident:$fe:ty,$rf:ident:$ff:ty,$rg:ident:$fg:ty,$rh:ident:$fh:ty,$ri:ident:$fi:ty,$rj:ident:$fj:ty,$rk:ident:$fk:ty,$rl:ident:$fl:ty,$rm:ident:$fm:ty,$rn:ident:$fn:ty,$ro:ident:$fo:ty,$rp:ident:$fp:ty)=>$f:expr) => {
		$crate::reinit_internal!($vis $name:$t=($ra:$fa,$rb:$fb,$rc:$fc,$rd:$fd,$re:$fe,$rf:$ff,$rg:$fg,$rh:$fh,$ri:$fi,$rj:$fj,$rk:$fk,$rl:$fl,$rm:$fm,$rn:$fn,$ro:$fo,$rp:$fp)=>$f;16);
	};
}

#[macro_export]
macro_rules! clone_shadow {
	(_) => {};
	($t:tt) => {
		$crate::clone_shadow!($t, $t);
	};
	($p:pat_param, $e:expr) => {
		let $p = $e.clone();
	};
}

/// default reinit macro, always delegates initialization to an async task, function consumes ReinitRefs
#[macro_export]
macro_rules! reinit {
	($vis:vis $name:ident: $t:ty = ($($from:ident: $from_type:ty),*) => |$($in:tt),*| $f:expr) => {
		$crate::reinit_internal!($vis $name: $t = ($($from: $from_type),*) => (|$($in,)* con: $crate::reinit::Constructed<$t>| {
			$($crate::clone_shadow!($in);)*
			$crate::spawn(async move { con.constructed($f); }).detach()
		}));
	};

	// shorthand for case 0
	($vis:vis $name:ident: $t:ty = $f:expr) => {
		$crate::reinit!($vis $name: $t = () => |_| $f);
	};
}

/// reinit macro expecting a `Future<Output=T>`, always delegates initialization to an async task, function consumes ReinitRefs
#[macro_export]
macro_rules! reinit_future {
	($vis:vis $name:ident: $t:ty = ($($from:ident: $from_type:ty),*) => |$($in:tt),*| $f:expr) => {
		$crate::reinit!($vis $name: $t = ($($from: $from_type),*) => |$($in),*| $f.await);
	};

	// shorthand for case 0
	($vis:vis $name:ident: $t:ty = $f:expr) => {
		$crate::reinit_future!($vis $name: $t = () => |_| $f);
	};
}

/// reinit macro which does the initialization immediately instead of spawning a task, for small things such as just mapping a value, only gets ReinitRefs by ref
#[macro_export]
macro_rules! reinit_map {
	($vis:vis $name:ident: $t:ty = ($($from:ident: $from_type:ty),*) => |$($in:tt),*| $f:expr) => {
		$crate::reinit_internal!($vis $name: $t = ($($from: $from_type),*) => (|$($in,)* con: $crate::reinit::Constructed<$t>| con.constructed($f)));
	};

	// shorthand for case 0
	($vis:vis $name:ident: $t:ty = $f:expr) => {
		$crate::reinit_map!($vis $name: $t = () => |_| $f);
	};
}

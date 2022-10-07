struct Reinit1<T, F, A>
	where
		F: Fn(&A) -> T
{
	reinit: Inner<T>,
	constructor: F,
	a: Dependency<A>,
}

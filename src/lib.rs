//! Karutin

// TODO: KarutinIter doc
// TODO: KarutinFuture doc

pub use crate::proc_macro::{karutin, karutin_str};

/// State that obtained by resuming the [`Karutin`]
// TODO
// # Example
// ```ignore
//
// ```
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KarutinState<Yield, Return> {
	/// Yielded value from Karutin with `yield` or `~yield`
	Yielded(Yield),
	/// Returned value from Karutin
	Returned(Return),
	/// Karutin execution is completed
	///
	/// In [`karutin!`](karutin) context,
	/// this state means the Karutin already returned a value.
	Completed,
}

impl<Y, R> KarutinState<Y, R> {
	/// Check if Karutin is concluded,
	/// that means is this not a yielded value
	// TODO
	// # Example
	// ```ignore
	//
	// ```
	pub fn is_concluded(&self) -> bool {
		match self {
			Self::Yielded(_) => false,
			Self::Returned(_) => true,
			Self::Completed => true,
		}
	}
}

impl<T> KarutinState<T, T> {
	/// Convert [`KarutinState`] into [`Option`]
	///
	/// This is usefull for unwrap a value from the state directly
	// TODO
	// # Example
	// ```ignore
	//
	// ```
	pub fn into_option(self) -> Option<T> {
		match self {
			Self::Yielded(v) | Self::Returned(v) => Some(v),
			Self::Completed => None,
		}
	}
}

/// Simple representation of a coroutine
///
/// This trait declare general behaviour of a coroutine, like:
/// - What does it [yield](Self::Yield)
/// - What does it [return](Self::Yield)
/// - How is it [resumed](Self::resume)
///
/// Every coroutine defined in [`karutin!`](karutin)
/// returns auto-generated struct that implements this trait.<br>
/// Because of this, coroutines are also called `Karutin` throughout the crate.
///
/// For more detailed information about how does the crate implement this trait,
/// please refer to [karutin!](karutin) macro!
///
// TODO
// # Example
// ```ignore
//
// ```
pub trait Karutin<Args = ()>: Sized {
	/// Type of the value coroutine yields
	type Yield;
	/// Type of the value coroutine returns
	type Return;

	/// Resume the process of [`Karutin`]
	///
	/// This method is a way to communicate with a coroutine.
	///
	/// Returns [`KarutinState`] if the process:
	/// - yields
	/// - returns
	/// - is already completed
	// TODO
	// # Example
	// ```ignore
	//
	// ```
	fn resume(&mut self, args: Args) -> KarutinState<Self::Yield, Self::Return>;
}

#[doc(inline)]
pub use crate::iter::KarutinIter;

#[doc(inline)]
pub use crate::future::KarutinFuture;

/// Argument-less, aka generator supertrait of [`Karutin`]
///
/// It might not be accurate to call this as generator
/// (see [this][what-is-the-difference]),
/// but in the crate, the definition is accepted in this way.
///
/// [what-is-the-difference]: https://blog.rust-lang.org/inside-rust/2023/10/23/coroutines/#what-is-the-difference
///
/// Because of there is no need to any argument to resume,
/// generators can be easily convertable to [iterator][Self::into_iter]
/// or [closure][Self::into_closure].
// TODO
// # Example
// ```ignore
//
// ```
pub trait KarutinGen: Karutin<()> {
	/// Same method as [`Karutin::resume`],
	/// but this one do not take argument
	fn resume(&mut self) -> KarutinState<Self::Yield, Self::Return> {
		Karutin::resume(self, ())
	}

	/// Convert [`KarutinGen`] into [`KarutinIter`]
	///
	/// [CompleteStrategy] is [Once]
	///
	/// [CompleteStrategy]: crate::iter::CompleteStrategy
	/// [Once]: crate::iter::CompleteStrategy::Once
	///
	/// For more information, please refer to [KarutinIter::new]
	fn into_iter(self) -> KarutinIter<Self> {
		KarutinIter::new(self)
	}

	/// Convert [`KarutinGen`] into [`KarutinIter`]
	///
	/// [CompleteStrategy] is [Outed]
	///
	/// [CompleteStrategy]: crate::iter::CompleteStrategy
	/// [Outed]: crate::iter::CompleteStrategy::Outed
	///
	/// For more information, please refer to [KarutinIter::new_outed]
	fn into_iter_outed(self) -> KarutinIter<Self> {
		KarutinIter::new_outed(self)
	}

	/// Convert [`KarutinGen`] into [`KarutinIter`]
	///
	/// [CompleteStrategy] is [Infinite]
	///
	/// [CompleteStrategy]: crate::iter::CompleteStrategy
	/// [Infinite]: crate::iter::CompleteStrategy::Infinite
	///
	/// For more information, please refer to [KarutinIter::new_endless]
	fn into_iter_endless(self) -> KarutinIter<Self> {
		KarutinIter::new_endless(self)
	}

	/// Convert [`KarutinGen`] into [`FnMut`]
	///
	/// Same as [`Karutin::resume`], but the Karutin is moved to returned closure.
	///
	/// This method works like [`Iterator::next`],
	/// but by not converting Karutin into [`KarutinIter`]
	// TODO
	// # Example
	// ```ignore
	//
	// ```
	fn into_closure(mut self) -> impl FnMut() -> KarutinState<Self::Yield, Self::Return> {
		move || KarutinGen::resume(&mut self)
	}
}

/// Palindrome supertrait of [`Karutin`]
///
/// By the mean of "Palindrome",
/// this actually means the coroutine yields and returns same type of value.
/// So with this ability, [`KarutinState`] can be downcasted to inner value easily.
// TODO
// # Example
// ```ignore
//
// ```
pub trait KarutinPal<T>: Karutin<Yield = T, Return = T> {}

/// Supertrait of [`KarutinGen`] + [`KarutinPal`]
///
/// With the powers of [`KarutinGen`] and [`KarutinPal`],
/// we can freely resume the coroutine
/// and easily downcast [`KarutinState`] into inner value.
///
/// So now, we can work on the inner values as a whole,
/// like [iterating][Self::into_value_iter]
/// and [collecting][Self::into_values] them.
// TODO
// # Example
// ```ignore
//
// ```
pub trait KarutinPalGen<T>: KarutinPal<T> + KarutinGen {
	/// Convert [`KarutinGen`] into [`Iterator<Item = T>`]
	///
	/// For more information, please refer to [KarutinIter::into_value_iter]
	fn into_value_iter(self) -> impl Iterator<Item = T> {
		self.into_iter().into_value_iter()
	}

	/// Convert [`KarutinGen`] into [`Vec<T>`]
	///
	/// For more information, please refer to [KarutinIter::into_values]
	fn into_values(self) -> Vec<T> {
		self.into_iter().into_values()
	}
}

/// Supertrait of [`KarutinGen<Yield = ()>`]
///
/// The "Uni" part in "KarutinUniGen" stands for [unit type][unit_type]
/// (it yields units),
/// but it also stands for the word "Universal".
///
/// [unit_type]: https://doc.rust-lang.org/std/primitive.unit.html
///
/// Because Karutins that implements [`KarutinUniGen`] can be worked with as a future ([KarutinFuture])
/// or closure ([FnOnce]), like universal conversiontial Karutin.
// TODO
// # Example
// ```ignore
//
// ```
pub trait KarutinUniGen: KarutinGen<Yield = ()> {
	/// Convert [`KarutinUniGen`] into [`KarutinFuture`]
	///
	/// For more information, please refer to [KarutinFuture]
	fn into_future(self) -> KarutinFuture<Self> {
		self.into()
	}

	/// Convert [`KarutinUniGen`] into [`FnOnce`]
	///
	/// This is what the returned closure does:
	///
	/// - Basicly looks for first [concluded](KarutinState::is_concluded) state,
	/// and if it is a [KarutinState::Returned], returns its inner value by wrapping it with [Some].
	///
	/// - If there is no [concluded](KarutinState::is_concluded) state,
	/// or it is a [KarutinState::Completed], returns [None].
	///
	/// This method is usefull for working with a Karutin as a normal function.
	// TODO
	// # Example
	// ```ignore
	//
	// ```
	fn into_closure_once(self) -> impl FnOnce() -> Option<Self::Return> {
		|| {
			let mut iter = self.into_iter_outed();
			let concluded = iter.find(KarutinState::is_concluded);

			match concluded {
				Some(KarutinState::Returned(ret)) => Some(ret),
				_ => None,
			}
		}
	}
}

// Implement all supertraits of Karutin that provide its own bounds
impl<T: Karutin<()>> KarutinGen for T {}
impl<U, T: Karutin<Yield = U, Return = U>> KarutinPal<U> for T {}
impl<U, T: KarutinPal<U> + KarutinGen> KarutinPalGen<U> for T {}
impl<T: KarutinGen<Yield = ()>> KarutinUniGen for T {}

/// Re-exports from [karutin_proc_macro]
pub mod proc_macro {
	#[doc(inline)]
	pub use karutin_proc_macro::karutin;

	#[doc(inline)]
	pub use karutin_proc_macro::karutin_str;
}

/// Iterator for `Karutin`s
pub mod iter {
	use crate::{Karutin, KarutinGen, KarutinState};

	pub enum CompleteStrategy {
		Once,
		Outed,
		Infinite,
	}

	impl Default for CompleteStrategy {
		fn default() -> Self {
			CompleteStrategy::Once
		}
	}

	/// Iterator-capable wrapper for [`KarutinGen`]
	pub struct KarutinIter<T: KarutinGen> {
		complete_strategy: CompleteStrategy,
		is_ended: bool,
		inner: T,
	}

	impl<T: KarutinGen> KarutinIter<T> {
		fn _new(i: T, s: CompleteStrategy) -> KarutinIter<T> {
			KarutinIter {
				complete_strategy: s,
				is_ended: false,
				inner: i,
			}
		}

		pub fn new(karutin: T) -> KarutinIter<T> {
			Self::_new(karutin, Default::default())
		}

		pub fn new_outed(karutin: T) -> KarutinIter<T> {
			Self::_new(karutin, CompleteStrategy::Outed)
		}

		pub fn new_endless(karutin: T) -> KarutinIter<T> {
			Self::_new(karutin, CompleteStrategy::Infinite)
		}

		fn is_next_last(&self, next: &KarutinState<T::Yield, T::Return>) -> bool {
			match (&next, &self.complete_strategy) {
				| (KarutinState::Completed, CompleteStrategy::Once)
				| (KarutinState::Completed, CompleteStrategy::Outed)
				| (KarutinState::Returned(_), CompleteStrategy::Outed) => true,
				_whoever_reads_this_is_gay_ => false,
			}
		}
	}

	impl<T, U> KarutinIter<T>
	where
		T: Karutin<(), Yield = U, Return = U>,
	{
		pub fn into_value_iter(mut self) -> impl Iterator<Item = U> {
			self.complete_strategy = CompleteStrategy::Outed;
			self.filter_map(KarutinState::into_option)
		}

		pub fn into_values(self) -> Vec<U> {
			Self::into_value_iter(self).collect()
		}
	}

	impl<T: KarutinGen> Iterator for KarutinIter<T> {
		type Item = KarutinState<T::Yield, T::Return>;

		fn next(&mut self) -> Option<Self::Item> {
			if self.is_ended {
				return None;
			}

			let next = KarutinGen::resume(&mut self.inner);

			if self.is_next_last(&next) {
				self.is_ended = true;
			}

			Some(next)
		}
	}

	impl<T: KarutinGen> From<T> for KarutinIter<T> {
		fn from(karutin: T) -> Self {
			Self::new(karutin)
		}
	}

	struct KarutinBridgeIter<T, U>
	where
		T: Iterator<Item = U>,
	{
		inner: T,
	}

	impl<T, U> Karutin<()> for KarutinBridgeIter<T, U>
	where
		T: Iterator<Item = U>,
	{
		type Yield = T::Item;
		type Return = T::Item;

		fn resume(&mut self, _args: ()) -> KarutinState<Self::Yield, Self::Return> {
			if let Some(yielded) = self.inner.next() {
				KarutinState::Yielded(yielded)
			} else {
				KarutinState::Completed
			}
		}
	}

	impl<T, U> Iterator for KarutinBridgeIter<T, U>
	where
		T: Iterator<Item = U>,
	{
		type Item = U;

		fn next(&mut self) -> Option<Self::Item> {
			self.inner.next()
		}
	}

	impl<T, U> From<T> for KarutinBridgeIter<T::IntoIter, U>
	where
		T: IntoIterator<Item = U>,
		T::IntoIter: Iterator<Item = U>,
	{
		fn from(value: T) -> Self {
			Self {
				inner: value.into_iter(),
			}
		}
	}

	impl<T, U> From<T> for KarutinIter<KarutinBridgeIter<T::IntoIter, U>>
	where
		T: IntoIterator<Item = U>,
		T::IntoIter: Iterator<Item = U>,
	{
		fn from(value: T) -> Self {
			Self::new(KarutinBridgeIter::from(value))
		}
	}

	#[doc(hidden)]
	#[rustfmt::skip]
	#[macro_export]
	macro_rules! into_value_iter {
		($expr:expr) => {
			::karutin::iter::KarutinIter::from($expr).into_value_iter()
		};
	}

	/// Convert [`KarutinPalGen`][crate::KarutinPalGen]
	/// or any [`IntoIterator`] into [`Iterator`]
	#[doc(inline)]
	pub use into_value_iter;
}

/// Future for `Karutin`s
pub mod future {
	use crate::{KarutinIter, KarutinState, KarutinUniGen};
	use std::pin::Pin;
	use std::task::{Context, Poll};

	/// Async-capable wrapper for [`KarutinUniGen`]
	pub struct KarutinFuture<T: KarutinUniGen> {
		iter: KarutinIter<T>,
	}

	impl<T: KarutinUniGen> KarutinFuture<T> {
		pub fn new(karutin: T) -> KarutinFuture<T> {
			Self {
				iter: karutin.into_iter_outed(),
			}
		}
	}

	impl<T: KarutinUniGen> From<T> for KarutinFuture<T> {
		fn from(karutin: T) -> Self {
			Self::new(karutin)
		}
	}

	impl<T: KarutinUniGen + Unpin> Future for KarutinFuture<T> {
		type Output = Option<T::Return>;

		fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			let Some(next) = self.get_mut().iter.next() else {
				return Poll::Pending;
			};

			cx.waker().wake_by_ref();

			match next {
				KarutinState::Yielded(_) => Poll::Pending,
				KarutinState::Returned(r) => Poll::Ready(r.into()),
				KarutinState::Completed => Poll::Ready(None),
			}
		}
	}
}

/// Necessary things for auto-generated [`Karutin`]
/// implementations with [`karutin!`](karutin) to work
#[doc(hidden)]
pub mod internal {
	use crate::KarutinState;

	#[inline(always)]
	pub fn unchecked_zeroed<T>() -> T {
		unsafe {
			#[allow(invalid_value)]
			std::mem::MaybeUninit::zeroed().assume_init()
		}
	}

	pub type RawStackPair<'a> = (&'a [u8], &'a [u8]);
	pub type LeakedStackPair<'a, T> = (&'a mut T, &'a mut T);
	pub type BoxedStackPair<T> = (Box<T>, Box<T>);

	#[derive(Debug)]
	pub struct KarutinStack<'a> {
		inner: &'a [u8],
		rep: &'a [u8],
	}

	impl<'a> KarutinStack<'a> {
		pub fn create_zeroeds<T>() -> BoxedStackPair<T> {
			(Box::new(unchecked_zeroed()), Box::new(unchecked_zeroed()))
		}

		pub fn get_boxes<T>(&self) -> BoxedStackPair<T> {
			unsafe {
				(
					Box::from_raw(self.inner.as_ptr() as *mut T),
					Box::from_raw(self.rep.as_ptr() as *mut T),
				)
			}
		}

		fn leak<'b, T>(boxeds: BoxedStackPair<T>) -> LeakedStackPair<'b, T> {
			(Box::leak(boxeds.0), Box::leak(boxeds.1))
		}

		fn get_raws<'b, T>(refs: LeakedStackPair<'b, T>) -> RawStackPair<'b> {
			let get_raw = |ref_| unsafe {
				let pointer = ref_ as *mut T as *mut u8;
				std::slice::from_raw_parts_mut(pointer, std::mem::size_of::<T>())
			};

			(get_raw(refs.0), get_raw(refs.1))
		}

		fn from_raws(value: RawStackPair<'a>) -> Self {
			Self {
				inner: value.0,
				rep: value.1,
			}
		}
	}

	impl<'a, T: 'a> From<BoxedStackPair<T>> for KarutinStack<'a> {
		fn from(value: BoxedStackPair<T>) -> Self {
			let leakeds = KarutinStack::leak(value);
			let raws = KarutinStack::get_raws(leakeds);

			KarutinStack::from_raws(raws)
		}
	}

	pub enum KarutinResponse<'a, Y, R> {
		StackExpose(KarutinStack<'a>),
		StateLoop(KarutinState<Y, R>),
	}
}

/// Re-export collection for a sufficient experience
pub mod prelude {
	// Necessary more than necessariest :)
	pub use crate::KarutinState;
	
	// I think there is no human being
	// who will implement these traits manually
	pub use crate::proc_macro::karutin;

	// There is no problem to export all traits,
	// they each may needed at any point
	pub use crate::{Karutin, KarutinGen, KarutinPal, KarutinPalGen, KarutinUniGen};
}

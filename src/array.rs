//! A contiguous growable array for use with the blend2d api.
use std::borrow::Borrow;
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::{fmt, io, ops, ptr, slice};

use crate::codec::ImageCodec;
use crate::error::{errcode_to_result, expect_mem_err, OutOfMemory, Result};
use crate::util::range_to_tuple;
use crate::variant::WrappedBlCore;

/// A contiguous growable array for use with the blend2d api.
/// This is an array managed by blend2d, unless required one should use [`Vec`]
/// instead. Its api tries to mimic [`Vec`] as close as possible.
///
/// [`String`]: std/vec/struct.Vec.html
#[repr(transparent)]
pub struct Array<T: ArrayType> {
    core: ffi::BLArrayCore,
    _pd: PhantomData<T>,
}

unsafe impl<T: ArrayType> WrappedBlCore for Array<T> {
    type Core = ffi::BLArrayCore;
    const IMPL_TYPE_INDEX: usize = T::IMPL_IDX;

    #[inline]
    fn from_core(core: Self::Core) -> Self {
        Array {
            core,
            _pd: PhantomData,
        }
    }
}

impl<T: ArrayType> Array<T> {
    /// Creates a new empty array.
    pub fn new() -> Self {
        Self::from_core(*Self::none())
    }

    /// Creates a new empty array with space for `cap` elements.
    pub fn with_capacity(cap: usize) -> Self {
        let mut this = Array::from_core(*Self::none());
        this.reserve(cap);
        this
    }

    /// Clears the array and its contents.
    #[inline]
    pub fn clear(&mut self) {
        unsafe { ffi::blArrayClear(self.core_mut()) };
    }

    /// Shrinks the arrays allocated capacity down to its currently used size.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        unsafe { expect_mem_err(ffi::blArrayShrink(self.core_mut())) };
    }

    /// Reserves capacity for at least n items.
    ///
    /// # Panics
    ///
    /// Panics if blend2d returns an
    /// [`OutOfMemory`](../error/enum.Error.html#variant.OutOfMemory) error
    #[inline]
    pub fn reserve(&mut self, n: usize) {
        self.try_reserve(n).expect("memory allocation failed");
    }

    /// Reserves capacity for at least n items.
    #[inline]
    pub fn try_reserve(&mut self, n: usize) -> std::result::Result<(), OutOfMemory> {
        unsafe { OutOfMemory::from_errcode(ffi::blArrayReserve(self.core_mut(), n)) }
    }

    /// Truncates the array down to n elements.
    #[inline]
    pub fn truncate(&mut self, n: usize) {
        unsafe {
            expect_mem_err(ffi::blArrayResize(
                self.core_mut(),
                n.min(self.len()),
                ptr::null(),
            ))
        };
    }

    /// Resizes the array so that its len is equal to `n`, filling any new items
    /// with `fill`.
    pub fn resize(&mut self, n: usize, fill: T)
    where
        T: Clone,
    {
        unsafe {
            let diff = n.checked_sub(self.len()).unwrap_or_default();
            let buff = vec![fill; diff];
            expect_mem_err(ffi::blArrayResize(
                self.core_mut(),
                n,
                buff.as_ptr() as *const _,
            ))
        };
    }

    /// Removes the element at the given index.
    #[inline]
    pub fn remove(&mut self, index: usize) -> Result<()> {
        unsafe { errcode_to_result(ffi::blArrayRemoveIndex(self.core_mut(), index)) }
    }

    /// Removes the elements whose indices reside inside of the range
    pub fn remove_range<R: ops::RangeBounds<usize>>(&mut self, range: R) -> Result<()> {
        let (start, end) = range_to_tuple(range, || self.len());
        unsafe { errcode_to_result(ffi::blArrayRemoveRange(self.core_mut(), start, end)) }
    }

    /// Returns the array as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self
    }

    /// Returns the length of the array.
    #[inline]
    pub fn len(&self) -> usize {
        unsafe { ffi::blArrayGetSize(self.core()) }
    }

    /// Returns true if this array has no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the current capacity of the array.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.impl_().capacity as usize
    }

    #[inline]
    fn data_ptr(&self) -> *const T {
        unsafe { ffi::blArrayGetData(self.core()) as *const _ }
    }
}

impl<T> Array<T>
where
    T: ArrayType + Clone,
{
    /// Appends all items in the slice to the array.
    pub fn extend_from_slice<S: AsRef<[T]>>(&mut self, data: S) {
        unsafe {
            let data = data.as_ref();
            expect_mem_err(ffi::blArrayAppendView(
                self.core_mut(),
                data.as_ptr() as *const _,
                data.len(),
            ))
        };
    }

    /// Inserts all items in the slice into the array at the given index.
    pub fn insert_from_slice<S: AsRef<[T]>>(&mut self, index: usize, data: S) {
        unsafe {
            let data = data.as_ref();
            expect_mem_err(ffi::blArrayInsertView(
                self.core_mut(),
                index,
                data.as_ptr() as *const _,
                data.len(),
            ))
        };
    }

    /// Replaces the elements specified by the range of indices with the given
    /// slice.
    pub fn replace_from_slice<R, S>(&mut self, range: R, data: S)
    where
        R: ops::RangeBounds<usize>,
        S: AsRef<[T]>,
    {
        let (start, end) = range_to_tuple(range, || self.len());
        let data = data.as_ref();
        unsafe {
            expect_mem_err(ffi::blArrayReplaceView(
                self.core_mut(),
                start,
                end,
                data.as_ptr() as *const _,
                data.len(),
            ))
        };
    }
}

impl<T: ArrayType> Extend<T> for Array<T> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        for item in iter {
            self.push(item);
        }
    }
}

impl<T: ArrayType> FromIterator<T> for Array<T> {
    fn from_iter<I>(iter: I) -> Array<T>
    where
        I: IntoIterator<Item = T>,
    {
        let iter = iter.into_iter();
        let len = iter.size_hint().1.unwrap_or(iter.size_hint().0);
        let mut this = Self::with_capacity(len);
        this.extend(iter);
        this
    }
}

impl<T: ArrayType + Clone> From<Vec<T>> for Array<T> {
    fn from(v: Vec<T>) -> Self {
        let mut this = Self::with_capacity(v.len());
        this.extend_from_slice(&v);
        this
    }
}

impl<'a, T> From<&'a [T]> for Array<T>
where
    T: ArrayType + Clone,
{
    fn from(v: &[T]) -> Self {
        let mut this = Self::with_capacity(v.len());
        this.extend_from_slice(v);
        this
    }
}

impl io::Write for Array<u8> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<T: ArrayType> AsRef<[T]> for Array<T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self
    }
}

impl<T: ArrayType> AsMut<[T]> for Array<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        self
    }
}

impl<T: ArrayType> Borrow<[T]> for Array<T> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self
    }
}

impl<T: ArrayType> ops::Deref for Array<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.data_ptr(), self.len()) }
    }
}

impl<T: ArrayType> ops::DerefMut for Array<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let mut data_ptr = ptr::null_mut();
            expect_mem_err(ffi::blArrayMakeMutable(self.core_mut(), &mut data_ptr));
            slice::from_raw_parts_mut(data_ptr as _, self.len())
        }
    }
}

impl<T, I> ops::Index<I> for Array<T>
where
    T: ArrayType,
    I: slice::SliceIndex<[T]>,
{
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        ops::Index::index(&**self, index)
    }
}

impl<'a, T: ArrayType> IntoIterator for &'a Array<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: ArrayType> Default for Array<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ArrayType> PartialEq for Array<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        unsafe { ffi::blArrayEquals(self.core(), other.core()) }
    }
}

impl<T: ArrayType> Clone for Array<T> {
    fn clone(&self) -> Self {
        Self::from_core(self.init_weak())
    }
}

impl<T> fmt::Debug for Array<T>
where
    T: ArrayType + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ref())
    }
}

impl<T: ArrayType> Drop for Array<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { ffi::blArrayReset(&mut self.core) };
    }
}

impl<T> Array<T>
where
    T: ArrayType,
{
    #[inline]
    pub fn push(&mut self, item: T) {
        unsafe { T::push(self.core_mut(), item) };
    }
    #[inline]
    pub fn insert(&mut self, index: usize, item: T) {
        unsafe { T::insert(self.core_mut(), index, item) };
    }
    #[inline]
    pub fn replace(&mut self, index: usize, item: T) {
        unsafe { T::replace(self.core_mut(), index, item) };
    }
}

impl Array<ImageCodec> {
    /// Searches for an image codec in the array by the given name.
    pub fn find_codec_by_name(&self, name: &str) -> Option<&ImageCodec> {
        self.iter().find(|c| c.name() == name)
    }

    /// Searches for an image codec in the array by the given data.
    pub fn find_codec_by_data<R: AsRef<[u8]>>(&self, data: R) -> Option<&ImageCodec> {
        self.into_iter()
            .max_by_key(|codec| codec.inspect_data(data.as_ref()))
    }
}

use crate::variant::ImplType;

/// A marker trait for types that can be used for a blend2d [`Array`].
/// This trait is also implemented for all blend2d core types.
///
/// [`Array`]: ./struct.Array.html
pub trait ArrayType: Sized {
    #[doc(hidden)]
    const IMPL_IDX: usize;
    #[doc(hidden)]
    #[inline]
    unsafe fn push(core: &mut ffi::BLArrayCore, item: Self) {
        expect_mem_err(ffi::blArrayAppendItem(core, &item as *const _ as *const _));
    }
    #[doc(hidden)]
    #[inline]
    unsafe fn insert(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        expect_mem_err(ffi::blArrayInsertItem(
            core,
            index,
            &item as *const _ as *const _,
        ));
    }
    #[doc(hidden)]
    #[inline]
    unsafe fn replace(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        expect_mem_err(ffi::blArrayReplaceItem(
            core,
            index,
            &item as *const _ as *const _,
        ));
    }
}

#[doc(hidden)]
impl<T> ArrayType for T
where
    T: WrappedBlCore,
{
    const IMPL_IDX: usize = ImplType::ArrayVar as usize;
    #[inline]
    unsafe fn push(core: &mut ffi::BLArrayCore, item: Self) {
        expect_mem_err(ffi::blArrayAppendItem(
            core,
            item.core() as *const _ as *const _,
        ));
    }
    #[inline]
    unsafe fn insert(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        expect_mem_err(ffi::blArrayInsertItem(
            core,
            index,
            item.core() as *const _ as *const _,
        ));
    }
    #[inline]
    unsafe fn replace(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        expect_mem_err(ffi::blArrayReplaceItem(
            core,
            index,
            item.core() as *const _ as *const _,
        ));
    }
}

impl<T> ArrayType for *const T {
    const IMPL_IDX: usize = usize::IMPL_IDX;
    #[inline]
    unsafe fn push(core: &mut ffi::BLArrayCore, item: Self) {
        usize::push(core, item as usize);
    }
    #[inline]
    unsafe fn insert(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        usize::insert(core, index, item as usize);
    }
    #[inline]
    unsafe fn replace(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        usize::insert(core, index, item as usize);
    }
}

impl<T> ArrayType for *mut T {
    const IMPL_IDX: usize = usize::IMPL_IDX;
    #[inline]
    unsafe fn push(core: &mut ffi::BLArrayCore, item: Self) {
        usize::push(core, item as usize);
    }
    #[inline]
    unsafe fn insert(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        usize::insert(core, index, item as usize);
    }
    #[inline]
    unsafe fn replace(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
        usize::insert(core, index, item as usize);
    }
}

// Macro-zone ahead, you have been warned

macro_rules! impl_array_type {
    ($( $append:ident, $insert:ident, $replace:ident for $( ($ty:ty = $idx:expr) ),+);*$(;)*) => {
        $(
            $(
                impl ArrayType for $ty {
                    const IMPL_IDX: usize = $idx as usize;
                    #[inline]
                    unsafe fn push(core: &mut ffi::BLArrayCore, item: Self) {
                        expect_mem_err(ffi::$append(core, item as _))
                    }
                    #[inline]
                    unsafe fn insert(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
                        expect_mem_err(ffi::$insert(core, index, item as _))
                    }
                    #[inline]
                    unsafe fn replace(core: &mut ffi::BLArrayCore, index: usize, item: Self) {
                        expect_mem_err(ffi::$replace(core, index, item as _))
                    }
                }
            )+
        )*
    };
    ($( ( $( $ty:ty ),+ = $idx:expr) );* $(;)*) => {
        $(
            $(
                impl ArrayType for $ty {
                    const IMPL_IDX: usize = $idx as usize;
                }
            )+
        )*
    }
}

impl_array_type! {
    blArrayAppendU8,  blArrayInsertU8,  blArrayInsertU8  for (i8  = ImplType::ArrayI8),  (u8  = ImplType::ArrayU8);
    blArrayAppendU16, blArrayInsertU16, blArrayInsertU16 for (i16 = ImplType::ArrayI16), (u16 = ImplType::ArrayU16);
    blArrayAppendU32, blArrayInsertU32, blArrayInsertU32 for (i32 = ImplType::ArrayI32), (u32 = ImplType::ArrayU32);
    blArrayAppendU64, blArrayInsertU64, blArrayInsertU64 for (i64 = ImplType::ArrayI64), (u64 = ImplType::ArrayU64);
    blArrayAppendF32, blArrayInsertF32, blArrayInsertF32 for (f32 = ImplType::ArrayF32);
    blArrayAppendF64, blArrayInsertF64, blArrayInsertF64 for (f64 = ImplType::ArrayF32);
}

#[cfg(target_pointer_width = "32")]
impl_array_type!(blArrayAppendU32, blArrayInsertU32, blArrayInsertU32 for (isize = ImplType::ArrayI32), (usize = ImplType::ArrayU32));
#[cfg(target_pointer_width = "64")]
impl_array_type!(blArrayAppendU64, blArrayInsertU64, blArrayInsertU64 for (isize = ImplType::ArrayI64), (usize = ImplType::ArrayU64));

mod scope {
    use crate::{array::ArrayType, font_defs::*, geometry::*, variant::ImplType, Tag};
    impl_array_type! {
        (Tag = ImplType::ArrayStruct4);
        (PointD, PointI, SizeD, SizeI, FontFeature, FontVariation = ImplType::ArrayStruct8);
        (Circle = ImplType::ArrayStruct12);
        (BoxD, BoxI, Ellipse, Line, RectD, RectI = ImplType::ArrayStruct16);
        (Arc, Chord, Pie, RoundRect, Triangle = ImplType::ArrayStruct24);
    }
}

#[cfg(test)]
mod test_array {
    use crate::{array::Array, image::Image, path::Path};

    #[test]
    fn test_array_resize() {
        let mut arr = Array::<i32>::new();
        arr.resize(10, 32);
        assert_eq!(&[32; 10][..], &*arr);

        let mut path = Path::new();
        path.move_to(1.0, 2.0);
        let mut arr = Array::<Path>::new();
        arr.resize(10, path.clone());
        assert_eq!(&vec![path; 10][..], &*arr);
    }

    #[test]
    fn test_array_ops_prim() {
        let mut arr = Array::<i32>::new();
        arr.push(32);
        arr.push(24);
        arr.push(16);
        arr.push(8);
        arr.remove(2).unwrap();
        arr.insert(1, 0);
        assert_eq!(&[32, 0, 24, 8], &*arr);
    }

    #[test]
    fn test_array_ops_objects() {
        let img = [
            Image::new(1, 1, Default::default()).unwrap(),
            Image::new(2, 2, Default::default()).unwrap(),
            Image::new(3, 3, Default::default()).unwrap(),
            Image::new(4, 4, Default::default()).unwrap(),
            Image::new(5, 5, Default::default()).unwrap(),
        ];
        let mut arr = Array::<Image>::new();
        for img in img.iter().take(4) {
            arr.push(img.clone());
        }
        arr.remove(2).unwrap();
        arr.insert(1, img[4].clone());
        assert_eq!(
            &[
                img[0].clone(),
                img[4].clone(),
                img[1].clone(),
                img[3].clone()
            ][..],
            &*arr
        );
    }

    #[test]
    fn test_array_deref_mut() {
        let data = [0, 1, 2, 3, 4, 5];
        let mut arr = Array::<i32>::from(&data[..]);
        assert_eq!(&data, &*arr);
        for i in 0..data.len() / 2 {
            arr.swap(i, data.len() - 1 - i);
        }
        assert_eq!(&[5, 4, 3, 2, 1, 0], &*arr);
    }
}

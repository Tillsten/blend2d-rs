//! Functionality for decoding and encoding images.
use std::ffi::CStr;
use std::{fmt, mem, ptr, str};

use ffi::BLImageCodecFeatures::*;

use crate::array::Array;
use crate::error::{errcode_to_result, expect_mem_err, Result};
use crate::image::{Image, ImageInfo};
use crate::util::cast_ref;
use crate::variant::WrappedBlCore;

bl_enum! {
    /// Image codec feature bits.
    pub enum ImageCodecFeatures {
        /// Image codec supports reading images (can create BLImageDecoder).
        Read       = BL_IMAGE_CODEC_FEATURE_READ,
        /// Image codec supports writing images (can create BLImageEncoder).
        Write      = BL_IMAGE_CODEC_FEATURE_WRITE,
        /// Image codec supports lossless compression.
        Lossless   = BL_IMAGE_CODEC_FEATURE_LOSSLESS,
        /// Image codec supports loosy compression.
        Lossy      = BL_IMAGE_CODEC_FEATURE_LOSSY,
        /// Image codec supports writing multiple frames (GIF).
        MultiFrame = BL_IMAGE_CODEC_FEATURE_MULTI_FRAME,
        /// Image codec supports IPTC metadata.
        Iptc       = BL_IMAGE_CODEC_FEATURE_IPTC,
        /// Image codec supports EXIF metadata.
        Exif       = BL_IMAGE_CODEC_FEATURE_EXIF,
        /// Image codec supports XMP metadata.
        Xmp        = BL_IMAGE_CODEC_FEATURE_XMP,
    }
    Default => Read
}

/// Provides a unified interface for inspecting image data and creating image
/// decoders & encoders.
#[repr(transparent)]
pub struct ImageCodec {
    core: ffi::BLImageCodecCore,
}

unsafe impl WrappedBlCore for ImageCodec {
    type Core = ffi::BLImageCodecCore;
    const IMPL_TYPE_INDEX: usize = crate::variant::ImplType::ImageCodec as usize;

    #[inline]
    fn from_core(core: Self::Core) -> Self {
        ImageCodec { core }
    }
}

impl ImageCodec {
    /// Creates an [`ImageDecoder`] for this codec.
    pub fn create_decoder(&self) -> Option<ImageDecoder> {
        unsafe {
            let mut decoder = ImageDecoder::from_core(*ImageDecoder::none());
            errcode_to_result(ffi::blImageCodecCreateDecoder(
                self.core(),
                decoder.core_mut(),
            ))
            .map(|_| decoder)
            .ok()
        }
    }

    /// Creates an [`ImageEncoder`] for this codec.
    pub fn create_encoder(&self) -> Option<ImageEncoder> {
        unsafe {
            let mut encoder = ImageEncoder::from_core(*ImageEncoder::none());
            errcode_to_result(ffi::blImageCodecCreateEncoder(
                self.core(),
                encoder.core_mut(),
            ))
            .map(|_| encoder)
            .ok()
        }
    }

    /// Inspects the given data blob and determines how likely it is that the
    /// file belongs to this codec.
    #[inline]
    pub fn inspect_data<R: AsRef<[u8]>>(&self, data: R) -> u32 {
        // FIXME figure out how to interpret the u32 return value
        unsafe {
            ffi::blImageCodecInspectData(
                self.core(),
                data.as_ref().as_ptr() as *const _,
                data.as_ref().len(),
            )
        }
    }

    /// Returns the blend2d builtin codecs.
    #[inline]
    pub fn built_in_codecs() -> Array<ImageCodec> {
        let mut core = ffi::BLArrayCore {
            impl_: ptr::null_mut(),
        };
        unsafe { ffi::blImageCodecArrayInitBuiltInCodecs(&mut core) };
        WrappedBlCore::from_core(core)
    }

    /// Adds a codec to the built in codecs list.
    #[inline]
    pub fn add_to_built_in(codec: &ImageCodec) {
        unsafe { expect_mem_err(ffi::blImageCodecAddToBuiltIn(codec.core())) };
    }

    /// Removes the codec from the built in codecs list.
    #[inline]
    pub fn remove_from_built_in(codec: &ImageCodec) {
        unsafe { ffi::blImageCodecRemoveFromBuiltIn(codec.core()) };
    }

    /// The codec's name.
    #[inline]
    pub fn name(&self) -> &str {
        unsafe { str::from_utf8_unchecked(CStr::from_ptr(self.impl_().name).to_bytes()) }
    }

    /// The codec's vendor.
    #[inline]
    pub fn vendor(&self) -> &str {
        unsafe { str::from_utf8_unchecked(CStr::from_ptr(self.impl_().vendor).to_bytes()) }
    }

    /// The codec's mime-type.
    #[inline]
    pub fn mime_type(&self) -> &str {
        unsafe { str::from_utf8_unchecked(CStr::from_ptr(self.impl_().mimeType).to_bytes()) }
    }

    /// The codec's file extensions.
    pub fn extensions(&self) -> impl Iterator<Item = &str> {
        unsafe {
            CStr::from_ptr(self.impl_().extensions)
                .to_str()
                .unwrap_or_default()
                .split('|')
        }
    }

    /// The codec's features.
    #[inline]
    pub fn features(&self) -> ImageCodecFeatures {
        (self.impl_().features as u32).into()
    }
}

impl fmt::Debug for ImageCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageCodec")
            .field("name", &self.name())
            .field("vendor", &self.vendor())
            .field("mime_type", &self.mime_type())
            .field("features", &self.features())
            .field("extensions", &self.extensions().collect::<Vec<_>>())
            .finish()
    }
}

impl PartialEq for ImageCodec {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.impl_equals(other)
    }
}

impl Clone for ImageCodec {
    fn clone(&self) -> Self {
        Self::from_core(self.init_weak())
    }
}

impl Drop for ImageCodec {
    fn drop(&mut self) {
        unsafe { ffi::blImageCodecReset(&mut self.core) };
    }
}

/// An image encoder belonging to a certain [`ImageCodec`].
#[repr(transparent)]
pub struct ImageEncoder {
    core: ffi::BLImageEncoderCore,
}

unsafe impl WrappedBlCore for ImageEncoder {
    type Core = ffi::BLImageEncoderCore;
    const IMPL_TYPE_INDEX: usize = crate::variant::ImplType::ImageEncoder as usize;

    #[inline]
    fn from_core(core: Self::Core) -> Self {
        ImageEncoder { core }
    }
}

impl ImageEncoder {
    /// The codec this encoder belongs to.
    #[inline]
    pub fn codec(&self) -> &ImageCodec {
        unsafe { cast_ref(&self.impl_().codec) }
    }

    #[inline]
    pub fn restart(&mut self) -> Result<()> {
        unsafe { errcode_to_result(ffi::blImageEncoderRestart(self.core_mut())) }
    }

    /// The last encoding result.
    #[inline]
    pub fn last_result(&self) -> Result<()> {
        errcode_to_result(self.impl_().lastResult)
    }

    /// The current frame index (to be decoded).
    #[inline]
    pub fn frame_index(&self) -> u64 {
        self.impl_().frameIndex
    }

    /// The position in the source buffer.
    #[inline]
    pub fn buffer_index(&self) -> usize {
        self.impl_().bufferIndex
    }

    #[inline]
    pub fn write_frame(&mut self, image: &Image) -> Result<Array<u8>> {
        unsafe {
            let mut arr = Array::<u8>::new();
            errcode_to_result(ffi::blImageEncoderWriteFrame(
                self.core_mut(),
                arr.core_mut(),
                image.core(),
            ))
            .map(|_| arr)
        }
    }
}

impl PartialEq for ImageEncoder {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.impl_equals(other)
    }
}

impl Clone for ImageEncoder {
    fn clone(&self) -> Self {
        Self::from_core(self.init_weak())
    }
}

impl fmt::Debug for ImageEncoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageEncoder")
            .field("codec", &self.codec())
            .finish()
    }
}

impl Drop for ImageEncoder {
    fn drop(&mut self) {
        unsafe { ffi::blImageEncoderReset(&mut self.core) };
    }
}

/// An image decoder belonging to a certain [`ImageCodec`].
#[repr(transparent)]
pub struct ImageDecoder {
    core: ffi::BLImageDecoderCore,
}

unsafe impl WrappedBlCore for ImageDecoder {
    type Core = ffi::BLImageDecoderCore;
    const IMPL_TYPE_INDEX: usize = crate::variant::ImplType::ImageDecoder as usize;

    #[inline]
    fn from_core(core: Self::Core) -> Self {
        ImageDecoder { core }
    }
}

impl ImageDecoder {
    /// The codec this decoder belongs to.
    #[inline]
    pub fn codec(&self) -> &ImageCodec {
        unsafe { cast_ref(&self.impl_().codec) }
    }

    #[inline]
    pub fn restart(&mut self) -> Result<()> {
        unsafe { errcode_to_result(ffi::blImageDecoderRestart(self.core_mut())) }
    }

    /// The last decoding result.
    #[inline]
    pub fn last_result(&self) -> Result<()> {
        errcode_to_result(self.impl_().lastResult)
    }

    /// The current frame index (to be decoded).
    #[inline]
    pub fn frame_index(&self) -> u64 {
        self.impl_().frameIndex
    }

    /// The position in the source buffer.
    #[inline]
    pub fn buffer_index(&self) -> usize {
        self.impl_().bufferIndex
    }

    pub fn read_info<R: AsRef<[u8]>>(&mut self, buf: R) -> Result<ImageInfo> {
        unsafe {
            let mut dst = mem::zeroed();
            errcode_to_result(ffi::blImageDecoderReadInfo(
                self.core_mut(),
                &mut dst as *mut _ as *mut _,
                buf.as_ref().as_ptr() as *const _,
                buf.as_ref().len(),
            ))
            .map(|_| dst)
        }
    }

    pub fn read_frame<R: AsRef<[u8]>>(&mut self, buf: R) -> Result<Image> {
        unsafe {
            let mut dst = Image::from_core(*Image::none());
            errcode_to_result(ffi::blImageDecoderReadFrame(
                self.core_mut(),
                dst.core_mut(),
                buf.as_ref().as_ptr() as *const _,
                buf.as_ref().len(),
            ))
            .map(|_| dst)
        }
    }
}

impl PartialEq for ImageDecoder {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.impl_equals(other)
    }
}

impl Clone for ImageDecoder {
    fn clone(&self) -> Self {
        Self::from_core(self.init_weak())
    }
}

impl fmt::Debug for ImageDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageDecoder")
            .field("codec", &self.codec())
            .finish()
    }
}

impl Drop for ImageDecoder {
    fn drop(&mut self) {
        unsafe { ffi::blImageDecoderReset(&mut self.core) };
    }
}

#[cfg(test)]
mod test_codec {
    use crate::codec::ImageCodec;

    #[test]
    fn test_built_in_codecs() {
        assert_ne!(ImageCodec::built_in_codecs().len(), 0)
    }

    #[test]
    fn test_encoder_creation() {
        let codecs = ImageCodec::built_in_codecs();
        let codec = codecs.first().unwrap();
        let encoder = codec
            .create_encoder()
            .expect("codec does not support encoding");
        assert_eq!(codec, encoder.codec());
    }

    #[test]
    fn test_decoder_creation() {
        let codecs = ImageCodec::built_in_codecs();
        let codec = codecs.first().unwrap();
        let decoder = codec
            .create_decoder()
            .expect("codec does not support decoding");
        assert_eq!(codec, decoder.codec());
    }
}

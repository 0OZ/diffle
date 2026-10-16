//! Recognising the raster formats a browser displays by their leading bytes.

/// A raster image format a browser can show inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterImage {
    Png,
    Jpeg,
    Gif,
    Webp,
    Avif,
    Bmp,
    Icon,
}

impl RasterImage {
    #[must_use]
    pub const fn media_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Avif => "image/avif",
            Self::Bmp => "image/bmp",
            Self::Icon => "image/x-icon",
        }
    }
}

/// Bytes that must sit at `offset`; the bytes between marks may be anything.
struct Mark {
    offset: usize,
    bytes: &'static [u8],
}

struct Signature {
    image: RasterImage,
    marks: &'static [Mark],
}

impl Signature {
    fn matches(&self, head: &[u8]) -> bool {
        self.marks
            .iter()
            .all(|mark| head.get(mark.offset..mark.offset + mark.bytes.len()) == Some(mark.bytes))
    }
}

const fn at(offset: usize, bytes: &'static [u8]) -> Mark {
    Mark { offset, bytes }
}

/// The box that follows the four size bytes of an AVIF file, naming its brand.
const BOX_OFFSET: usize = 4;
/// The `WEBP` tag follows `RIFF` and the four size bytes.
const WEBP_OFFSET: usize = 8;

const SIGNATURES: [Signature; 8] = [
    Signature {
        image: RasterImage::Png,
        marks: &[at(0, &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])],
    },
    Signature {
        image: RasterImage::Jpeg,
        marks: &[at(0, &[0xff, 0xd8, 0xff])],
    },
    Signature {
        image: RasterImage::Gif,
        marks: &[at(0, b"GIF8")],
    },
    Signature {
        image: RasterImage::Webp,
        marks: &[at(0, b"RIFF"), at(WEBP_OFFSET, b"WEBP")],
    },
    Signature {
        image: RasterImage::Avif,
        marks: &[at(BOX_OFFSET, b"ftypavif")],
    },
    Signature {
        image: RasterImage::Avif,
        marks: &[at(BOX_OFFSET, b"ftypavis")],
    },
    Signature {
        image: RasterImage::Bmp,
        marks: &[at(0, b"BM")],
    },
    Signature {
        image: RasterImage::Icon,
        marks: &[at(0, &[0x00, 0x00, 0x01, 0x00])],
    },
];

/// The raster format `bytes` begin as, or `None` for anything else. The file name is never
/// consulted: only bytes that are an image get served as one.
#[must_use]
pub fn image_type(bytes: &[u8]) -> Option<RasterImage> {
    SIGNATURES
        .iter()
        .find(|signature| signature.matches(bytes))
        .map(|signature| signature.image)
}

#[cfg(test)]
#[path = "image_type_tests.rs"]
mod tests;

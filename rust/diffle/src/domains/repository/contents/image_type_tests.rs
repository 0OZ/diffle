use super::*;

fn padded(head: &[u8]) -> Vec<u8> {
    let mut bytes = head.to_vec();
    bytes.extend([0x55; 16]);
    bytes
}

#[test]
fn names_each_format_by_its_magic_bytes() {
    let cases: [(&[u8], RasterImage, &str); 8] = [
        (b"\x89PNG\r\n\x1a\n", RasterImage::Png, "image/png"),
        (b"\xff\xd8\xff\xe0", RasterImage::Jpeg, "image/jpeg"),
        (b"GIF89a", RasterImage::Gif, "image/gif"),
        (
            b"RIFF\x01\x02\x03\x04WEBPVP8 ",
            RasterImage::Webp,
            "image/webp",
        ),
        (b"\0\0\0\x1cftypavif", RasterImage::Avif, "image/avif"),
        (b"\0\0\0\x1cftypavis", RasterImage::Avif, "image/avif"),
        (b"BM\x36\0", RasterImage::Bmp, "image/bmp"),
        (b"\0\0\x01\0\x01\0", RasterImage::Icon, "image/x-icon"),
    ];
    for (head, image, media_type) in cases {
        let found = image_type(&padded(head));
        assert_eq!(found, Some(image), "{head:?}");
        assert_eq!(image.media_type(), media_type);
    }
}

#[test]
fn the_bytes_between_marks_are_wild() {
    let mut riff = b"RIFF\xde\xad\xbe\xefWEBP".to_vec();
    assert_eq!(image_type(&riff), Some(RasterImage::Webp));
    riff[8] = b'X';
    assert_eq!(image_type(&riff), None);
    assert_eq!(
        image_type(b"\xaa\xbb\xcc\xddftypavif"),
        Some(RasterImage::Avif)
    );
}

#[test]
fn anything_else_and_anything_too_short_is_not_an_image() {
    assert_eq!(image_type(b""), None);
    assert_eq!(image_type(b"fn main() {}\n"), None);
    assert_eq!(
        image_type(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
        None
    );
    // A prefix of a signature is not the signature.
    assert_eq!(image_type(b"\x89PNG\r\n\x1a"), None);
    assert_eq!(image_type(b"RIFF\0\0\0\0WEB"), None);
    assert_eq!(image_type(b"\0\0\0\x1cftypavi"), None);
    assert_eq!(image_type(b"B"), None);
}

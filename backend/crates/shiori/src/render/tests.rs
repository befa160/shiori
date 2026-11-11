use shiori_testkit::{jpeg, png};

use super::*;

#[test]
fn jpeg_keeps_the_aspect_ratio_and_the_picture() {
    let out = render(&png(600, 900, [10, 200, 30]), 320, Format::Jpeg).unwrap();
    let image = image::load_from_memory(&out).unwrap().to_rgb8();
    assert_eq!(image.dimensions(), (320, 480));
    let [r, g, b] = image.get_pixel(160, 240).0;
    assert!(r.abs_diff(10) <= 4 && g.abs_diff(200) <= 4 && b.abs_diff(30) <= 4);
}

#[test]
fn avif_is_a_real_avif_of_the_right_size() {
    let out = render(&jpeg(800, 1280, [120, 60, 200]), 480, Format::Avif).unwrap();
    assert_eq!(&out[4..12], b"ftypavif");
    let size = imagesize::blob_size(&out).unwrap();
    assert_eq!((size.width, size.height), (480, 768));
}

#[test]
fn undecodable_input_is_an_error() {
    assert!(render(b"pas une image", 320, Format::Jpeg).is_err());
    let full = png(600, 900, [1, 2, 3]);
    assert!(render(&full[..full.len() / 2], 320, Format::Avif).is_err());
}

#[test]
fn formats_have_their_media_types() {
    assert_eq!(
        (Format::Avif.extension(), Format::Avif.media_type()),
        ("avif", "image/avif")
    );
    assert_eq!(
        (Format::Jpeg.extension(), Format::Jpeg.media_type()),
        ("jpg", "image/jpeg")
    );
}

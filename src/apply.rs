use image::{DynamicImage, Luma};
use imageproc::map::map_pixels;
use splines::Spline;

pub fn apply(image: &DynamicImage, curve: &Spline<f64, f64>) -> DynamicImage {
    let input_image_16 = image.to_luma16();

    return DynamicImage::ImageLuma16(map_pixels(&input_image_16, |_x, _y, p| {
        Luma([curve.clamped_sample(p[0] as f64).unwrap() as u16])
    }));
}

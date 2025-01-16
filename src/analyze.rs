use image::{DynamicImage, GenericImageView, ImageBuffer, Luma, SubImage};
use imageproc::drawing::draw_filled_rect_mut;
use imageproc::edges;
use imageproc::hough;
use imageproc::map::map_pixels;
use imageproc::rect::Rect;
use splines::{Interpolation, Key, Spline};

#[derive(Debug)]
pub enum AnalyzeError {
    Err(String),
}

pub struct AnalyzeResults {
    pub edges_image: ImageBuffer<Luma<u8>, Vec<u8>>,
    pub normalized_image: ImageBuffer<Luma<u16>, Vec<u16>>,
    pub curve: Spline<f64, f64>,
}

/* analyze takes a path to a scanned image (input) and a path to a
 * directory to write its outputs.
 *
 * Analyze looks at an input image assumed to be a scan of a print
 * of the generated image from `generate`. It then searches that
 * image for the sequence of tonal values in each square and outputs
 * a curve adjustment that maps the scanned tonal values to a linear
 * tone curve.
 *
 */
pub fn analyze(image: &DynamicImage, debug: bool) -> Result<AnalyzeResults, AnalyzeError> {
    let square_count: u16 = 101;
    let image_16 = image.to_luma16();

    if debug {
        println!("Detecting Edges...");
    }
    // Canny is an edge detection algorithm, it's the input to the hough transform
    // we'll use later to do line detection
    let edges_image = edges::canny(&image.to_luma8(), 50.0, 100.0);

    if debug {
        println!("Finding Lines...");
    }
    // Detect lines using Hough transform. The generated image uses lines to differentiate the
    // steps in the print This should allow us then to find those lines and then search the image
    // for our steps.
    let options = hough::LineDetectionOptions {
        vote_threshold: 200,
        suppression_radius: 8,
    };
    let lines: Vec<hough::PolarLine> = hough::detect_lines(&edges_image, options);

    if debug {
        generate_hough_lines_image(&edges_image, &lines);
    }

    if debug {
        println!("Searching for grid...");
    }

    // Note! In the future lines wont be perfectly alinged, I'll need to find
    // the angle of a nearby line and then adjust the image to match that
    //
    // See: https://docs.rs/imageproc/latest/imageproc/geometric_transformations/fn.rotate.html

    let vertical_lines: Vec<&hough::PolarLine> =
        lines.iter().filter(|l| l.angle_in_degrees == 90).collect();

    let horizontal_lines: Vec<&hough::PolarLine> =
        lines.iter().filter(|l| l.angle_in_degrees == 0).collect();

    // Safety check to make sure our image is clear enough to find all the lines
    if vertical_lines.len() < 11 || horizontal_lines.len() < 11 {
        panic!("Failed to find all the lines");
    }

    // A note: For vertical lines "r" in the PolarLine is the same as the x coordinate. For
    // horizontal lines "r" is the same as the y coordinate.
    let origin_x = vertical_lines[0].r as u32;
    let origin_y = horizontal_lines[0].r as u32;
    if debug {
        println!("Origin: ({}, {})", origin_x, origin_y);
    }

    // Find the distance between the first two lines. Use it to find our squares
    let square_size = (vertical_lines[1].r - vertical_lines[0].r).floor() as u32;
    if debug {
        println!("Square Size: {}", square_size);
    }

    let mut samples: Vec<u16> = vec![0; square_count as usize];
    let mut n = 0;
    for row in 0..11 {
        for col in 0..10 {
            if n >= 101 {
                break;
            }
            let x = origin_x + (col * square_size);
            let y = origin_y + (row * square_size);

            let view = image_16.view(x + 25, y + 25, square_size - 30, square_size - 30);
            let sample = sampled_mean(view);
            samples[n] = sample;
            n += 1;
        }
    }

    let expected_interval = u16::MAX / (square_count - 1);
    let expected_values = (0..square_count).map(|x| x * expected_interval);

    // Now normalize the samples based on the maximum and minimum values
    // We expect for the max observed to be greater than zero and the minimum
    // to to less than u16::max. Assuming the print was printed to d-max then
    // we want to distribute our observed values evenly between max and min
    // before determining curve adjustments

    let sample_min = samples.iter().min().ok_or(AnalyzeError::Err(
        "No min sample found, no valid samples, check source image.".to_string(),
    ))?;
    let sample_max = samples.iter().max().ok_or(AnalyzeError::Err(
        "No max sample found, no valid samples, check source image.".to_string(),
    ))?;

    if debug {
        println!("sample min: {}", sample_min);
        println!("sample max: {}", sample_max);
    }

    /* example
     *
     * Suppose we have:
     *  - a full range from [0, 65535] inclusive
     *  - a subset in the range [256, 25280]
     *
     * If we want to expand the subset to fill the full range we can:
     * subtract the minimum value from all the values in the subset to
     * bring the minimum value to 0;
     *
     * subset - 256 => [0, 65024]
     *
     * Then we need to expand those values to fill up to 65535 by multiplying them
     * by 65535 / our new max (65024)
     */
    let normalize_factor = (u16::MAX as f32) / ((sample_max - sample_min) as f32);

    if debug {
        println!("dynamic range: {}", sample_max - sample_min);
        println!("normalize_factor: {}", normalize_factor);
    }

    let normalized_samples: Vec<u16> = samples
        .iter()
        .map(|s| ((s - sample_min) as f32 * normalize_factor) as u16)
        .collect();

    let normalized_image = map_pixels(&image_16, |_, _, p| {
        let new_v = p[0].saturating_sub(*sample_min);
        Luma([(new_v as f32 * normalize_factor) as u16])
    });

    //if debug {
    //    normalized_image
    //        .save(output_dir.join("normalized.png"))
    //        .unwrap();
    //}

    /* Use our own observed values to find where we should place
     * our points to curve with
     *
     * Ax an example let's assume we have a table like below:
     *
     * ---
     * i    exp     norm
     * ---
     * 1    0       0
     * 2    655     648
     * ...
     * 15   6550    5000
     * ...
     * 20   9825    6576
     * ...
     * 101  65500   64800
     * ---
     *
     * Using this we can figure out how to push our densities around to get a linear relationship.
     *
     * like take the 15th step. In a linear relationshop the input density of 6550 would be
     * perfectly reflected in the observed density 6550 -> 6550. But in our real world test it
     * didn't.
     *
     * So instead we want to search through our observed densities and figure out what input
     * density did create an ouput of 6550. Looking we can see step 20 was close.
     *
     * So when mapping our values when we render a new 15 we want the density to be 9825 so that it
     * achieve an output density close to 6550.
     *
     * We'll find our value by finding the largest input density that is still less than our
     * target, and the least input density that is still greater than our density. We'll then use
     * the midpoint.
     */

    // assume a linear relationship, so every value of expected on the x
    // axis should be expected on the y axis. Our observed values will be
    // different. The curve is the delta.

    let samples_with_expected_values: Vec<(u16, u16)> = expected_values
        .clone()
        .into_iter()
        .zip(normalized_samples)
        .collect();

    let curve_points: Vec<(u16, u16)> = expected_values
        .clone()
        .into_iter()
        .map(|e| {
            (
                e,
                find_closest_matching_input_density(&samples_with_expected_values, e),
            )
        })
        .collect();

    //if debug {
    //    let mut observed_file = fs::File::create(output_dir.join("observed.csv")).unwrap();
    //    for (e, s) in samples_with_expected_values.clone() {
    //        observed_file
    //            .write(format!("{},{}\n", e, s).as_bytes())
    //            .unwrap();
    //    }
    //    let mut curve_points_file = fs::File::create(output_dir.join("curve_points.csv")).unwrap();
    //    for (e, s) in curve_points.clone() {
    //        curve_points_file
    //            .write(format!("{},{}\n", e, s).as_bytes())
    //            .unwrap();
    //    }
    //}

    let curve = best_fit_spline(&curve_points);
    //let curve_file = fs::File::create(output_dir.join("curve.json")).unwrap();
    //serde_json::to_writer(&curve_file, &curve).unwrap();

    //if debug {
    //    let mut curve_image: ImageBuffer<im::Rgb<u8>, Vec<u8>> = ImageBuffer::new(1024, 1024);
    //    draw_curve(&mut curve_image, &curve);

    //    let histogram = create_histogram(&normalized_image);
    //    //println!("Histogram: {:?}", histogram);
    //    draw_histogram(&mut curve_image, &histogram);

    //    curve_image.save(output_dir.join("curve.png")).unwrap();
    //}

    Ok(AnalyzeResults {
        edges_image,
        normalized_image,
        curve,
    })
}

fn generate_hough_lines_image(
    edges_image: &ImageBuffer<Luma<u8>, Vec<u8>>,
    lines: &Vec<hough::PolarLine>,
) -> ImageBuffer<image::Rgb<u8>, Vec<u8>> {
    let white = image::Rgb::<u8>([255, 255, 255]);
    let green = image::Rgb::<u8>([0, 255, 0]);
    let black = image::Rgb::<u8>([0, 0, 0]);

    // Convert edge image to colour
    let color_edges = map_pixels(edges_image, |_, _, p| if p[0] > 0 { white } else { black });

    let horiz_and_vert_lines: Vec<hough::PolarLine> = lines
        .into_iter()
        .filter(|l| l.angle_in_degrees == 90 || l.angle_in_degrees == 0)
        .map(|l| *l)
        .collect();

    // Draw lines on top of edge image
    hough::draw_polar_lines(&color_edges, &horiz_and_vert_lines, green);
    color_edges
}

/* Generate a spline (that can later be sampled from) based on the a vector of 2D points. Used for
 * creating the correction curve.
 */
fn best_fit_spline(curve: &Vec<(u16, u16)>) -> Spline<f64, f64> {
    Spline::from_vec(
        curve
            .into_iter()
            .map(|(input_density, output_density)| {
                Key::new(
                    *input_density as f64,
                    *output_density as f64,
                    Interpolation::default(),
                )
            })
            .collect(),
    )
}

/* This is hardly "sampled" at this point. Instead it just finds the mean value
 * of ALL of the pixels in the given Rect
 */
fn sampled_mean(image: SubImage<&ImageBuffer<Luma<u16>, Vec<u16>>>) -> u16 {
    let (width, height) = image.dimensions();
    let mut total: u64 = 0;
    let count = (width * height) as u64;

    for x in 0..width {
        for y in 0..height {
            let pixel = image.get_pixel(x, y);
            total += pixel[0] as u64
        }
    }

    return (total / count) as u16;
}

fn draw_curve(image: &mut ImageBuffer<image::Rgb<u8>, Vec<u8>>, curve: &Spline<f64, f64>) {
    for i in (0..u16::MAX).step_by(64) {
        let green = image::Rgb::<u8>([0, 255, 0]);
        let y = 1024 - (curve.clamped_sample(i as f64).unwrap() / 64.) as u32;
        let x = (i / 64) as u32;
        image.put_pixel(x, y, green);
    }
}

fn draw_histogram(image: &mut ImageBuffer<image::Rgb<u8>, Vec<u8>>, histogram: &Vec<u32>) {
    let white = image::Rgb::<u8>([255, 255, 255]);
    let total: u32 = histogram.into_iter().sum();

    for (i, value) in histogram.into_iter().enumerate() {
        if i == 0 || i == 256 {
            continue;
        }
        let scaled_percentage = (((*value as f32) / (total as f32)) * 1024. * 5.) as u32;

        if scaled_percentage > 0 {
            let x = (i * 4) as i32;
            let rect = Rect::at(x, (1024 - scaled_percentage) as i32).of_size(4, scaled_percentage);
            draw_filled_rect_mut(image, rect, white);
        }
    }
}

/* Look through the haystack of (input_density, output_density) for the input density with the
 * output density that most closely matches needle.
 *
 * An exact match is unlikely, so instead build a range of values. First searching from the start
 * forward until finding the first output density larger than needle. Then reversing the search to
 * find the first output density smaller than needle. Interpolate the two input densities for our
 * resulting value.
 */
fn find_closest_matching_input_density(haystack: &Vec<(u16, u16)>, needle: u16) -> u16 {
    let mut lower_bound_density: Option<u16> = None;
    let mut upper_bound_density: Option<u16> = None;

    for (i, (_, output_density)) in haystack.into_iter().enumerate() {
        if *output_density >= needle {
            if i == 0 {
                lower_bound_density = Some(0);
                break;
            }

            // if we found an exact match don't look backwards
            let adjustment = if *output_density == needle { 0 } else { 1 };
            let (input_density, _) = haystack[i - adjustment];
            lower_bound_density = Some(input_density);
            break;
        }
    }

    for (i, (_, output_density)) in haystack.into_iter().rev().enumerate() {
        if *output_density <= needle {
            if i == 0 {
                upper_bound_density = Some(u16::MAX);
                break;
            }
            // if we found an exact match don't look backwards
            let adjustment = if *output_density == needle { 0 } else { 1 };

            let (input_density, _) = haystack[(haystack.len() - adjustment) - i];
            upper_bound_density = Some(input_density);
            break;
        }
    }

    match (lower_bound_density, upper_bound_density) {
        (None, None) => panic!("Unable to map tones, value out of range"),
        (Some(l), None) => l,
        (None, Some(u)) => u,
        (Some(l), Some(u)) => (((l as u32) + (u as u32)) / 2) as u16,
    }
}

// simple histogram of the image with 256 buckets
fn create_histogram(image: &ImageBuffer<Luma<u16>, Vec<u16>>) -> Vec<u32> {
    let mut histogram: Vec<u32> = vec![0; 256];

    for (_, _, p) in image.enumerate_pixels() {
        let bucket = (p[0] / 256) as usize;
        histogram[bucket] = histogram[bucket].saturating_add(1)
    }

    histogram
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampled_mean_zero() {
        let buffer: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::new(100, 100);
        let sub_image = SubImage::new(&buffer, 10, 10, 10, 10);
        let result = sampled_mean(sub_image);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_sampled_mean() {
        let mut buffer: ImageBuffer<Luma<u16>, Vec<u16>> = ImageBuffer::new(100, 100);
        for x in 0..100 {
            for y in 0..100 {
                buffer.put_pixel(x, y, Luma([(x * y) as u16]))
            }
        }
        let sub_image = SubImage::new(&buffer, 10, 10, 10, 10);
        let result = sampled_mean(sub_image);
        assert_eq!(result, 210);
    }

    #[test]
    fn test_find_closest_matching_input_density() {
        let haystack = vec![(1, 1), (2, 4), (3, 9), (4, 16), (5, 25)];
        let mut result = find_closest_matching_input_density(&haystack, 2);
        assert_eq!(result, 1);

        result = find_closest_matching_input_density(&haystack, 5);
        assert_eq!(result, 2);

        result = find_closest_matching_input_density(&haystack, 19);
        assert_eq!(result, 4);

        // check an exact match
        result = find_closest_matching_input_density(&haystack, 9);
        assert_eq!(result, 3);
    }
}

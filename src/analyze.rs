use anyhow::{anyhow, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Luma, Rgb, SubImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut};
use imageproc::map::map_pixels;
use imageproc::rect::Rect;
use splines::{Interpolation, Key, Spline};

use super::step_description::StepDescription;

pub struct AnalyzeResults {
    pub normalized_image: DynamicImage,
    pub curve: Spline<f64, f64>,
    pub histogram: Vec<u32>,
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
pub fn analyze(
    image: &DynamicImage,
    invert_image: bool,
    debug: bool,
) -> anyhow::Result<AnalyzeResults> {
    let step_description = StepDescription::new(101, 10, 1000, u16::MAX as u32);
    let input_values = step_description.input_values();

    // convert to a 16bit Greyscale image this is our working set
    let image_16 = image.to_luma16();

    // convert to 8bit greyscale used for edge / line detection
    let image_8 = image.to_luma8();

    let grid_analysis = analyze_grid(&image_8)?;
    let sampled_areas = sampled_areas(&step_description, &grid_analysis);
    let samples = collect_samples(&image_16, &sampled_areas);

    if debug {
        println!("Found: {} samples", samples.values.len());
        println!("sample min: {}", samples.min);
        println!("sample max: {}", samples.max);
        println!("dynamic range: {}", samples.max - samples.min);
    }

    let NormalizedResults {
        image: normalized_image,
        samples: normalized_samples,
    } = normalize_image(&step_description, &image_16, &samples, invert_image);

    let curve_points = linearize_inputs(&input_values, &normalized_samples)?;
    if debug {
        println!("curve_points\n{:?}", curve_points);
    }
    let curve = best_fit_spline(&curve_points);
    let histogram = create_histogram(&normalized_image, &grid_analysis, &step_description);

    let normalized_image_with_rects =
        draw_sampled_areas(&DynamicImage::ImageLuma16(normalized_image), &sampled_areas)?;

    Ok(AnalyzeResults {
        normalized_image: DynamicImage::ImageRgb8(normalized_image_with_rects),
        histogram,
        curve,
    })
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

fn draw_sampled_areas(
    image: &DynamicImage,
    rects: &Vec<Rect>,
) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    let mut image_rgb = image.to_rgb8();
    let green = image::Rgb::<u8>([0, 255, 0]);
    for r in rects {
        draw_hollow_rect_mut(&mut image_rgb, *r, green)
    }
    Ok(image_rgb)
}

pub fn draw_curve(
    image: &mut ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    curve: &Spline<f64, f64>,
) -> Result<()> {
    for i in (0..u16::MAX).step_by(64) {
        let green = image::Rgb::<u8>([0, 255, 0]);
        let sample = curve
            .clamped_sample(i as f64)
            .ok_or(anyhow!("failed to sample spline"))?;
        let y = 1023 - (sample / 64.) as u32;
        let x = (i / 64) as u32;
        image.put_pixel(x, y, green);
    }
    Ok(())
}

/* Draws a histogram ontop of `image`
 *
 * expects the image to be 1024x1024
 */
pub fn draw_histogram(
    image: &mut ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    histogram: &Vec<u32>,
) -> anyhow::Result<()> {
    let grey = image::Rgb::<u8>([128, 128, 128]);

    // The first and last buckets tend to get filled with stuff like
    // lines and letters, not useful. Remove them.
    let histogram_minus = &histogram[1..256];

    let max = histogram_minus
        .into_iter()
        .max()
        .ok_or(anyhow!("could not find maximum histogram value"))?;

    for (i, value) in histogram_minus.into_iter().enumerate() {
        if i == 0 || i == 256 {
            continue;
        }

        let scaled_percentage = (((*value as f32) / (*max as f32)) * 1024.) as u32;

        let x = (i * 4) as i32;
        let rect = Rect::at(x, (1024 - scaled_percentage) as i32).of_size(4, scaled_percentage);
        draw_filled_rect_mut(image, rect, grey);
    }
    Ok(())
}

/* Look through the haystack of (input_density, output_density) for the input density with the
 * output density that most closely matches needle.
 *
 * An exact match is unlikely, so instead build a range of values. First searching from the start
 * forward until finding the first output density larger than needle. Then reversing the search to
 * find the first output density smaller than needle. Interpolate the two input densities for our
 * resulting value.
 *
 * Issue: If the highs or lows completely or nearly clip then I think we end up with clumbs at the top and
 * bottom. Need a way to avoid this.
 *
 * for example of our distrbution looks like
 *
 * > [(0, 0), (5,0), (10,0), (15, 5), (20,10), (25, 15), (30, 20), (35, 20), (40, 20)]
 *
 * searching for the output value that match 2 we would look forward and find the first greater
 * output (5) and record (10,0) as our match. Then looking backward we would find (10,0) as the
 * first lesser value and record (15,5) as our match.
 *
 *
 */
fn find_closest_matching_input_density(
    haystack: &Vec<(u16, u16)>,
    needle: u16,
) -> anyhow::Result<u16> {
    let mut lower_bound_density: Option<u16> = None;
    let mut upper_bound_density: Option<u16> = None;

    // search forward, find the first output_density /greater/ than needle
    // our lower bound will then be the input density immediately prior
    for (i, (_, output_density)) in haystack.into_iter().enumerate() {
        if *output_density > needle {
            if i == 0 {
                lower_bound_density = Some(0);
            } else {
                lower_bound_density = Some(haystack[i - 1].0);
            }

            break;
        }
    }

    // search backwards, find the first output_density /lesser/ than needle
    // our upper bound will then be the input density immediately prior
    for (i, (_, output_density)) in haystack.into_iter().rev().enumerate() {
        if *output_density < needle {
            if i == 0 {
                upper_bound_density = Some(u16::MAX);
            } else {
                upper_bound_density = Some(haystack[haystack.len() - i].0);
            }

            break;
        }
    }

    let closest = match (lower_bound_density, upper_bound_density) {
        (None, None) => {
            return Err(anyhow!("Unable to map tones, value out of range"));
        }
        (Some(l), None) => l,
        (None, Some(u)) => u,
        (Some(l), Some(u)) => (((l as u32) + (u as u32)) / 2) as u16,
    };

    Ok(closest)
}

// simple histogram of the image with 256 buckets
fn create_histogram(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    grid_analysis: &GridAnalysis,
    step_description: &StepDescription,
) -> Vec<u32> {
    let view = image
        .view(
            grid_analysis.origin_x,
            grid_analysis.origin_y,
            grid_analysis.square_size * step_description.columns,
            grid_analysis.square_size * step_description.rows,
        )
        .to_image();

    let mut histogram: Vec<u32> = vec![0; 256];

    for (_, _, p) in view.enumerate_pixels() {
        let bucket = (p[0] / 256) as usize;
        histogram[bucket] = histogram[bucket].saturating_add(1)
    }

    histogram
}

struct GridAnalysis {
    origin_x: u32,
    origin_y: u32,
    square_size: u32,
}

// Analyzes `image` looking for the grid of squares
//
// returns the discovered x,y cordinates of the top left corner of the grid, the observed square
// size, as well as the lines image used for rendering the results
//
// Note: Consider making the lines image a function so we don't have to pre-compute?
fn analyze_grid(image: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<GridAnalysis> {
    // Find the distance between the first two lines. Use it to find our squares
    let (width, _) = image.dimensions();
    let square_size = width / 10;

    Ok(GridAnalysis {
        origin_x: 0,
        origin_y: 0,
        square_size,
    })
}

struct Samples {
    values: Vec<u16>,
    min: u16,
    max: u16,
}

fn collect_samples(image: &ImageBuffer<Luma<u16>, Vec<u16>>, rects: &Vec<Rect>) -> Samples {
    let mut values: Vec<u16> = vec![0; rects.len()];
    let mut max: u16 = 0;
    let mut min: u16 = u16::MAX;

    for (i, r) in rects.iter().enumerate() {
        let view = image.view(r.left() as u32, r.top() as u32, r.width(), r.height());
        let sample = sampled_mean(view);

        values[i] = sample;
        if sample > max {
            max = sample;
        }
        if sample < min {
            min = sample;
        }
    }

    Samples { values, max, min }
}

struct NormalizedResults {
    image: ImageBuffer<Luma<u16>, Vec<u16>>,
    samples: Vec<u16>,
}

// Now normalize the samples based on the maximum and minimum values
// We expect for the max observed to be greater than zero and the minimum
// to to less than u16::max. Assuming the print was printed to d-max then
// we want to distribute our observed values evenly between max and min
// before determining curve adjustments
fn normalize_image(
    step_description: &StepDescription,
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    samples: &Samples,
    invert_image: bool,
) -> NormalizedResults {
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
    let normalize_factor =
        (step_description.max_tone as f32) / ((samples.max - samples.min) as f32);

    let mut normalized_samples: Vec<u16> = samples
        .values
        .iter()
        .map(|s| ((s - samples.min) as f32 * normalize_factor) as u16)
        .collect();

    // this is dumb but I've changed how I want the order to work
    // it used to be black to white, this is white to black
    if !invert_image {
        normalized_samples.reverse();
    }

    let normalized_image = map_pixels(image, |_, _, p| {
        let new_v = p[0].saturating_sub(samples.min);
        Luma([(new_v as f32 * normalize_factor) as u16])
    });

    NormalizedResults {
        image: normalized_image,
        samples: normalized_samples,
    }
}

/* Use our own observed values to find where we should place
 * our points to curve with
 *
 * As an example let's assume we have a table like below:
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
fn linearize_inputs(
    input_values: &Vec<u16>,
    normalized_samples: &Vec<u16>,
) -> Result<Vec<(u16, u16)>> {
    // assume a linear relationship, so every value of expected on the x
    // axis should be expected on the y axis. Our observed values will be
    // different. The curve is the delta.
    let input_values_with_samples: Vec<(u16, u16)> = input_values
        .clone()
        .into_iter()
        .zip(normalized_samples.into_iter().copied())
        .collect();

    input_values
        .clone()
        .into_iter()
        .map(|e| find_closest_matching_input_density(&input_values_with_samples, e).map(|c| (e, c)))
        .collect()
}

fn sampled_areas(step_description: &StepDescription, grid_analysis: &GridAnalysis) -> Vec<Rect> {
    let mut n: usize = 0;
    let mut rects = Vec::new();

    // 10% margin around the whole square
    let margin = (grid_analysis.square_size as f32 * 0.25).floor() as u32;
    let analyzed_size = grid_analysis.square_size - (2 * margin);

    for row in 0..step_description.rows {
        for col in 0..step_description.columns {
            if n >= step_description.count as usize {
                break;
            }
            let x = grid_analysis.origin_x + (col * grid_analysis.square_size) + margin;
            let y = grid_analysis.origin_y + (row * grid_analysis.square_size) + margin;

            // this is a "window" of the square, stepped in 25-30 pixels on each side so as
            // to avoid any malarky with the ednge of the square or the number on the top
            // left corner
            let rect = Rect::at(x as i32, y as i32).of_size(analyzed_size, analyzed_size);
            rects.push(rect);
            n += 1;
        }
    }
    rects
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
        let haystack = vec![
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 4),
            (5, 9),
            (6, 16),
            (7, 25),
            (8, 25),
            (9, 25),
            (10, 25),
        ];
        // l_bound = 3
        // u_bound = None
        // result = floor((3 + 4) / 2) => 3
        let mut result = find_closest_matching_input_density(&haystack, 0).unwrap();
        assert_eq!(result, 3);

        // l_bound = 3
        // u_bound = None
        // result = floor((3 + 4) / 2) => 3
        result = find_closest_matching_input_density(&haystack, 1).unwrap();
        assert_eq!(result, 3);

        // l_bound = None
        // u_bound = 7
        result = find_closest_matching_input_density(&haystack, 25).unwrap();
        assert_eq!(result, 7);

        // l_bound = 3
        // u_bound = 4
        // result = floor((3 + 4) / 2) => 3
        result = find_closest_matching_input_density(&haystack, 2).unwrap();
        assert_eq!(result, 3);

        // l_bound = 4
        // u_bound = 5
        // result = floor((4 + 5) / 2) => 4
        result = find_closest_matching_input_density(&haystack, 5).unwrap();
        assert_eq!(result, 4);

        // l_bound = 6
        // u_bound = 7
        // result = floor((6 + 7) / 2) => 6
        result = find_closest_matching_input_density(&haystack, 19).unwrap();
        assert_eq!(result, 6);

        // check an exact match
        result = find_closest_matching_input_density(&haystack, 9).unwrap();
        assert_eq!(result, 5);
    }
}

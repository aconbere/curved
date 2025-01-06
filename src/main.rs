use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use image as im;
use image::{DynamicImage, ImageBuffer, Luma};

use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use imageproc::edges;
use imageproc::hough;
use imageproc::map::map_pixels;
use imageproc::rect::Rect;

use ab_glyph::FontRef;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long)]
    debug: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Adds files to myapp
    Scan {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(short, long)]
        output_dir: PathBuf,
    },
    Generate {
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn sampled_mean(image: &ImageBuffer<Luma<u16>, Vec<u16>>, rect: Rect) -> u16 {
    let count = rect.width() * rect.height();
    let mut total: u32 = 0;

    for x in 0..rect.width() {
        for y in 0..rect.height() {
            let pixel = image.get_pixel(rect.left() as u32 + x, rect.top() as u32 + y);
            total += pixel[0] as u32
        }
    }

    return ((total as u32) / count) as u16;
}

fn draw_hough_lines_image(
    edges_image: &ImageBuffer<Luma<u8>, Vec<u8>>,
    lines: &Vec<hough::PolarLine>,
    output_dir: &PathBuf,
) {
    let white = im::Rgb::<u8>([255, 255, 255]);
    let green = im::Rgb::<u8>([0, 255, 0]);
    let black = im::Rgb::<u8>([0, 0, 0]);

    // Convert edge image to colour
    let color_edges = map_pixels(edges_image, |_, _, p| if p[0] > 0 { white } else { black });

    // Draw lines on top of edge image
    let lines_image = hough::draw_polar_lines(&color_edges, &lines, green);
    let lines_path = output_dir.join("lines.png");
    lines_image.save(&lines_path).unwrap();
}

/* scan takes a path to a scanned image (input) and a path to a
 * directory to write its outputs.
 *
 * Scan looks at an input image assumed to be a scan of a print
 * of the generated image from `generate`. It then searches that
 * image for the sequence of tonal values in each square and outputs
 * a curve adjustment that maps the scanned tonal values to a linear
 * tone curve.
 *
 */
fn scan(input: &PathBuf, output_dir: &PathBuf, debug: bool) {
    let square_count = 101;
    let input_file_path = fs::canonicalize(&input).unwrap();
    let output_dir = fs::canonicalize(&output_dir).unwrap();

    let image = image::open(&input_file_path).unwrap();
    let image_16 = image.to_luma16();

    // note: Once processing scans we'll want to scale the image appropriately
    let (width, height) = image_16.dimensions();
    if debug {
        println!("Dimensions: ({}, {})", width, height);
    }

    // Canny is an edge detection algorithm, it's the input to the hough transform
    // we'll use later to do line detection
    let edges_image = edges::canny(&image.to_luma8(), 50.0, 100.0);

    if debug {
        edges_image.save(output_dir.join("canny.png")).unwrap();
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
        draw_hough_lines_image(&edges_image, &lines, &output_dir);
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
    println!("Origin: ({}, {})", origin_x, origin_y);

    // Find the distance between the first two lines. Use it to find our squares
    let square_size = (vertical_lines[1].r - vertical_lines[0].r).floor() as u32;
    println!("Square Size: {}", square_size);

    let mut samples: Vec<u16> = vec![0; square_count];
    let mut n = 0;
    for row in 0..11 {
        for col in 0..10 {
            if n >= 101 {
                break;
            }
            let x = origin_x + (col * square_size);
            let y = origin_y + (row * square_size);

            // Take the expected 100x100 pixel square at (x,y) and avoid the numbers in the top
            // left as well as any line inconsistencies.
            let rect = Rect::at((x + 25) as i32, (y + 25) as i32)
                .of_size(square_size - 30, square_size - 30);

            let sample = sampled_mean(&image_16, rect);
            samples[n] = sample;
            println!("Sample: {}: {:?} - {}", n, rect, sample);
            n += 1;
        }
    }

    // Now normalize the samples based on the maximum and minimum values
    // We expect for the max observed to be greater than zero and the minimum
    // to to less than u16::max. Assuming the print was printed to d-max then
    // we want to distribute our observed values evenly between max and min
    // before applying a curve.
}

/* generate takes an output path and creates a black and white stepwedge
 * 0 is black
 * 65536 is white
 *
 * divide the range by count then draw that value into each square
 */
fn generate(output: &PathBuf, _debug: bool) {
    const FONT_BYTES: &[u8] = include_bytes!("../data/fonts/Lato-Black.ttf");
    let font = FontRef::try_from_slice(FONT_BYTES).unwrap();

    let count = 101;
    let columns = 10;
    let rows = (count as f32 / columns as f32).ceil() as u32;

    // number of pixels each square
    let square_size: u32 = 100;

    // count of pixels on the margin of the image
    let margin = 10;

    let width = columns * square_size + (margin * 2);
    let height = (rows * square_size) + (margin * 2);

    let mut image = DynamicImage::new_luma16(width, height).to_luma16();

    // the amount that each square increases as we go towards max
    // Note that because we start at zero we want 100 equal chunks
    // to filled up 101 times
    let interval = u16::MAX / (count - 1);

    let mut n = 0;
    for row in 0..rows {
        for col in 0..columns {
            let x = (margin + (col * square_size)) as i32;
            let y = (margin + (row * square_size)) as i32;

            let rect = Rect::at(x, y).of_size(square_size, square_size);
            let tone = interval * n;
            let foreground_color = if n < count / 2 { u16::MAX } else { 0 };

            draw_filled_rect_mut(&mut image, rect, Luma([tone]));

            draw_text_mut(
                &mut image,
                Luma([foreground_color]),
                x + 5,
                y + 5,
                20 as f32,
                &font,
                format!("{}", n).as_str(),
            );

            n += 1;
            if n == count {
                break;
            }
        }
    }

    // Draw the horizontal grid lines
    for row in 0..rows {
        // flip the tone from the color of the first square in this line so that it
        // shows up in the dark and lights.
        let foreground_color = if row < rows / 2 { u16::MAX } else { 0 };
        let y = ((row * square_size) + margin) as i32;
        let squares_width = square_size * columns;
        let rect = Rect::at(margin as i32, y).of_size(squares_width, 2);
        draw_filled_rect_mut(&mut image, rect, Luma([foreground_color]));
    }

    // Draw the vertical grid lines
    for col in 0..(columns + 1) {
        // pick a generic middle grey
        let tone = u16::MAX / 2;
        let x = ((col * square_size) + margin) as i32;
        let squares_height = square_size * rows;
        let rect = Rect::at(x, margin as i32).of_size(2, squares_height);
        draw_filled_rect_mut(&mut image, rect, Luma([tone]));
    }

    image.save(output).unwrap();
}

fn main() {
    let args = Args::parse();

    match &args.command {
        Commands::Scan { input, output_dir } => {
            scan(&input, &output_dir, args.debug);
        }
        Commands::Generate { output } => {
            generate(&output, args.debug);
        }
    }
}

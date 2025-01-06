use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use image as im;
use image::{DynamicImage, Luma};

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
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Adds files to myapp
    Scan {
        #[arg(short, long)]
        input: PathBuf,
        output_dir: PathBuf,
    },
    Generate {
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn scan(input: &PathBuf, output_dir: &PathBuf) {
    let input_file_path = fs::canonicalize(&input).unwrap();
    println!("Input File Path: {}", &input_file_path.display());

    let output_dir = fs::canonicalize(&output_dir).unwrap();
    println!("Outout File Path: {}", &output_dir.display());
    let image = image::open(&input_file_path).unwrap().to_luma8();
    let (width, height) = image.dimensions();
    println!("Width: {}", width);
    println!("Height: {}", height);
    let edges_image = edges::canny(&image, 50.0, 100.0);
    edges_image.save(output_dir.join("canny.png")).unwrap();

    // Detect lines using Hough transform
    let options = hough::LineDetectionOptions {
        vote_threshold: 200,
        suppression_radius: 8,
    };
    let lines: Vec<hough::PolarLine> = hough::detect_lines(&edges_image, options);

    let white = im::Rgb::<u8>([255, 255, 255]);
    let green = im::Rgb::<u8>([0, 255, 0]);
    let black = im::Rgb::<u8>([0, 0, 0]);

    // Convert edge image to colour
    let color_edges = map_pixels(&edges_image, |_, _, p| if p[0] > 0 { white } else { black });

    // Draw lines on top of edge image
    let lines_image = hough::draw_polar_lines(&color_edges, &lines, green);
    let lines_path = output_dir.join("lines.png");
    lines_image.save(&lines_path).unwrap();

    let vertical_lines: Vec<&hough::PolarLine> =
        lines.iter().filter(|l| l.angle_in_degrees == 90).collect();

    for vl in vertical_lines.iter() {
        println!("Vertical Line: {:?}", vl);
    }
}

/* generate takes an output path and creates a black and white stepwedge
 * 0 is black
 * 65536 is white
 *
 * divide the range by count then draw that value into each square
 */
fn generate(output: &PathBuf) {
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
    let interval = u16::MAX / count;

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
            scan(&input, &output_dir);
        }
        Commands::Generate { output } => {
            generate(&output);
        }
    }
}

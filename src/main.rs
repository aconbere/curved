use std::fmt;
use std::fs::File;
use std::path::PathBuf;

use clap::Parser;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::ColorType;

fn colortype_to_str(c: ColorType) -> String {
    return match c {
        ColorType::RGB(depth) => format!("RGB: {}", depth),
        ColorType::Gray(depth) => format!("Gray:{}", depth),
        ColorType::Palette(depth) => format!("Palette:{}", depth),
        ColorType::GrayA(depth) => format!("GrayA:{}", depth),
        ColorType::RGBA(depth) => format!("RGBA:{}", depth),
        ColorType::CMYK(depth) => format!("CMYK:{}", depth),
        ColorType::YCbCr(depth) => format!("YCbCr:{}", depth),
    };
}

struct Pos {
    x: u32,
    y: u32,
}

impl Pos {
    fn new(x: u32, y: u32) -> Pos {
        Pos { x, y }
    }
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

struct Image {
    pixels: Vec<u16>,
    width: u32,
    height: u32,
}

fn find_not_black(img: &Image, not_black: u16) -> Option<(Pos, u16)> {
    println!("Looking for less than: {}", not_black);
    let w = img.width / 4;
    let h = img.height / 4;
    println!("Searching to ({}, {})", w, h);
    for x in 0..(w / 4) {
        for y in 0..(h / 4) {
            let pos = Pos::new(x, y);
            let pxl = img.pixel_at(&pos);
            if pxl < not_black {
                return Some((pos, pxl));
            }
        }
    }
    None
}

impl Image {
    fn new(pixels: Vec<u16>, width: u32, height: u32) -> Image {
        return Image {
            pixels,
            width,
            height,
        };
    }

    fn index_from_pos(&self, pos: &Pos) -> usize {
        assert!(
            pos.x <= self.width,
            "Assertion! x: {}, width: {}",
            pos.x,
            self.width
        );
        assert!(
            pos.y <= self.height,
            "Assertion! y: {}, height: {}",
            pos.y,
            self.height
        );
        ((pos.y * self.width) + pos.x).try_into().unwrap()
    }

    fn pixel_at(&self, pos: &Pos) -> u16 {
        let idx = self.index_from_pos(pos);
        self.pixels[idx]
    }

    fn max_black(&self) -> Option<u16> {
        self.sample_at(Pos::new(10, 10), 10, 10).mean_value()
    }

    fn sample_at(&self, pos: Pos, width: u32, height: u32) -> Sample {
        let mut pixels = Vec::new();

        for y in 1..pos.y + height {
            for x in 1..pos.x + width {
                let idx = self.index_from_pos(&Pos::new(x, y));
                pixels.push(self.pixels[idx]);
            }
        }

        Sample::new(pixels)
    }
}

struct Sample {
    pixels: Vec<u16>,
}

impl Sample {
    fn new(pixels: Vec<u16>) -> Sample {
        Sample { pixels }
    }

    fn mean_value(&self) -> Option<u16> {
        let count = self.pixels.len() as u32;
        if count == 0 {
            return None;
        }

        let mut total: u32 = 0;
        for x in self.pixels.iter() {
            total = total + (*x as u32);
        }

        let mean = total / count;
        Some(mean as u16)
    }
}

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[arg(short, long)]
    file_path: PathBuf,
}

fn main() {
    let args = Args::parse();

    println!("File Path: {}", args.file_path.display());
    let tiff_file = File::open(args.file_path).unwrap();

    let mut decoder = Decoder::new(tiff_file).expect("Failed to create decoder");
    let (width, height) = decoder.dimensions().expect("Failed to decode dimensions");

    println!("Width: {}", width);
    println!("Height: {}", height);

    let colortype = decoder.colortype().expect("Failed to decode colortype");
    println!("ColorType: {}", colortype_to_str(colortype));

    let decoding_result = decoder.read_image().expect("Failed to decode tiff");

    let image_data = match decoding_result {
        DecodingResult::U16(res) => res,
        _ => panic!("wrong bitdepth"),
    };

    let image = Image::new(image_data, width, height);

    // test index_from_pos
    // let pos = Pos::new(5, 5);
    // let idx = image.index_from_pos(&pos);
    // println!("pos: {}, idx: {}", pos, idx);

    // 1. What is "Black"
    //
    // Assume area around the step wedge represents max black for the print.
    // Take a sample of those pixels. This is our "black point".
    //
    // Is it valuable to know the range of expected blacks so that I can ask
    // questions like "Is this within 1 std deviation of black"?
    //
    // 2: What is "White"
    //
    // Assume the whitest square is the first square. First job is to find
    // the first square.
    //
    // The first square should be the first "substantial" patch of low values
    // found when search from left to right and top to bottom.
    //
    // It should be found in the first 15% of the image width.
    if let Some(max_black) = image.max_black() {
        if max_black < u16::MAX / 2 {
            panic!("Max black isn't black enough");
        }
        println!("Max Black: {}", max_black);
        // Find first not black
        //
        // Scan the first 15% of the left hand side of the image starting at the top and moving to the
        // bottom until a value is reached that is less than 85% of the max black.
        if let Some((pos, pxl)) = find_not_black(&image, max_black / 8) {
            println!("First not black: {}: {}", pos, pxl);
        };
    }

    // sample at known white area
    let sample = image.sample_at(Pos::new(150, 50), 10, 10);
    if let Some(mean_value) = sample.mean_value() {
        println!("mean white: {}", mean_value);
    }
}

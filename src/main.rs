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

struct Image {
    pixels: Vec<u16>,
    width: u32,
    height: u32,
}

impl Image {
    fn new(pixels: Vec<u16>, width: u32, height: u32) -> Image {
        return Image {
            pixels,
            width,
            height,
        };
    }

    fn from_pos(&self, pos: &Pos) -> usize {
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

    fn find_max_black(&self) -> u16 {
        Sample::new_from_image(self, Pos::new(10, 10), 10, 10).average_value()
    }
}

struct Sample {
    pixels: Vec<u16>,
    average_value: Option<u16>,
}

impl Sample {
    fn new(pixels: Vec<u16>) -> Sample {
        Sample {
            pixels,
            average_value: None,
        }
    }

    fn new_from_image(image: &Image, pos: Pos, width: u32, height: u32) -> Sample {
        let mut pixels = Vec::new();

        for y in 1..pos.y + height {
            for x in 1..pos.x + width {
                let idx = image.from_pos(&Pos::new(x, y));
                pixels.push(image.pixels[idx]);
            }
        }

        Self::new(pixels)
    }

    fn average_value(&mut self) -> u16 {
        if let Some(average_value) = self.average_value {
            return average_value;
        }

        let total: u32 = self.pixels.clone().into_iter().map(u32::from).sum::<u32>();
        let av = (total / self.pixels.len() as u32) as u16;

        self.average_value = Some(av);
        return av;
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

    let max_value = 2 ^ 16;

    let max_black = image.find_max_black();
    if max_black < max_value / 2 {
        panic!("Max black isn't black enough");
    }
    println!("Max Black: {}", max_black);
}

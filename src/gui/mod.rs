use eframe::egui;
use egui::epaint;
use egui::widgets;
use egui::widgets::Widget;
use egui_extras;
use image;
use image::DynamicImage;
use std::path::PathBuf;

pub fn start(debug: bool) {
    println!("starting gui");
    println!("Debug: {}", debug);

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Curved",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(CurvedApp::new(cc)))
        }),
    )
    .unwrap();
}

struct ImageState {
    path: PathBuf,
    image: DynamicImage,
    preview: TextureBufferedImage,
}

enum ImageLoaded {
    Waiting,
    Analyzed,
}

#[derive(Default)]
enum AppState {
    #[default]
    Waiting,
    ImageLoaded,
}

#[derive(Default)]
struct CurvedApp {
    state: AppState,
    image: Option<ImageState>,
}

impl CurvedApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self::default()
    }
}

fn imagebuffer_to_colorimage(image: &DynamicImage) -> epaint::ColorImage {
    let buffer = image.to_rgba8();
    let pixels = buffer.as_flat_samples();

    epaint::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        pixels.as_slice(),
    )
}

struct TextureBufferedImage {
    texture: Option<egui::TextureHandle>,
    color_image: epaint::ColorImage,
    handle: String,
}

impl TextureBufferedImage {
    fn new(handle: String, image_data: &DynamicImage) -> Self {
        let color_image = imagebuffer_to_colorimage(image_data);
        Self {
            texture: None,
            handle,
            color_image,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let handle = self.handle.clone();
        let ci = &self.color_image;

        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            ui.ctx()
                .load_texture(handle, ci.clone(), Default::default())
        });

        widgets::Image::new((texture.id(), texture.size_vec2()))
            .shrink_to_fit()
            .ui(ui);
    }
}

impl eframe::App for CurvedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| match self.state {
            AppState::Waiting => {
                if ui.button("Open file…").clicked() {
                    if let Some(file) = rfd::FileDialog::new().pick_file() {
                        let path = PathBuf::from(file.display().to_string());
                        let image = image::open(&path).unwrap();
                        let preview = TextureBufferedImage::new(
                            path.clone().into_os_string().into_string().unwrap(),
                            &image,
                        );

                        self.image = Some(ImageState {
                            path: path.clone(),
                            image,
                            preview,
                        });
                        self.state = AppState::ImageLoaded;
                    }
                }
            }
            AppState::ImageLoaded => {
                if ui.button("analyze").clicked() {}

                if let Some(image) = &mut self.image {
                    ui.horizontal(|ui| {
                        ui.label("Picked file:");
                        ui.monospace(image.path.to_string_lossy());
                    });
                    image.preview.ui(ui)
                }
            }
        });
    }
}

use egui::epaint;
use egui::widgets;
use egui::widgets::Widget;

use eframe::egui;
use image::DynamicImage;

pub struct TextureBufferedImage {
    texture: Option<egui::TextureHandle>,
    color_image: epaint::ColorImage,
    handle: String,
}

/* TextureBufferedImage is a container for a texture and handle so that we can easily render it.
 * Normally you would use something like egui::image or the image widget except that since
 * we want to use our image.rs processes we want to own the original image bytes and simply
 * present egui with some data to render.
 */
impl TextureBufferedImage {
    pub fn new(handle: String, image: &DynamicImage) -> Self {
        // Our firs step is to convert from that image.rs dynamic image into a egui color image so
        // that we can send it to the gpu to get a texture handle
        let buffer = image.to_rgba8();
        let pixels = buffer.as_flat_samples();

        let color_image = epaint::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            pixels.as_slice(),
        );

        Self {
            texture: None,
            handle,
            color_image,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let handle = self.handle.clone();
        let ci = &self.color_image;

        // Careful! This isn't safe to run in immediate mode this generates the texture
        // and memoizes it
        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            ui.ctx()
                .load_texture(handle, ci.clone(), Default::default())
        });

        widgets::Image::new((texture.id(), texture.size_vec2()))
            .shrink_to_fit()
            .ui(ui);
    }
}

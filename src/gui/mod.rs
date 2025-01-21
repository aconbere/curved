use std::fs;
use std::path::PathBuf;

use eframe::egui;
use image;
use image::DynamicImage;
use splines::Spline;

use super::analyze;
use super::apply;
use super::generate;

mod texture_buffered_image;

use texture_buffered_image::TextureBufferedImage;

struct ImageState {
    path: PathBuf,
    image: DynamicImage,
}

enum AnalyzeStates {
    WaitingForImage,
    ImageLoaded,
    Analyzed(analyze::AnalyzeResults),
}

struct ApplyState {
    curve: Option<Spline<f64, f64>>,
    image: Option<DynamicImage>,
    curved_image: Option<DynamicImage>,
}

struct GenerateState {
    process: String,
    notes: String,
    image: Option<DynamicImage>,
}

#[derive(Default)]
enum AppStates {
    #[default]
    Home,
    Analyze(AnalyzeStates),
    Generate(GenerateState),
    Apply(ApplyState),
}

#[derive(Default)]
struct CurvedApp {
    state: AppStates,
    debug: bool,
    image_state: Option<ImageState>,

    generate_texture: Option<TextureBufferedImage>,
    apply_texture: Option<TextureBufferedImage>,
    analyze_texture: Option<TextureBufferedImage>,
}

impl CurvedApp {
    fn new(_cc: &eframe::CreationContext<'_>, debug: bool) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self {
            debug,
            ..Self::default()
        }
    }
}

/* Notes:
 *
 * Store user local state in $XDG_DATA_HOME or $HOME/.local/state
 *
 * Maybe a sqlite database with previously stored curves? Could store them with date, process used,
 * maybe a snapshot of the scan?
 *
 * Maybe store the prevoius state of the app there so restarts are nicer?
 */

pub fn start(debug: bool) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Curved",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(CurvedApp::new(cc, debug)))
        }),
    )
    .unwrap();
}

fn app_state_to_string(state: &AppStates) -> String {
    match state {
        AppStates::Home => "generate-or-analyze".to_string(),
        AppStates::Apply(_) => "apply".to_string(),
        AppStates::Generate(_) => "generate".to_string(),
        AppStates::Analyze(_) => "analyze".to_string(),
    }
}

fn preview_area(ui: &mut egui::Ui, texture: &mut Option<TextureBufferedImage>) {
    egui::Frame::none().fill(egui::Color32::RED).show(ui, |ui| {
        ui.set_min_width(500.0);
        ui.set_min_height(500.0);
        ui.vertical(|ui| {
            if let Some(preview) = texture {
                preview.ui(ui);
            }
        });
    });
}

fn tab_bar(ui: &mut egui::Ui, app: &mut CurvedApp) {
    let app_state_tab = app_state_to_string(&app.state);

    let buttons = vec![
        (
            "generate-or-analyze",
            egui::Button::new("home"),
            AppStates::Home,
        ),
        (
            "generate",
            egui::Button::new("generate"),
            AppStates::Generate(GenerateState {
                process: "".to_string(),
                notes: "".to_string(),
                image: None,
            }),
        ),
        (
            "analyze",
            egui::Button::new("analyze"),
            AppStates::Analyze(AnalyzeStates::WaitingForImage),
        ),
        (
            "apply",
            egui::Button::new("apply"),
            AppStates::Apply(ApplyState {
                curve: None,
                curved_image: None,
                image: None,
            }),
        ),
    ];

    ui.horizontal(|ui| {
        for (name, button, new_state) in buttons {
            let e = name != app_state_tab;
            if ui.add_enabled(e, button).clicked() {
                app.state = new_state;
            };
        }
    });
}

fn home_page(ui: &mut egui::Ui, app: &mut CurvedApp) {
    ui.heading("Curved");
    ui.separator();
    ui.add_space(12.0);
    ui.label(
        "Curved is a tool to help you build tone adjustment curves for analog printing. To use it \
         first generate a step wedge. Then go print the step wedge however you like. Scan the \
         results and use the Analyze function to produce a tone adjustment curve. Finally you can \
         apply the curve to an image with the Apply tool.",
    );
    ui.add_space(12.0);
    ui.heading("Generate Step Wedge");
    ui.separator();
    ui.label("Use this tool to build a fresh step wedge for your testing.");
    if ui.button("Generate").clicked() {
        app.state = AppStates::Generate(GenerateState {
            process: "".to_string(),
            notes: "".to_string(),
            image: None,
        });
    }
    ui.add_space(12.0);

    ui.heading("Analyze Scan");
    ui.separator();
    ui.label("Use this tool build a tone adjustment curve from the scan of your print.");
    if ui.button("Analyze").clicked() {
        app.state = AppStates::Analyze(AnalyzeStates::WaitingForImage);
    }
    ui.add_space(12.0);

    ui.heading("Apply Curve");
    ui.separator();
    ui.label("Use this tool apply a curve to an image you want to print.");
    if ui.button("Apply").clicked() {
        app.state = AppStates::Apply(ApplyState {
            curve: None,
            curved_image: None,
            image: None,
        });
    }
}

fn generate_page(ui: &mut egui::Ui, app: &mut CurvedApp) {
    if let AppStates::Generate(ref mut generate_state) = app.state {
        let mut process = generate_state.process.clone();
        let mut notes = generate_state.notes.clone();

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width(300.0);
                ui.label(
                    "Generates a step wedge for printing as a transparency. Output will be a \
                     16bit greyscale image. It is generated in its \"inverted\" form and does not \
                     need to be further inverted before printing. A 300 dpi print should be about \
                     5\"x5.25\".",
                );

                let process_label = ui.label("Process: ");
                ui.text_edit_singleline(&mut process)
                    .labelled_by(process_label.id);

                let notes_label = ui.label("Notes: ");
                ui.text_edit_singleline(&mut notes)
                    .labelled_by(notes_label.id);

                if ui.button("Generate").clicked() {
                    let image = generate::generate(app.debug);
                    let preview = TextureBufferedImage::new(
                        format!(
                            "generated_step_wedge_{}_{}",
                            generate_state.process, generate_state.notes
                        ),
                        &image,
                    );
                    generate_state.image = Some(image);
                    app.generate_texture = Some(preview);
                }
                let has_image = generate_state.image.is_some();
                if ui
                    .add_enabled(has_image, egui::Button::new("Save"))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        if let Some(image) = &generate_state.image {
                            image.save(path).unwrap();
                        }
                    }
                }
            });

            preview_area(ui, &mut app.generate_texture);

            generate_state.process = process.to_string();
            generate_state.notes = notes.to_string();
        });
    }
}

fn apply_page(ui: &mut egui::Ui, app: &mut CurvedApp) {
    if let AppStates::Apply(ref mut apply_state) = app.state {
        if ui.button("Select Image").clicked() {
            if let Some(image_file) = rfd::FileDialog::new().pick_file() {
                let image = image::open(&image_file).unwrap();
                let preview = TextureBufferedImage::new(
                    image_file.clone().into_os_string().into_string().unwrap(),
                    &image,
                );
                apply_state.image = Some(image);
                app.apply_texture = Some(preview);
            }
        };

        if ui.button("select curve").clicked() {
            if let Some(curve_file) = rfd::FileDialog::new().pick_file() {
                let curve_data = fs::read_to_string(curve_file).unwrap();
                let curve = serde_json::from_str::<Spline<f64, f64>>(&curve_data).unwrap();
                apply_state.curve = Some(curve);
            }
        };

        if let Some(preview) = &mut app.apply_texture {
            preview.ui(ui);
        }

        if let (Some(image), Some(curve)) = (&apply_state.image, &apply_state.curve) {
            if ui.button("Apply").clicked() {
                let curved_image = apply::apply(&image, &curve);

                let preview =
                    TextureBufferedImage::new("curved_image_preview".to_string(), &curved_image);
                app.apply_texture = Some(preview);
            }
        }

        if let Some(curved_image) = &apply_state.curved_image {
            if ui.button("Save").clicked() {
                if let Some(path) = rfd::FileDialog::new().save_file() {
                    curved_image.save(path).unwrap();
                }
            }
        }
    }
}

fn analyze_page(ui: &mut egui::Ui, app: &mut CurvedApp) {
    ui.label(
        "This tool will evaluate a scan of the generated step wedge to generate a new curve file.",
    );

    // Draw the side bar
    //

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            if ui.button("Select Scan").clicked() {
                if let Some(file) = rfd::FileDialog::new().pick_file() {
                    let path = PathBuf::from(file.display().to_string());
                    let image = image::open(&path).unwrap();

                    let preview = TextureBufferedImage::new(
                        path.clone().into_os_string().into_string().unwrap(),
                        &image,
                    );

                    app.image_state = Some(ImageState {
                        path: path.clone(),
                        image,
                    });
                    app.analyze_texture = Some(preview);

                    app.state = AppStates::Analyze(AnalyzeStates::ImageLoaded);
                }
            }

            if let Some(image_state) = &app.image_state {
                if ui.add_enabled(true, egui::Button::new("analyze")).clicked() {
                    let analyze_results = analyze::analyze(&image_state.image, app.debug).unwrap();

                    app.state = AppStates::Analyze(AnalyzeStates::Analyzed(analyze_results));
                }
            } else {
                ui.add_enabled(false, egui::Button::new("analyze"));
            }
        });

        ui.vertical(|ui| {
            preview_area(ui, &mut app.analyze_texture);
            if let Some(image_state) = &app.image_state {
                let path: String = image_state.path.to_string_lossy().to_string();
                ui.monospace(path);
            }

            if let AppStates::Analyze(AnalyzeStates::Analyzed(analyze_results)) = &app.state {
                if ui.button("save curve").clicked() {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        let curve_file = fs::File::create(path).unwrap();
                        serde_json::to_writer(&curve_file, &analyze_results.curve).unwrap();
                    }
                }
            }
        });
    });
}

impl eframe::App for CurvedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            tab_bar(ui, self);
            ui.separator();
            ui.add_space(12.0);
            match &mut self.state {
                AppStates::Home => {
                    home_page(ui, self);
                }
                AppStates::Generate(_) => {
                    generate_page(ui, self);
                }
                AppStates::Apply(_) => {
                    apply_page(ui, self);
                }
                AppStates::Analyze(_) => {
                    analyze_page(ui, self);
                }
            }
        });
    }
}

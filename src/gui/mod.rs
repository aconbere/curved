use regex;
use std::fs;
use std::path::PathBuf;

use anyhow;
use eframe::egui;
use egui::{Color32, RichText};
use image;
use image::DynamicImage;
use rfd;
use splines::Spline;

use super::analyze;
use super::apply;
use super::generate;

mod texture_buffered_image;

use texture_buffered_image::TextureBufferedImage;

struct PreviewedImage {
    path: PathBuf,
    image: DynamicImage,
    preview: TextureBufferedImage,
}

#[derive(Default)]
struct ApplyPageState {
    curve: Option<Spline<f64, f64>>,
    image: Option<PreviewedImage>,
    curved_image: Option<PreviewedImage>,
}

#[derive(Default)]
struct GeneratePageState {
    process: String,
    notes: String,
    image: Option<PreviewedImage>,
}

#[derive(Default, PartialEq)]
enum AnalyzePreviewTab {
    #[default]
    Scan,
    Results,
    Normalized,
}

struct AnalyzePageState {
    scan: Option<PreviewedImage>,
    analysis: Option<analyze::AnalyzeResults>,
    analysis_preview: Option<TextureBufferedImage>,
    normalized_preview: Option<TextureBufferedImage>,
    preview_tab: AnalyzePreviewTab,
    inverted: bool,
}

impl Default for AnalyzePageState {
    fn default() -> Self {
        Self {
            scan: None,
            analysis: None,
            analysis_preview: None,
            normalized_preview: None,
            preview_tab: AnalyzePreviewTab::default(),
            inverted: false,
        }
    }
}

#[derive(Default, PartialEq)]
enum Page {
    #[default]
    Generate,
    Analyze,
    Apply,
}

#[derive(Default)]
struct CurvedApp {
    debug: bool,

    page: Page,
    generate_page_state: GeneratePageState,
    analyze_page_state: AnalyzePageState,
    apply_page_state: ApplyPageState,
}

fn action_button(text: &str) -> egui::Button {
    egui::Button::new(RichText::new(text).color(Color32::from_gray(16)))
        .fill(Color32::from_rgb(255, 143, 0))
}

fn draw_analyze_preview(
    curve: &Spline<f64, f64>,
    histogram: &Vec<u32>,
) -> anyhow::Result<TextureBufferedImage> {
    let mut image: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::new(1024, 1024);
    analyze::draw_histogram(&mut image, &histogram)?;
    analyze::draw_curve(&mut image, &curve)?;
    Ok(TextureBufferedImage::new(
        format!("curve_and_histogram"),
        &DynamicImage::ImageRgb8(image),
    ))
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
        viewport: egui::ViewportBuilder::default().with_inner_size([950.0, 750.0]),
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

fn slugify(s: String) -> anyhow::Result<String> {
    let re = regex::Regex::new("\\s")?;
    let downcased = str::to_lowercase(&s);
    let trimmed = str::trim(&downcased);
    let no_white_space = re.replace_all(&trimmed, "-");
    Ok(no_white_space.to_string())
}

fn tab_bar(ui: &mut egui::Ui, app: &mut CurvedApp) {
    egui::TopBottomPanel::top("top_tab_bar")
        .resizable(false)
        .min_height(32.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Curved");
                ui.separator();
                ui.selectable_value(&mut app.page, Page::Generate, "Generate");
                ui.selectable_value(&mut app.page, Page::Analyze, "Analyze");
                ui.selectable_value(&mut app.page, Page::Apply, "Apply");
            });
        });
}

fn generate_page(ui: &mut egui::Ui, state: &mut GeneratePageState) {
    let mut process = state.process.clone();
    let mut notes = state.notes.clone();

    egui::SidePanel::left("side_bar")
        .default_width(325.0)
        .show_inside(ui, |ui| {
            ui.add_space(12.0);
            ui.label(
                "Generates a step wedge for printing as a transparency. Output will be a 16bit \
                 greyscale image. It is generated in its \"inverted\" form and does not need to \
                 be further inverted before printing. A 300 dpi print should be about 5\"x5.25\".",
            );

            ui.separator();
            ui.add_space(12.0);

            let process_label = ui.label("Process: ");
            ui.text_edit_singleline(&mut process)
                .labelled_by(process_label.id);
            ui.add_space(12.0);

            let notes_label = ui.label("Notes: ");
            ui.text_edit_singleline(&mut notes)
                .labelled_by(notes_label.id);
            ui.add_space(12.0);

            if ui.button("Generate").clicked() {
                let no = if notes == "" {
                    None
                } else {
                    Some(notes.clone())
                };
                let pr = if process == "" {
                    None
                } else {
                    Some(process.clone())
                };
                let image = generate::generate(no, pr).unwrap();
                let preview = TextureBufferedImage::new(
                    format!("generated_step_wedge_{}_{}", state.process, state.notes),
                    &image,
                );
                state.image = Some(PreviewedImage {
                    path: PathBuf::new(),
                    image,
                    preview,
                });
            }
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::TopBottomPanel::bottom("image_commands")
            .min_height(32.0)
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(image) = &state.image {
                        let filename = if state.process == "" {
                            "step-wedge.png".to_string()
                        } else {
                            format!("{}-step-wedge.png", slugify(state.process.clone()).unwrap())
                        };
                        if ui.add(action_button("Save")).clicked() {
                            if let Some(path) =
                                rfd::FileDialog::new().set_file_name(filename).save_file()
                            {
                                image.image.save(path).unwrap();
                            }
                        };
                    } else {
                        ui.add_enabled(false, egui::Button::new("Save"));
                    }
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(image) = &mut state.image {
                image.preview.ui(ui);
            }
        });
    });

    state.process = process.to_string();
    state.notes = notes.to_string();
}

fn apply_page(ui: &mut egui::Ui, state: &mut ApplyPageState) {
    egui::SidePanel::left("side_bar")
        .min_width(325.0)
        .show_inside(ui, |ui| {
            ui.add_space(12.0);
            ui.label("Apply a given curve to an image for printing.");
            ui.add_space(12.0);
            if ui.button("Select Image").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let image = image::open(&path).unwrap();
                    let preview = TextureBufferedImage::new(
                        path.clone().into_os_string().into_string().unwrap(),
                        &image,
                    );
                    state.image = Some(PreviewedImage {
                        path,
                        image,
                        preview,
                    });
                }
            };
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::TopBottomPanel::bottom("controls")
            .min_height(32.0)
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if state.curved_image.is_some() {
                        ui.horizontal(|ui| {
                            if state.curved_image.is_some() {
                                if ui.add(action_button("Undo")).clicked() {
                                    state.curved_image = None;
                                }
                            }
                            if let Some(ci) = &state.curved_image {
                                if ui.add(action_button("Save")).clicked() {
                                    if let Some(path) = rfd::FileDialog::new().save_file() {
                                        ci.image.save(path).unwrap();
                                    }
                                }
                            }
                        });
                    } else if let Some(image) = &state.image {
                        if ui.add(action_button("Apply Curve")).clicked() {
                            if let Some(curve_file) = rfd::FileDialog::new().pick_file() {
                                let curve_data = fs::read_to_string(curve_file).unwrap();
                                let curve =
                                    serde_json::from_str::<Spline<f64, f64>>(&curve_data).unwrap();
                                let curved_image = apply::apply(&image.image, &curve);
                                state.curve = Some(curve);

                                let preview = TextureBufferedImage::new(
                                    "curved_image_preview".to_string(),
                                    &curved_image,
                                );
                                state.curved_image = Some(PreviewedImage {
                                    path: image.path.clone(),
                                    image: curved_image,
                                    preview,
                                });
                            }
                        };
                    }
                })
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(ci) = &mut state.curved_image {
                ci.preview.ui(ui);
            } else if let Some(image) = &mut state.image {
                image.preview.ui(ui);
            }
        });
    });
}

fn analyze_page(ui: &mut egui::Ui, state: &mut AnalyzePageState, debug: bool) {
    egui::SidePanel::left("side_bar")
        .min_width(325.0)
        .show_inside(ui, |ui| {
            ui.add_space(12.0);
            ui.label(
                "Analyze evaluates a scan of the generated step wedge to generate a new curve \
                 file.",
            );
            ui.separator();
            ui.add_space(12.0);
            if ui.button("Select Scan").clicked() {
                if let Some(file) = rfd::FileDialog::new().pick_file() {
                    let path = PathBuf::from(file.display().to_string());
                    let image = image::open(&path).unwrap();

                    let preview = TextureBufferedImage::new(
                        path.clone().into_os_string().into_string().unwrap(),
                        &image,
                    );

                    state.scan = Some(PreviewedImage {
                        path: path.clone(),
                        image,
                        preview,
                    });
                }
            }
            if let Some(scan) = &mut state.scan {
                if ui.button("left").clicked() {
                    scan.image = scan.image.rotate270();
                    scan.preview =
                        TextureBufferedImage::new(format!("image_rotated_270"), &scan.image)
                };
                if ui.button("right").clicked() {
                    scan.image = scan.image.rotate90();
                    scan.preview =
                        TextureBufferedImage::new(format!("image_rotated_90"), &scan.image)
                };
                if state.inverted {
                    if ui.button("uninvert").clicked() {
                        state.inverted = false
                    };
                } else {
                    if ui.button("invert").clicked() {
                        state.inverted = true
                    };
                }
            }
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        egui::TopBottomPanel::top("analyze_tabs")
            .min_height(32.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.preview_tab, AnalyzePreviewTab::Scan, "Scan");
                    ui.selectable_value(
                        &mut state.preview_tab,
                        AnalyzePreviewTab::Results,
                        "Results",
                    );
                    ui.selectable_value(
                        &mut state.preview_tab,
                        AnalyzePreviewTab::Normalized,
                        "Normalized",
                    );
                });
            });
        egui::TopBottomPanel::bottom("actions")
            .min_height(32.0)
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match state.preview_tab {
                        AnalyzePreviewTab::Scan => {
                            if let Some(scan) = &state.scan {
                                if ui.add_enabled(true, action_button("Analyze")).clicked() {
                                    let analyze_results =
                                        analyze::analyze(&scan.image, state.inverted, debug)
                                            .unwrap();
                                    state.analysis_preview = Some(
                                        draw_analyze_preview(
                                            &analyze_results.curve,
                                            &analyze_results.histogram,
                                        )
                                        .unwrap(),
                                    );
                                    state.normalized_preview = Some(TextureBufferedImage::new(
                                        "normalized_image".to_string(),
                                        &analyze_results.normalized_image,
                                    ));
                                    state.analysis = Some(analyze_results);
                                    state.preview_tab = AnalyzePreviewTab::Results;
                                }
                            } else {
                                ui.add_enabled(false, action_button("Analyze"));
                            }
                            if let Some(scan) = &mut state.scan {
                                let path: String = scan.path.to_string_lossy().to_string();
                                ui.monospace(path);
                            }
                        }
                        AnalyzePreviewTab::Results => {
                            if let Some(analysis) = &state.analysis {
                                if ui.add(action_button("Save JSON")).clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name("curve.json")
                                        .save_file()
                                    {
                                        let curve_file = fs::File::create(path).unwrap();
                                        serde_json::to_writer(&curve_file, &analysis.curve)
                                            .unwrap();
                                    }
                                };
                                if ui.add(action_button("Save CSV")).clicked() {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name("curve.csv")
                                        .save_file()
                                    {
                                        let mut csv_file = fs::File::create(path).unwrap();
                                        analyze::write_small_csv(&mut csv_file, &analysis.curve)
                                            .unwrap();
                                    }
                                };
                            } else {
                                let _ = ui.button("Save");
                            }
                        }
                        AnalyzePreviewTab::Normalized => {}
                    };
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| match state.preview_tab {
            AnalyzePreviewTab::Scan => {
                if let Some(scan) = &mut state.scan {
                    scan.preview.ui(ui);
                }
            }
            AnalyzePreviewTab::Results => {
                if let Some(preview) = &mut state.analysis_preview {
                    preview.ui(ui);
                }
            }
            AnalyzePreviewTab::Normalized => {
                if let Some(preview) = &mut state.normalized_preview {
                    preview.ui(ui);
                }
            }
        });
    });
}

impl eframe::App for CurvedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            tab_bar(ui, self);
            match &mut self.page {
                Page::Generate => {
                    generate_page(ui, &mut self.generate_page_state);
                }
                Page::Apply => {
                    apply_page(ui, &mut self.apply_page_state);
                }
                Page::Analyze => {
                    analyze_page(ui, &mut self.analyze_page_state, self.debug);
                }
            }
        });
    }
}

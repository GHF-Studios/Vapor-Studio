use clap::Parser;
use eframe::egui;
use std::path::PathBuf;

mod studio;

#[derive(Debug, Parser)]
#[command(name = "vapor-studio", about = "Vapor Studio and composition launcher")]
struct Args {
    /// Open a specific Vapor workspace.
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Build and launch the selected profile's composition immediately.
    #[arg(long)]
    play: bool,
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Vapor Studio")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Vapor Studio",
        options,
        Box::new(move |creation| {
            Ok(Box::new(studio::StudioApp::new(
                creation,
                args.workspace,
                args.play,
            )))
        }),
    )
}

use eframe::egui;

// Include SVG icons using the new egui::include_image! macro
pub static CONNECT_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/connect.svg");
pub static DISCONNECT_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/disconnect.svg");
pub static ADD_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/add.svg");
pub static TABLE_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/table.svg");
pub static DATA_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/data.svg");
pub static SCHEMA_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/schema.svg");
pub static CLOSE_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/close.svg");
pub static PLAY_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/play.svg");
pub static QUERY_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/query.svg");
pub static ERROR_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/error.svg");
pub static SAVE_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/save.svg");
pub static EDIT_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/edit.svg");
pub static DELETE_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/delete.svg");
pub static TEST_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/test.svg");
pub static INFO_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/info.svg");
pub static CANCEL_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/cancel.svg");
pub static RESULTS_ICON: egui::ImageSource<'static> = egui::include_image!("../../assets/icons/results.svg");

// Helper function to create icon buttons using egui's built-in image_and_text method
pub fn icon_button(ui: &mut egui::Ui, icon_source: &egui::ImageSource<'static>, text: &str) -> egui::Response {
    let icon_image = egui::Image::new(icon_source.clone())
        .max_size(egui::Vec2::new(16.0, 16.0))
        .tint(ui.visuals().text_color()); // Tint with theme's text color for proper contrast
    
    ui.add(egui::Button::image_and_text(icon_image, text))
}

// Helper function to create small icon buttons
pub fn small_icon_button(ui: &mut egui::Ui, icon_source: &egui::ImageSource<'static>, text: &str) -> egui::Response {
    let icon_image = egui::Image::new(icon_source.clone())
        .max_size(egui::Vec2::new(14.0, 14.0))
        .tint(ui.visuals().text_color());
    
    ui.add(egui::Button::image_and_text(icon_image, text).small())
}

// Helper function to create icon-only buttons
pub fn icon_only_button(ui: &mut egui::Ui, icon_source: &egui::ImageSource<'static>, tooltip: &str) -> egui::Response {
    let icon_image = egui::Image::new(icon_source.clone())
        .max_size(egui::Vec2::new(16.0, 16.0))
        .tint(ui.visuals().text_color());
    
    ui.add(egui::ImageButton::new(icon_image))
        .on_hover_text(tooltip)
} 
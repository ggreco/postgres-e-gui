use eframe::egui;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// Global icon cache - thread-safe singleton using pointer addresses as keys
static ICON_CACHE: OnceLock<Mutex<HashMap<usize, egui::Image<'static>>>> = OnceLock::new();

// Initialize the icon cache
fn get_icon_cache() -> &'static Mutex<HashMap<usize, egui::Image<'static>>> {
    ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// Include PNG icons (32px for better quality and scalability)
pub const CONNECT_ICON: &[u8] = include_bytes!("../../assets/icons/connect.png");
pub const DISCONNECT_ICON: &[u8] = include_bytes!("../../assets/icons/disconnect.png");
pub const ADD_ICON: &[u8] = include_bytes!("../../assets/icons/add.png");
pub const TABLE_ICON: &[u8] = include_bytes!("../../assets/icons/table.png");
pub const DATA_ICON: &[u8] = include_bytes!("../../assets/icons/data.png");
pub const SCHEMA_ICON: &[u8] = include_bytes!("../../assets/icons/schema.png");
pub const CLOSE_ICON: &[u8] = include_bytes!("../../assets/icons/close.png");
pub const PLAY_ICON: &[u8] = include_bytes!("../../assets/icons/play.png");
pub const QUERY_ICON: &[u8] = include_bytes!("../../assets/icons/query.png");
pub const ERROR_ICON: &[u8] = include_bytes!("../../assets/icons/error.png");
pub const SAVE_ICON: &[u8] = include_bytes!("../../assets/icons/save.png");
pub const EDIT_ICON: &[u8] = include_bytes!("../../assets/icons/edit.png");
pub const DELETE_ICON: &[u8] = include_bytes!("../../assets/icons/delete.png");
pub const TEST_ICON: &[u8] = include_bytes!("../../assets/icons/test.png");
pub const INFO_ICON: &[u8] = include_bytes!("../../assets/icons/info.png");
pub const CANCEL_ICON: &[u8] = include_bytes!("../../assets/icons/cancel.png");
pub const RESULTS_ICON: &[u8] = include_bytes!("../../assets/icons/results.png");

// Get or create cached icon using pointer address as key
fn get_cached_icon(icon_bytes: &'static [u8]) -> egui::Image<'static> {
    let cache_key = icon_bytes.as_ptr() as usize;
    let cache = get_icon_cache();
    let mut cache_guard = cache.lock().unwrap();

    if let Some(cached_icon) = cache_guard.get(&cache_key) {
        // Return cached icon
        cached_icon.clone()
    } else {
        // Create new icon and cache it using a unique identifier
        let unique_id = format!("icon_{:p}", icon_bytes.as_ptr());
        let icon_image = egui::Image::from_bytes(unique_id, icon_bytes);
        cache_guard.insert(cache_key, icon_image.clone());
        icon_image
    }
}

// Helper function to create icon buttons using egui's built-in image_and_text method
pub fn icon_button(ui: &mut egui::Ui, icon_bytes: &'static [u8], text: &str) -> egui::Response {
    let icon_image = get_cached_icon(icon_bytes)
        .max_size(egui::Vec2::new(16.0, 16.0))
        .tint(ui.visuals().text_color()); // Tint with theme's text color for proper contrast
    
    ui.add(egui::Button::image_and_text(icon_image, text))
}

// Helper function to create small icon buttons
pub fn small_icon_button(ui: &mut egui::Ui, icon_bytes: &'static [u8], text: &str) -> egui::Response {
    let icon_image = get_cached_icon(icon_bytes)
        .max_size(egui::Vec2::new(14.0, 14.0))
        .tint(ui.visuals().text_color());
    
    ui.add(egui::Button::image_and_text(icon_image, text).small())
}

// Helper function to create icon-only buttons
pub fn icon_only_button(ui: &mut egui::Ui, icon_bytes: &'static [u8], tooltip: &str) -> egui::Response {
    let icon_image = get_cached_icon(icon_bytes)
        .max_size(egui::Vec2::new(16.0, 16.0))
        .tint(ui.visuals().text_color());
    
    ui.add(egui::ImageButton::new(icon_image))
        .on_hover_text(tooltip)
} 
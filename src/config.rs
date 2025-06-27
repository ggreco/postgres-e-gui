use crate::models::DatabaseConnection;

pub fn load_connections() -> Vec<DatabaseConnection> {
    if let Some(config_dir) = dirs::config_dir() {
        let config_file = config_dir.join("postgres-gui").join("connections.json");
        if let Ok(content) = std::fs::read_to_string(config_file) {
            if let Ok(connections) = serde_json::from_str(&content) {
                return connections;
            }
        }
    }
    Vec::new()
}

pub fn save_connections(connections: &[DatabaseConnection]) {
    if let Some(config_dir) = dirs::config_dir() {
        let config_dir = config_dir.join("postgres-gui");
        let _ = std::fs::create_dir_all(&config_dir);
        let config_file = config_dir.join("connections.json");
        if let Ok(content) = serde_json::to_string_pretty(connections) {
            let _ = std::fs::write(config_file, content);
        }
    }
} 
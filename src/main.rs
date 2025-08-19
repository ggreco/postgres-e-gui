use eframe::egui;
use tokio::sync::mpsc;
use egui_extras::install_image_loaders;

mod models;
mod config;
mod database;
mod ui;

use crate::models::{PostgresGuiApp, DatabaseResponse, Tab, TabContent};
use crate::ui::icons::*;

impl Default for PostgresGuiApp {
    fn default() -> Self {
        let (_response_sender, response_receiver) = mpsc::unbounded_channel();
        
        Self {
            connections: config::load_connections(),
            show_connection_wizard: false,
            connection_wizard: models::ConnectionWizard::default(),
            current_connection: None,
            tables: Vec::new(),
            tabs: Vec::new(),
            active_tab_index: None,
            loading: false,
            error_message: None,
            db_sender: None,
            db_receiver: response_receiver,
        }
    }
}

impl PostgresGuiApp {


    fn save_connections(&self) {
        config::save_connections(&self.connections);
    }

    fn start_database_worker(&mut self, ctx: &egui::Context) {
        let (db_sender, db_receiver) = mpsc::unbounded_channel();
        let (response_sender, response_receiver) = mpsc::unbounded_channel();
        
        self.db_sender = Some(db_sender);
        self.db_receiver = response_receiver;
        
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            database::start_database_worker(db_receiver, response_sender, ctx_clone).await;
        });
    }

    fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        self.active_tab_index = Some(self.tabs.len() - 1);
    }

    fn close_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            
            if self.tabs.is_empty() {
                self.active_tab_index = None;
            } else if let Some(active) = self.active_tab_index {
                if active >= index && active > 0 {
                    self.active_tab_index = Some(active - 1);
                } else if active >= self.tabs.len() {
                    self.active_tab_index = Some(self.tabs.len() - 1);
                }
            }
        }
    }

    fn handle_database_responses(&mut self) {
        while let Ok(response) = self.db_receiver.try_recv() {
            match response {
                DatabaseResponse::Connected(tables) => {
                    self.tables = tables;
                    self.loading = false;
                }
                DatabaseResponse::TableDataLoaded(tab_id, new_data, new_columns) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let TabContent::TableData { data, columns, loading, .. } = &mut tab.content {
                            *data = new_data;
                            *columns = new_columns;
                            *loading = false;
                        }
                    }
                }
                DatabaseResponse::TableSchemaLoaded(tab_id, schema_columns) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let TabContent::TableSchema { columns, loading, .. } = &mut tab.content {
                            *columns = schema_columns;
                            *loading = false;
                        }
                    }
                }
                DatabaseResponse::QueryResult(tab_id, new_data, new_columns) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let TabContent::Query { results, columns, loading, error, .. } = &mut tab.content {
                            *results = Some(new_data);
                            *columns = new_columns;
                            *loading = false;
                            *error = None;
                        }
                    }
                }
                DatabaseResponse::Error(err) => {
                    self.error_message = Some(err);
                    self.loading = false;
                    
                    // Also update any loading tabs
                    for tab in &mut self.tabs {
                        match &mut tab.content {
                            TabContent::TableData { loading, .. } => *loading = false,
                            TabContent::TableSchema { loading, .. } => *loading = false,
                            TabContent::Query { loading, error, .. } => {
                                *loading = false;
                                *error = Some(self.error_message.clone().unwrap_or_default());
                            }
                        }
                    }
                }
                DatabaseResponse::ConnectionTestResult(success, message) => {
                    self.connection_wizard.testing = false;
                    self.connection_wizard.test_result = Some(if success {
                        format!("✅ {}", message)
                    } else {
                        format!("❌ {}", message)
                    });
                }
                DatabaseResponse::TableRowCount(tab_id, count) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let TabContent::TableData { total_rows, .. } = &mut tab.content {
                            *total_rows = Some(count);
                        }
                    }
                }
            }
        }
    }
}

impl eframe::App for PostgresGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.db_sender.is_none() {
            self.start_database_worker(ctx);
        }
        
        self.handle_database_responses();
        
        // Top panel with connection controls
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("PostgreSQL GUI");
                ui.separator();
                
                if let Some(conn) = &self.current_connection {
                    let conn_name = conn.name.clone();
                    if icon_button(ui, &DISCONNECT_ICON, "Disconnect").clicked() {
                        self.current_connection = None;
                        self.tables.clear();
                        self.tabs.clear();
                        self.active_tab_index = None;
                    }
                    ui.label(format!("Connected to: {}", conn_name));
                } else {
                    if icon_button(ui, &CONNECT_ICON, "Connect").clicked() {
                        self.show_connection_wizard = true;
                    }
                }
                
                if self.loading {
                    ui.spinner();
                }
            });
        });
        
        // Side panel with table browser
        if self.current_connection.is_some() {
            egui::SidePanel::left("table_browser").resizable(true).show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Image::new(TABLE_ICON.clone())
                            .fit_to_exact_size(egui::Vec2::new(22.0, 22.0))
                            .tint(ui.visuals().text_color()));
                        ui.heading("Tables");
                    });
                    ui.separator();
                    
                    let tables = self.tables.clone();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                                                for table in &tables {
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(ui.available_width(), 0.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.group(|ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.vertical(|ui| {
                                            // First line: table name and schema
                                            ui.horizontal(|ui| {
                                                ui.add(egui::Image::new(TABLE_ICON.clone())
                                                    .fit_to_exact_size(egui::Vec2::new(16.0, 16.0))
                                                    .tint(ui.visuals().text_color()));
                                                ui.strong(&table.name);
                                                ui.small(format!("({})", table.schema));
                                            });
                                            
                                            // Second line: buttons
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if small_icon_button(ui, &DATA_ICON, "Data").clicked() {
                                                    let tab_id = uuid::Uuid::new_v4().to_string();
                                                    let tab = Tab {
                                                        id: tab_id.clone(),
                                                        title: table.name.clone(),
                                                        content: TabContent::TableData { 
                                                            table: table.clone(), 
                                                            data: Vec::new(),
                                                            columns: Vec::new(),
                                                            loading: true,
                                                            total_rows: None,
                                                        },
                                                    };
                                                    self.add_tab(tab);
                                                    
                                                    if let Some(sender) = &self.db_sender {
                                                        let _ = sender.send(crate::models::DatabaseMessage::LoadTableData(table.clone(), tab_id.clone()));
                                                        let _ = sender.send(crate::models::DatabaseMessage::GetTableRowCount(table.clone(), tab_id));
                                                    }
                                                }
                                                
                                                if small_icon_button(ui, &SCHEMA_ICON, "Schema").clicked() {
                                                    let tab_id = uuid::Uuid::new_v4().to_string();
                                                    let tab = Tab {
                                                        id: tab_id.clone(),
                                                        title: format!("{} Schema", table.name),
                                                        content: TabContent::TableSchema { 
                                                            table: table.clone(), 
                                                            columns: Vec::new(),
                                                            loading: true,
                                                        },
                                                    };
                                                    self.add_tab(tab);
                                                    
                                                    if let Some(sender) = &self.db_sender {
                                                        let _ = sender.send(crate::models::DatabaseMessage::LoadTableSchema(table.clone(), tab_id));
                                                    }
                                                }
                                            });
                                        });
                                    });
                                },
                            );
                        }
                    });
                });
            });
        }
        
        // Main content area with tabs
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.current_connection.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.heading("Welcome to PostgreSQL GUI");
                    ui.label("Click 'Connect' to get started");
                });
            } else if self.tabs.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("No tabs open");
                        ui.label("Select a table from the sidebar to view its data or schema");
                        ui.separator();
                        
                        if icon_button(ui, &QUERY_ICON, "New Query").clicked() {
                            let tab_id = uuid::Uuid::new_v4().to_string();
                            let tab = Tab {
                                id: tab_id,
                                title: "Query".to_string(),
                                content: TabContent::Query { 
                                    sql: String::new(),
                                    results: None,
                                    columns: Vec::new(),
                                    loading: false,
                                    error: None,
                                },
                            };
                            self.add_tab(tab);
                        }
                    });
                });
            } else {
                // Tab bar
                ui.horizontal(|ui| {
                    let mut tab_to_close = None;
                    
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let is_active = self.active_tab_index == Some(i);
                        
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                let response = ui.selectable_label(is_active, &tab.title);
                                if response.clicked() {
                                    self.active_tab_index = Some(i);
                                }
                                
                                if icon_only_button(ui, &CLOSE_ICON, "Close tab").clicked() {
                                    tab_to_close = Some(i);
                                }
                            });
                        });
                    }
                    
                    ui.separator();
                    
                    if icon_button(ui, &ADD_ICON, "New Query").clicked() {
                        let tab_id = uuid::Uuid::new_v4().to_string();
                        let tab = Tab {
                            id: tab_id,
                            title: "Query".to_string(),
                            content: TabContent::Query { 
                                sql: String::new(),
                                results: None,
                                columns: Vec::new(),
                                loading: false,
                                error: None,
                            },
                        };
                        self.add_tab(tab);
                    }
                    
                    if let Some(index) = tab_to_close {
                        self.close_tab(index);
                    }
                });
                
                ui.separator();
                
                // Tab content
                if let Some(active_index) = self.active_tab_index {
                    if let Some(tab) = self.tabs.get_mut(active_index) {
                        match &mut tab.content {
                            TabContent::TableData { data, columns, loading, total_rows, .. } => {
                                if *loading {
                                    ui.centered_and_justified(|ui| {
                                        ui.spinner();
                                        ui.label("Loading table data...");
                                    });
                                } else {
                                    ui.vertical(|ui| {
                                        // Table data in scroll area (reserve space for status bar)
                                        let available_height = ui.available_height() - 40.0; // Reserve space for status bar
                                        egui::ScrollArea::both()
                                            .max_height(available_height)
                                            .show(ui, |ui| {
                                                egui::Grid::new("data_grid").striped(true).show(ui, |ui| {
                                                    // Header
                                                    for column in columns.iter() {
                                                        ui.strong(column);
                                                    }
                                                    ui.end_row();
                                                    
                                                    // Data rows
                                                    for row in data.iter() {
                                                        for column in columns.iter() {
                                                            let null_string = "NULL".to_string();
                                                            let value = row.get(column).unwrap_or(&null_string);
                                                            ui.label(value);
                                                        }
                                                        ui.end_row();
                                                    }
                                                });
                                            });
                                        
                                        // Status bar with row count (below the table)
                                        ui.separator();
                                        ui.horizontal(|ui| {
                                            ui.add(egui::Image::new(DATA_ICON.clone())
                                                .fit_to_exact_size(egui::Vec2::new(16.0, 16.0))
                                                .tint(ui.visuals().text_color()));
                                            ui.label(format!("Showing {} rows", data.len()));
                                            if let Some(total) = total_rows {
                                                if *total != data.len() as i64 {
                                                    ui.label(format!("of {} total rows", total));
                                                }
                                            }
                                        });
                                    });
                                }
                            }
                            TabContent::TableSchema { columns, loading, .. } => {
                                if *loading {
                                    ui.centered_and_justified(|ui| {
                                        ui.spinner();
                                        ui.label("Loading table schema...");
                                    });
                                } else {
                                    egui::ScrollArea::vertical().show(ui, |ui| {
                                        egui::Grid::new("schema_grid").striped(true).show(ui, |ui| {
                                            // Header
                                            ui.strong("Column");
                                            ui.strong("Type");
                                            ui.strong("Nullable");
                                            ui.strong("Default");
                                            ui.end_row();
                                            
                                            // Schema rows
                                            for column in columns.iter() {
                                                ui.label(&column.name);
                                                ui.label(&column.data_type);
                                                ui.label(if column.is_nullable { "YES" } else { "NO" });
                                                ui.label(column.default_value.as_deref().unwrap_or(""));
                                                ui.end_row();
                                            }
                                        });
                                    });
                                }
                            }
                            TabContent::Query { sql, results, columns, loading, error } => {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Image::new(QUERY_ICON.clone())
                                            .fit_to_exact_size(egui::Vec2::new(20.0, 20.0))
                                            .tint(ui.visuals().text_color()));
                                        ui.heading("SQL Query");
                                        
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if *loading {
                                                ui.spinner();
                                            } else if icon_button(ui, &PLAY_ICON, "Execute").clicked() {
                                                if !sql.trim().is_empty() {
                                                    *loading = true;
                                                    *error = None;
                                                    if let Some(sender) = &self.db_sender {
                                                        let _ = sender.send(crate::models::DatabaseMessage::ExecuteQuery(sql.clone(), tab.id.clone()));
                                                    }
                                                }
                                            }
                                        });
                                    });
                                    
                                    ui.separator();
                                    
                                    // SQL input
                                    ui.label("SQL:");
                                    let response = ui.add_sized(
                                        [ui.available_width(), 100.0],
                                        egui::TextEdit::multiline(sql)
                                            .code_editor()
                                            .desired_rows(5)
                                    );
                                    
                                    // Execute on Ctrl+Enter
                                    if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl) {
                                        if !sql.trim().is_empty() {
                                            *loading = true;
                                            *error = None;
                                            if let Some(sender) = &self.db_sender {
                                                let _ = sender.send(crate::models::DatabaseMessage::ExecuteQuery(sql.clone(), tab.id.clone()));
                                            }
                                        }
                                    }
                                    
                                    ui.separator();
                                    
                                    // Results
                                    if let Some(error_msg) = error {
                                        ui.horizontal(|ui| {
                                            ui.add(egui::Image::new(ERROR_ICON.clone())
                                                .fit_to_exact_size(egui::Vec2::new(18.0, 18.0))
                                                .tint(ui.visuals().error_fg_color));
                                            ui.colored_label(ui.visuals().error_fg_color, error_msg);
                                        });
                                    } else if let Some(data) = results {
                                        ui.horizontal(|ui| {
                                            ui.add(egui::Image::new(RESULTS_ICON.clone())
                                                .fit_to_exact_size(egui::Vec2::new(18.0, 18.0))
                                                .tint(ui.visuals().text_color()));
                                            ui.heading("Results");
                                            ui.label(format!("({} rows)", data.len()));
                                        });
                                        
                                        egui::ScrollArea::both().show(ui, |ui| {
                                            egui::Grid::new("query_results_grid").striped(true).show(ui, |ui| {
                                                // Header
                                                for column in columns.iter() {
                                                    ui.strong(column);
                                                }
                                                ui.end_row();
                                                
                                                // Data rows
                                                for row in data.iter() {
                                                    for column in columns.iter() {
                                                        let null_string = "NULL".to_string();
                                                        let value = row.get(column).unwrap_or(&null_string);
                                                        ui.label(value);
                                                    }
                                                    ui.end_row();
                                                }
                                            });
                                        });
                                    }
                                });
                            }
                        }
                    }
                }
            }
        });
        
        // Connection wizard dialog
        if self.show_connection_wizard {
            egui::Window::new("Connection Manager")
                .collapsible(false)
                .resizable(true)
                .default_width(600.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui::connection_wizard::show_connection_wizard_ui(self, ui);
                });
        }

        // Error dialog
        if let Some(error) = &self.error_message.clone() {
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("OK").clicked() {
                        self.error_message = None;
                    }
                });
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("PostgreSQL GUI - Sequel Pro Style")
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "PostgreSQL GUI",
        options,
        Box::new(|cc| {
            // Install image loaders to support PNG files
            install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(PostgresGuiApp::default()))
        }),
    )
}

// PostgreSQL enum types are now fully supported!
// Enum values will be displayed as "Home (team_type)" in table data
// Enum schemas will show "team_type (Home, Away)" in schema view

use eframe::egui;
use crate::models::{PostgresGuiApp, DatabaseConnection, DatabaseMessage};
use crate::ui::icons::*;

pub fn show_connection_wizard_ui(app: &mut PostgresGuiApp, ui: &mut egui::Ui) {
    // Connection list
    ui.horizontal(|ui| {
        ui.add(egui::Image::from_bytes("saved_connections_icon", SAVE_ICON)
            .fit_to_exact_size(egui::Vec2::new(20.0, 20.0))
            .tint(ui.visuals().text_color()));
        ui.heading("Saved Connections");
    });
    ui.separator();

    let mut connection_to_edit = None;
    let mut connection_to_delete = None;
    let mut connection_to_connect = None;

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for conn in &app.connections {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Create an interactive area for the connection info that can detect double-clicks
                    let (rect, response) = ui.allocate_exact_size(
                        egui::Vec2::new(ui.available_width() - 120.0, 40.0), // Reserve space for buttons
                        egui::Sense::click()
                    );
                    
                    // Draw the connection info in the allocated area
                    ui.allocate_ui_at_rect(rect, |ui| {
                        ui.vertical(|ui| {
                            ui.strong(&conn.name);
                            ui.label(&conn.url);
                        });
                    });
                    
                    // Handle double-click to connect
                    if response.double_clicked() {
                        connection_to_connect = Some(conn.clone());
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, CONNECT_ICON, "Connect").clicked() {
                            connection_to_connect = Some(conn.clone());
                        }

                        if icon_button(ui, DELETE_ICON, "Delete").clicked() {
                            connection_to_delete = Some(conn.id.clone());
                        }

                        if icon_button(ui, EDIT_ICON, "Edit").clicked() {
                            connection_to_edit = Some(conn.clone());
                        }
                    });
                });
            });
        }
    });

    // Handle connection actions
    if let Some(conn) = connection_to_edit {
        app.connection_wizard.editing_id = Some(conn.id);
        app.connection_wizard.name = conn.name;
        app.connection_wizard.url = conn.url;
    }

    if let Some(id) = connection_to_delete {
        app.connections.retain(|c| c.id != id);
        app.save_connections();
    }

    if let Some(conn) = connection_to_connect {
        app.loading = true;
        app.current_connection = Some(conn.clone());
        
        if let Some(sender) = &app.db_sender {
            let _ = sender.send(DatabaseMessage::Connect(conn));
        }
        
        app.show_connection_wizard = false;
    }

    ui.separator();

    // Connection form
    ui.horizontal(|ui| {
        let icon = if app.connection_wizard.editing_id.is_some() { EDIT_ICON } else { ADD_ICON };
        let text = if app.connection_wizard.editing_id.is_some() { "Edit Connection" } else { "New Connection" };
        ui.add(egui::Image::from_bytes("connection_form_icon", icon)
            .fit_to_exact_size(egui::Vec2::new(20.0, 20.0))
            .tint(ui.visuals().text_color()));
        ui.heading(text);
    });

    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut app.connection_wizard.name);
    });

    ui.horizontal(|ui| {
        ui.label("URL:");
        ui.text_edit_singleline(&mut app.connection_wizard.url);
        
        if ui.button("Paste example").clicked() {
            app.connection_wizard.url = "postgres://postgres:postgres@localhost:5432/database".to_string();
        }
    });

    ui.collapsing("Connection URL Examples", |ui| {
        ui.label("Local: postgres://username:password@localhost:5432/database");
        ui.label("Remote: postgres://user:pass@host:port/db");
        ui.label("With SSL: postgres://user:pass@host:port/db?sslmode=require");
    });

    ui.horizontal(|ui| {
        if icon_button(ui, TEST_ICON, "Test Connection").clicked() && !app.connection_wizard.url.is_empty() {
            app.connection_wizard.testing = true;
            app.connection_wizard.test_result = None;
            
            if let Some(sender) = &app.db_sender {
                let _ = sender.send(DatabaseMessage::TestConnection(app.connection_wizard.url.clone()));
            }
        }

        if app.connection_wizard.testing {
            ui.spinner();
            ui.label("Testing...");
        }
    });

    if let Some(result) = &app.connection_wizard.test_result {
        ui.label(result);
    }

    ui.separator();

    ui.horizontal(|ui| {
        if icon_button(ui, SAVE_ICON, "Save").clicked() && !app.connection_wizard.name.is_empty() && !app.connection_wizard.url.is_empty() {
            if let Some(editing_id) = &app.connection_wizard.editing_id {
                // Update existing connection
                if let Some(conn) = app.connections.iter_mut().find(|c| c.id == *editing_id) {
                    conn.name = app.connection_wizard.name.clone();
                    conn.url = app.connection_wizard.url.clone();
                }
            } else {
                // Add new connection
                let new_conn = DatabaseConnection {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: app.connection_wizard.name.clone(),
                    url: app.connection_wizard.url.clone(),
                };
                app.connections.push(new_conn);
            }
            
            app.save_connections();
            app.connection_wizard = crate::models::ConnectionWizard::default();
        }

        if icon_button(ui, CANCEL_ICON, "Cancel").clicked() {
            app.show_connection_wizard = false;
        }
    });
} 
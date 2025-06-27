use eframe::egui;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_postgres::{Client, NoTls, Row};
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatabaseConnection {
    id: String,
    name: String,
    url: String,
}

#[derive(Debug, Clone)]
struct TableInfo {
    name: String,
    schema: String,
}

#[derive(Debug, Clone)]
struct ColumnInfo {
    name: String,
    data_type: String,
    is_nullable: bool,
    default_value: Option<String>,
}

#[derive(Debug, Clone)]
enum TabContent {
    TableData { 
        table: TableInfo, 
        data: Vec<HashMap<String, String>>,
        columns: Vec<String>,
        loading: bool,
    },
    TableSchema { 
        table: TableInfo, 
        columns: Vec<ColumnInfo>,
        loading: bool,
    },
    Query { 
        sql: String, 
        results: Option<Vec<HashMap<String, String>>>,
        columns: Vec<String>,
        loading: bool,
        error: Option<String>,
    },
}

#[derive(Debug, Clone)]
struct Tab {
    id: String,
    title: String,
    content: TabContent,
}

#[derive(Debug)]
enum DatabaseMessage {
    Connect(DatabaseConnection),
    LoadTableData(TableInfo, String), // table, tab_id
    LoadTableSchema(TableInfo, String), // table, tab_id
    ExecuteQuery(String, String), // sql, tab_id
    TestConnection(String), // url
}

#[derive(Debug)]
enum DatabaseResponse {
    Connected(Vec<TableInfo>),
    TableDataLoaded(String, Vec<HashMap<String, String>>, Vec<String>), // tab_id, data, columns
    TableSchemaLoaded(String, Vec<ColumnInfo>), // tab_id, columns
    QueryResult(String, Vec<HashMap<String, String>>, Vec<String>), // tab_id, data, columns
    Error(String),
    ConnectionTestResult(bool, String),
}

struct PostgresGuiApp {
    connections: Vec<DatabaseConnection>,
    show_connection_wizard: bool,
    connection_wizard: ConnectionWizard,
    current_connection: Option<DatabaseConnection>,
    tables: Vec<TableInfo>,
    tabs: Vec<Tab>,
    active_tab_index: Option<usize>,
    loading: bool,
    error_message: Option<String>,
    
    // Async communication
    db_sender: Option<mpsc::UnboundedSender<DatabaseMessage>>,
    db_receiver: mpsc::UnboundedReceiver<DatabaseResponse>,
}

#[derive(Default)]
struct ConnectionWizard {
    editing_id: Option<String>,
    name: String,
    url: String,
    test_result: Option<String>,
    testing: bool,
}

impl Default for PostgresGuiApp {
    fn default() -> Self {
        let (_response_sender, response_receiver) = mpsc::unbounded_channel();
        
        Self {
            connections: Self::load_connections(),
            show_connection_wizard: false,
            connection_wizard: ConnectionWizard::default(),
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
    fn load_connections() -> Vec<DatabaseConnection> {
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

    fn save_connections(&self) {
        if let Some(config_dir) = dirs::config_dir() {
            let config_dir = config_dir.join("postgres-gui");
            let _ = std::fs::create_dir_all(&config_dir);
            let config_file = config_dir.join("connections.json");
            if let Ok(content) = serde_json::to_string_pretty(&self.connections) {
                let _ = std::fs::write(config_file, content);
            }
        }
    }

    fn start_database_worker(&mut self, ctx: &egui::Context) {
        let (db_sender, mut db_receiver) = mpsc::unbounded_channel();
        let (response_sender, response_receiver) = mpsc::unbounded_channel();
        
        self.db_sender = Some(db_sender);
        self.db_receiver = response_receiver;
        
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let mut client: Option<Arc<Mutex<Client>>> = None;
            
            while let Some(message) = db_receiver.recv().await {
                match message {
                    DatabaseMessage::Connect(connection) => {
                        match Self::connect_to_database(&connection.url).await {
                            Ok(new_client) => {
                                client = Some(Arc::new(Mutex::new(new_client)));
                                if let Ok(tables) = Self::load_tables(&client).await {
                                    let _ = response_sender.send(DatabaseResponse::Connected(tables));
                                } else {
                                    let _ = response_sender.send(DatabaseResponse::Error("Failed to load tables".to_string()));
                                }
                            }
                            Err(e) => {
                                let _ = response_sender.send(DatabaseResponse::Error(format!("Connection failed: {}", e)));
                            }
                        }
                    }
                    DatabaseMessage::LoadTableData(table, tab_id) => {
                        if let Some(ref client) = client {
                            match Self::load_table_data(&client, &table).await {
                                Ok((data, columns)) => {
                                    let _ = response_sender.send(DatabaseResponse::TableDataLoaded(tab_id, data, columns));
                                }
                                Err(e) => {
                                    let _ = response_sender.send(DatabaseResponse::Error(format!("Failed to load table data: {}", e)));
                                }
                            }
                        }
                    }
                    DatabaseMessage::LoadTableSchema(table, tab_id) => {
                        if let Some(ref client) = client {
                            match Self::load_table_schema(&client, &table).await {
                                Ok(columns) => {
                                    let _ = response_sender.send(DatabaseResponse::TableSchemaLoaded(tab_id, columns));
                                }
                                Err(e) => {
                                    let _ = response_sender.send(DatabaseResponse::Error(format!("Failed to load table schema: {}", e)));
                                }
                            }
                        }
                    }
                    DatabaseMessage::ExecuteQuery(sql, tab_id) => {
                        if let Some(ref client) = client {
                            match Self::execute_query(&client, &sql).await {
                                Ok((data, columns)) => {
                                    let _ = response_sender.send(DatabaseResponse::QueryResult(tab_id, data, columns));
                                }
                                Err(e) => {
                                    let _ = response_sender.send(DatabaseResponse::Error(format!("Query failed: {}", e)));
                                }
                            }
                        }
                    }
                    DatabaseMessage::TestConnection(url) => {
                        match Self::test_connection(&url).await {
                            Ok(_) => {
                                let _ = response_sender.send(DatabaseResponse::ConnectionTestResult(true, "Connection successful".to_string()));
                            }
                            Err(e) => {
                                let _ = response_sender.send(DatabaseResponse::ConnectionTestResult(false, format!("Connection failed: {}", e)));
                            }
                        }
                    }
                }
                ctx.request_repaint();
            }
        });
    }

    async fn connect_to_database(url: &str) -> Result<Client> {
        let (client, connection) = tokio_postgres::connect(url, NoTls).await?;
        
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        Ok(client)
    }

    async fn test_connection(url: &str) -> Result<()> {
        let (client, connection) = tokio_postgres::connect(url, NoTls).await?;
        
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        // Test with a simple query
        client.query("SELECT 1", &[]).await?;
        Ok(())
    }

    async fn load_tables(client: &Option<Arc<Mutex<Client>>>) -> Result<Vec<TableInfo>> {
        if let Some(client) = client {
            let client = client.lock().await;
            let rows = client
                .query(
                    "SELECT table_name, table_schema FROM information_schema.tables 
                     WHERE table_schema NOT IN ('information_schema', 'pg_catalog') 
                     ORDER BY table_schema, table_name",
                    &[],
                )
                .await?;

            let tables = rows
                .iter()
                .map(|row| TableInfo {
                    name: row.get(0),
                    schema: row.get(1),
                })
                .collect();
            
            Ok(tables)
        } else {
            Err(anyhow::anyhow!("No database connection"))
        }
    }

    async fn load_table_data(client: &Arc<Mutex<Client>>, table: &TableInfo) -> Result<(Vec<HashMap<String, String>>, Vec<String>)> {
        let client = client.lock().await;
        let query = format!("SELECT * FROM {}.{} LIMIT 1000", table.schema, table.name);
        let rows = client.query(&query, &[]).await?;
        
        let columns: Vec<String> = if !rows.is_empty() {
            rows[0].columns().iter().map(|col| col.name().to_string()).collect()
        } else {
            Vec::new()
        };
        
        let data: Vec<HashMap<String, String>> = rows
            .iter()
            .map(|row| {
                let mut map = HashMap::new();
                for (i, column) in row.columns().iter().enumerate() {
                    let value = Self::format_cell_value(row, i);
                    map.insert(column.name().to_string(), value);
                }
                map
            })
            .collect();
        
        Ok((data, columns))
    }

    async fn load_table_schema(client: &Arc<Mutex<Client>>, table: &TableInfo) -> Result<Vec<ColumnInfo>> {
        let client = client.lock().await;
        let rows = client
            .query(
                "SELECT column_name, data_type, is_nullable, column_default 
                 FROM information_schema.columns 
                 WHERE table_schema = $1 AND table_name = $2 
                 ORDER BY ordinal_position",
                &[&table.schema, &table.name],
            )
            .await?;

        let columns = rows
            .iter()
            .map(|row| ColumnInfo {
                name: row.get(0),
                data_type: row.get(1),
                is_nullable: row.get::<_, String>(2) == "YES",
                default_value: row.get(3),
            })
            .collect();

        Ok(columns)
    }

    async fn execute_query(client: &Arc<Mutex<Client>>, sql: &str) -> Result<(Vec<HashMap<String, String>>, Vec<String>)> {
        let client = client.lock().await;
        let rows = client.query(sql, &[]).await?;
        
        let columns: Vec<String> = if !rows.is_empty() {
            rows[0].columns().iter().map(|col| col.name().to_string()).collect()
        } else {
            Vec::new()
        };
        
        let data: Vec<HashMap<String, String>> = rows
            .iter()
            .map(|row| {
                let mut map = HashMap::new();
                for (i, column) in row.columns().iter().enumerate() {
                    let value = Self::format_cell_value(row, i);
                    map.insert(column.name().to_string(), value);
                }
                map
            })
            .collect();
        
        Ok((data, columns))
    }

    fn format_cell_value(row: &Row, index: usize) -> String {
        // Handle different PostgreSQL types
        if let Ok(val) = row.try_get::<_, Option<String>>(index) {
            val.unwrap_or_else(|| "NULL".to_string())
        } else if let Ok(val) = row.try_get::<_, Option<i32>>(index) {
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        } else if let Ok(val) = row.try_get::<_, Option<i64>>(index) {
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        } else if let Ok(val) = row.try_get::<_, Option<f64>>(index) {
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        } else if let Ok(val) = row.try_get::<_, Option<bool>>(index) {
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        } else {
            "Unknown".to_string()
        }
    }

    fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        self.active_tab_index = Some(self.tabs.len() - 1);
    }

    fn close_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            if let Some(active) = self.active_tab_index {
                if active == index {
                    self.active_tab_index = if self.tabs.is_empty() {
                        None
                    } else if index >= self.tabs.len() {
                        Some(self.tabs.len() - 1)
                    } else {
                        Some(index)
                    };
                } else if active > index {
                    self.active_tab_index = Some(active - 1);
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
                DatabaseResponse::TableDataLoaded(tab_id, data, columns) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let TabContent::TableData { data: tab_data, columns: tab_columns, loading, .. } = &mut tab.content {
                            *tab_data = data;
                            *tab_columns = columns;
                            *loading = false;
                        }
                    }
                }
                DatabaseResponse::TableSchemaLoaded(tab_id, columns) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let TabContent::TableSchema { columns: tab_columns, loading, .. } = &mut tab.content {
                            *tab_columns = columns;
                            *loading = false;
                        }
                    }
                }
                DatabaseResponse::QueryResult(tab_id, data, columns) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let TabContent::Query { results, columns: tab_columns, loading, error, .. } = &mut tab.content {
                            *results = Some(data);
                            *tab_columns = columns;
                            *loading = false;
                            *error = None;
                        }
                    }
                }
                DatabaseResponse::Error(error) => {
                    self.error_message = Some(error);
                    self.loading = false;
                }
                DatabaseResponse::ConnectionTestResult(_success, message) => {
                    self.connection_wizard.test_result = Some(message);
                    self.connection_wizard.testing = false;
                }
            }
        }
    }
}

impl eframe::App for PostgresGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Initialize database worker if not already done
        if self.db_sender.is_none() {
            self.start_database_worker(ctx);
        }

        // Handle async responses
        self.handle_database_responses();

        // Top panel with connect button
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("🔌 Connect").clicked() {
                    self.show_connection_wizard = true;
                    self.connection_wizard = ConnectionWizard::default();
                }

                if let Some(conn) = &self.current_connection {
                    ui.separator();
                    ui.label(format!("📡 Connected to: {}", conn.name));
                }

                if self.loading {
                    ui.separator();
                    ui.spinner();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("➕ New Query").clicked() {
                        let tab = Tab {
                            id: uuid::Uuid::new_v4().to_string(),
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
        });

        // Left panel for table browser
        egui::SidePanel::left("table_browser").resizable(true).show(ctx, |ui| {
            ui.heading("📋 Tables");
            ui.separator();

            if self.current_connection.is_some() {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for table in &self.tables.clone() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("📊 {}.{}", table.schema, table.name));
                            });
                            
                            ui.horizontal(|ui| {
                                if ui.small_button("📄 Data").clicked() {
                                    let tab_id = uuid::Uuid::new_v4().to_string();
                                    let tab = Tab {
                                        id: tab_id.clone(),
                                        title: format!("{} - Data", table.name),
                                        content: TabContent::TableData {
                                            table: table.clone(),
                                            data: Vec::new(),
                                            columns: Vec::new(),
                                            loading: true,
                                        },
                                    };
                                    self.add_tab(tab);
                                    
                                    if let Some(sender) = &self.db_sender {
                                        let _ = sender.send(DatabaseMessage::LoadTableData(table.clone(), tab_id));
                                    }
                                }
                                
                                if ui.small_button("🏗️ Schema").clicked() {
                                    let tab_id = uuid::Uuid::new_v4().to_string();
                                    let tab = Tab {
                                        id: tab_id.clone(),
                                        title: format!("{} - Schema", table.name),
                                        content: TabContent::TableSchema {
                                            table: table.clone(),
                                            columns: Vec::new(),
                                            loading: true,
                                        },
                                    };
                                    self.add_tab(tab);
                                    
                                    if let Some(sender) = &self.db_sender {
                                        let _ = sender.send(DatabaseMessage::LoadTableSchema(table.clone(), tab_id));
                                    }
                                }
                            });
                        });
                        ui.add_space(4.0);
                    }
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("❌ No connection");
                });
            }
        });

        // Main content area with tabs
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.tabs.is_empty() {
                // Tab bar
                ui.horizontal(|ui| {
                    let mut tabs_to_close = Vec::new();
                    
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let is_active = self.active_tab_index == Some(i);
                        
                        ui.horizontal(|ui| {
                            if ui.selectable_label(is_active, &tab.title).clicked() {
                                self.active_tab_index = Some(i);
                            }
                            
                            if ui.small_button("❌").clicked() {
                                tabs_to_close.push(i);
                            }
                        });
                        
                        ui.separator();
                    }
                    
                    // Close tabs (in reverse order to maintain indices)
                    for &index in tabs_to_close.iter().rev() {
                        self.close_tab(index);
                    }
                });

                ui.separator();

                // Tab content
                if let Some(active_index) = self.active_tab_index {
                    if let Some(tab) = self.tabs.get_mut(active_index) {
                        match &mut tab.content {
                            TabContent::TableData { table, data, columns, loading } => {
                                ui.heading(format!("📊 Data: {}.{}", table.schema, table.name));
                                
                                if *loading {
                                    ui.centered_and_justified(|ui| {
                                        ui.spinner();
                                        ui.label("Loading table data...");
                                    });
                                } else if data.is_empty() {
                                    ui.centered_and_justified(|ui| {
                                        ui.label("No data found");
                                    });
                                } else {
                                    egui::ScrollArea::both().show(ui, |ui| {
                                        egui::Grid::new("table_grid")
                                            .striped(true)
                                            .min_col_width(100.0)
                                            .show(ui, |ui| {
                                                // Header
                                                for column in columns.iter() {
                                                    ui.strong(column);
                                                }
                                                ui.end_row();
                                                
                                                // Data rows
                                                for row in data {
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
                            }
                            TabContent::TableSchema { table, columns, loading } => {
                                ui.heading(format!("🏗️ Schema: {}.{}", table.schema, table.name));
                                
                                if *loading {
                                    ui.centered_and_justified(|ui| {
                                        ui.spinner();
                                        ui.label("Loading table schema...");
                                    });
                                } else {
                                    egui::ScrollArea::vertical().show(ui, |ui| {
                                        egui::Grid::new("schema_grid")
                                            .striped(true)
                                            .show(ui, |ui| {
                                                // Header
                                                ui.strong("Column");
                                                ui.strong("Type");
                                                ui.strong("Nullable");
                                                ui.strong("Default");
                                                ui.end_row();
                                                
                                                // Schema rows
                                                for column in columns {
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
                                ui.heading("💻 Query");
                                
                                ui.horizontal(|ui| {
                                    if ui.button("▶️ Execute").clicked() && !sql.is_empty() {
                                        *loading = true;
                                        *error = None;
                                        
                                        if let Some(sender) = &self.db_sender {
                                            let _ = sender.send(DatabaseMessage::ExecuteQuery(sql.clone(), tab.id.clone()));
                                        }
                                    }
                                    
                                    if *loading {
                                        ui.spinner();
                                        ui.label("Executing...");
                                    }
                                });
                                
                                ui.separator();
                                
                                // SQL Editor
                                ui.label("SQL:");
                                let sql_editor = egui::TextEdit::multiline(sql)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(8)
                                    .font(egui::TextStyle::Monospace);
                                ui.add(sql_editor);
                                
                                ui.separator();
                                
                                // Results
                                if let Some(error_msg) = error {
                                    ui.colored_label(egui::Color32::RED, format!("❌ Error: {}", error_msg));
                                } else if let Some(data) = results {
                                    ui.label(format!("📊 Results ({} rows):", data.len()));
                                    
                                    if data.is_empty() {
                                        ui.label("No results");
                                    } else {
                                        egui::ScrollArea::both().show(ui, |ui| {
                                            egui::Grid::new("query_results_grid")
                                                .striped(true)
                                                .min_col_width(100.0)
                                                .show(ui, |ui| {
                                                    // Header
                                                    for column in columns.iter() {
                                                        ui.strong(column);
                                                    }
                                                    ui.end_row();
                                                    
                                                    // Data rows
                                                    for row in data {
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
                                }
                            }
                        }
                    }
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(100.0);
                        ui.heading("🚀 PostgreSQL GUI");
                        ui.add_space(20.0);
                        ui.label("Welcome! Get started by:");
                        ui.label("1. 🔌 Connecting to a database");
                        ui.label("2. 📋 Browsing tables in the sidebar");
                        ui.label("3. ➕ Creating a new query tab");
                    });
                });
            }
        });

        // Connection wizard modal
        if self.show_connection_wizard {
            egui::Window::new("🔧 Connection Manager")
                .collapsible(false)
                .resizable(true)
                .default_width(600.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    self.show_connection_wizard_ui(ui);
                });
        }

        // Error dialog
        if let Some(error) = &self.error_message.clone() {
            egui::Window::new("❌ Error")
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

impl PostgresGuiApp {
    fn show_connection_wizard_ui(&mut self, ui: &mut egui::Ui) {
        // Connection list
        ui.heading("💾 Saved Connections");
        ui.separator();

        let mut connection_to_edit = None;
        let mut connection_to_delete = None;
        let mut connection_to_connect = None;

        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for conn in &self.connections {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(&conn.name);
                            ui.label(&conn.url);
                        });
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🔌 Connect").clicked() {
                                connection_to_connect = Some(conn.clone());
                            }
                            
                            if ui.button("🗑️ Delete").clicked() {
                                connection_to_delete = Some(conn.id.clone());
                            }
                            
                            if ui.button("✏️ Edit").clicked() {
                                connection_to_edit = Some(conn.clone());
                            }
                        });
                    });
                });
            }
        });

        // Handle connection actions
        if let Some(conn) = connection_to_edit {
            self.connection_wizard.editing_id = Some(conn.id);
            self.connection_wizard.name = conn.name;
            self.connection_wizard.url = conn.url;
        }

        if let Some(id) = connection_to_delete {
            self.connections.retain(|c| c.id != id);
            self.save_connections();
        }

        if let Some(conn) = connection_to_connect {
            self.loading = true;
            self.current_connection = Some(conn.clone());
            
            if let Some(sender) = &self.db_sender {
                let _ = sender.send(DatabaseMessage::Connect(conn));
            }
            
            self.show_connection_wizard = false;
        }

        ui.separator();

        // Connection form
        ui.heading(if self.connection_wizard.editing_id.is_some() {
            "✏️ Edit Connection"
        } else {
            "➕ New Connection"
        });

        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.connection_wizard.name);
        });

        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.text_edit_singleline(&mut self.connection_wizard.url);
            
            if ui.button("📋").on_hover_text("Paste example").clicked() {
                self.connection_wizard.url = "postgres://postgres:postgres@localhost:5432/database".to_string();
            }
        });

        ui.collapsing("💡 Connection URL Examples", |ui| {
            ui.label("Local: postgres://username:password@localhost:5432/database");
            ui.label("Remote: postgres://user:pass@host:port/db");
            ui.label("With SSL: postgres://user:pass@host:port/db?sslmode=require");
        });

        ui.horizontal(|ui| {
            if ui.button("🧪 Test Connection").clicked() && !self.connection_wizard.url.is_empty() {
                self.connection_wizard.testing = true;
                self.connection_wizard.test_result = None;
                
                if let Some(sender) = &self.db_sender {
                    let _ = sender.send(DatabaseMessage::TestConnection(self.connection_wizard.url.clone()));
                }
            }

            if self.connection_wizard.testing {
                ui.spinner();
                ui.label("Testing...");
            }
        });

        if let Some(result) = &self.connection_wizard.test_result {
            ui.label(result);
        }

        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("💾 Save").clicked() && !self.connection_wizard.name.is_empty() && !self.connection_wizard.url.is_empty() {
                if let Some(editing_id) = &self.connection_wizard.editing_id {
                    // Update existing connection
                    if let Some(conn) = self.connections.iter_mut().find(|c| c.id == *editing_id) {
                        conn.name = self.connection_wizard.name.clone();
                        conn.url = self.connection_wizard.url.clone();
                    }
                } else {
                    // Add new connection
                    let new_conn = DatabaseConnection {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: self.connection_wizard.name.clone(),
                        url: self.connection_wizard.url.clone(),
                    };
                    self.connections.push(new_conn);
                }
                
                self.save_connections();
                self.connection_wizard = ConnectionWizard::default();
            }

            if ui.button("❌ Cancel").clicked() {
                self.show_connection_wizard = false;
            }
        });
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
        Box::new(|_cc| {
            Ok(Box::new(PostgresGuiApp::default()))
        }),
    )
}

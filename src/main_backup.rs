use eframe::egui;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_postgres::{Client, NoTls, Row};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, NaiveDate, NaiveTime, Utc};

// Icon drawing functions using egui's built-in drawing capabilities

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
    // Helper function to draw simple geometric icons
    fn draw_icon(ui: &mut egui::Ui, icon_type: &str, size: f32, color: egui::Color32) {
        let (response, painter) = ui.allocate_painter(egui::Vec2::splat(size), egui::Sense::hover());
        let rect = response.rect;
        let center = rect.center();
        let radius = size * 0.4;
        
        match icon_type {
            "connect" => {
                // Draw a plug/connection icon - circle with dot
                painter.circle_stroke(center, radius, egui::Stroke::new(2.0, color));
                painter.circle_filled(center, radius * 0.3, color);
            }
            "disconnect" => {
                // Draw a disconnect icon - circle with X
                painter.circle_stroke(center, radius, egui::Stroke::new(2.0, color));
                let offset = radius * 0.5;
                painter.line_segment([center + egui::Vec2::new(-offset, -offset), center + egui::Vec2::new(offset, offset)], egui::Stroke::new(2.0, color));
                painter.line_segment([center + egui::Vec2::new(-offset, offset), center + egui::Vec2::new(offset, -offset)], egui::Stroke::new(2.0, color));
            }
            "add" => {
                // Draw a plus icon
                let offset = radius * 0.7;
                painter.line_segment([center + egui::Vec2::new(-offset, 0.0), center + egui::Vec2::new(offset, 0.0)], egui::Stroke::new(2.0, color));
                painter.line_segment([center + egui::Vec2::new(0.0, -offset), center + egui::Vec2::new(0.0, offset)], egui::Stroke::new(2.0, color));
            }
            "table" => {
                // Draw a table icon - rectangle with lines
                let rect_size = egui::Vec2::splat(radius * 1.4);
                let table_rect = egui::Rect::from_center_size(center, rect_size);
                painter.rect_stroke(table_rect, 2.0, egui::Stroke::new(2.0, color));
                painter.line_segment([egui::Pos2::new(table_rect.left(), center.y), egui::Pos2::new(table_rect.right(), center.y)], egui::Stroke::new(1.0, color));
            }
            "data" => {
                // Draw a document icon - rectangle with folded corner
                let rect_size = egui::Vec2::new(radius * 1.2, radius * 1.6);
                let doc_rect = egui::Rect::from_center_size(center, rect_size);
                painter.rect_stroke(doc_rect, 2.0, egui::Stroke::new(2.0, color));
                // Folded corner
                let corner_size = radius * 0.3;
                let corner_pos = egui::Pos2::new(doc_rect.right() - corner_size, doc_rect.top());
                painter.line_segment([corner_pos, egui::Pos2::new(doc_rect.right(), doc_rect.top() + corner_size)], egui::Stroke::new(2.0, color));
                painter.line_segment([corner_pos, egui::Pos2::new(doc_rect.right() - corner_size, doc_rect.top() + corner_size)], egui::Stroke::new(2.0, color));
            }
            "schema" => {
                // Draw a gear icon - simplified
                painter.circle_stroke(center, radius, egui::Stroke::new(2.0, color));
                painter.circle_filled(center, radius * 0.4, color);
            }
            "close" => {
                // Draw an X
                let offset = radius * 0.7;
                painter.line_segment([center + egui::Vec2::new(-offset, -offset), center + egui::Vec2::new(offset, offset)], egui::Stroke::new(2.0, color));
                painter.line_segment([center + egui::Vec2::new(-offset, offset), center + egui::Vec2::new(offset, -offset)], egui::Stroke::new(2.0, color));
            }
            "play" => {
                // Draw a play triangle
                let offset = radius * 0.6;
                let triangle = [
                    center + egui::Vec2::new(-offset, -offset),
                    center + egui::Vec2::new(-offset, offset),
                    center + egui::Vec2::new(offset, 0.0),
                ];
                painter.add(egui::Shape::convex_polygon(triangle.to_vec(), color, egui::Stroke::NONE));
            }
            "query" => {
                // Draw brackets < >
                let offset = radius * 0.6;
                painter.line_segment([center + egui::Vec2::new(-offset, -offset*0.5), center + egui::Vec2::new(-offset*0.5, 0.0)], egui::Stroke::new(2.0, color));
                painter.line_segment([center + egui::Vec2::new(-offset*0.5, 0.0), center + egui::Vec2::new(-offset, offset*0.5)], egui::Stroke::new(2.0, color));
                painter.line_segment([center + egui::Vec2::new(offset, -offset*0.5), center + egui::Vec2::new(offset*0.5, 0.0)], egui::Stroke::new(2.0, color));
                painter.line_segment([center + egui::Vec2::new(offset*0.5, 0.0), center + egui::Vec2::new(offset, offset*0.5)], egui::Stroke::new(2.0, color));
            }
            "error" => {
                // Draw a triangle with exclamation
                let offset = radius * 0.8;
                let triangle = [
                    center + egui::Vec2::new(0.0, -offset),
                    center + egui::Vec2::new(-offset*0.8, offset*0.6),
                    center + egui::Vec2::new(offset*0.8, offset*0.6),
                ];
                painter.add(egui::Shape::convex_polygon(triangle.to_vec(), color, egui::Stroke::NONE));
            }
            "save" => {
                // Draw a floppy disk - rectangle with notch
                let rect_size = egui::Vec2::splat(radius * 1.4);
                let save_rect = egui::Rect::from_center_size(center, rect_size);
                painter.rect_stroke(save_rect, 2.0, egui::Stroke::new(2.0, color));
                // Notch
                let notch_size = radius * 0.3;
                painter.rect_filled(egui::Rect::from_min_size(egui::Pos2::new(save_rect.right() - notch_size, save_rect.top()), egui::Vec2::new(notch_size, notch_size)), 2.0, ui.style().visuals.extreme_bg_color);
            }
            "edit" => {
                // Draw a pencil - line with triangle tip
                let offset = radius * 0.7;
                painter.line_segment([center + egui::Vec2::new(-offset, offset), center + egui::Vec2::new(offset*0.5, -offset*0.5)], egui::Stroke::new(2.0, color));
                let tip = [
                    center + egui::Vec2::new(offset*0.5, -offset*0.5),
                    center + egui::Vec2::new(offset*0.7, -offset*0.3),
                    center + egui::Vec2::new(offset*0.3, -offset*0.7),
                ];
                painter.add(egui::Shape::convex_polygon(tip.to_vec(), color, egui::Stroke::NONE));
            }
            "delete" => {
                // Draw a trash can - rectangle with lid
                let rect_size = egui::Vec2::new(radius * 1.0, radius * 1.2);
                let trash_rect = egui::Rect::from_center_size(center + egui::Vec2::new(0.0, radius*0.1), rect_size);
                painter.rect_stroke(trash_rect, 2.0, egui::Stroke::new(2.0, color));
                // Lid
                painter.line_segment([egui::Pos2::new(trash_rect.left() - radius*0.2, trash_rect.top()), egui::Pos2::new(trash_rect.right() + radius*0.2, trash_rect.top())], egui::Stroke::new(2.0, color));
            }
            "test" => {
                // Draw a magnifying glass - circle with handle
                let glass_center = center + egui::Vec2::new(-radius*0.2, -radius*0.2);
                painter.circle_stroke(glass_center, radius*0.5, egui::Stroke::new(2.0, color));
                painter.line_segment([glass_center + egui::Vec2::new(radius*0.35, radius*0.35), center + egui::Vec2::new(radius*0.6, radius*0.6)], egui::Stroke::new(2.0, color));
            }
            "cancel" => {
                // Draw a circle with X
                painter.circle_stroke(center, radius, egui::Stroke::new(2.0, color));
                let offset = radius * 0.5;
                painter.line_segment([center + egui::Vec2::new(-offset, -offset), center + egui::Vec2::new(offset, offset)], egui::Stroke::new(2.0, color));
                painter.line_segment([center + egui::Vec2::new(-offset, offset), center + egui::Vec2::new(offset, -offset)], egui::Stroke::new(2.0, color));
            }
            "results" => {
                // Draw a simple chart - bars
                let bar_width = radius * 0.2;
                let bar_spacing = radius * 0.3;
                for i in 0..3 {
                    let x = center.x - radius*0.5 + i as f32 * bar_spacing;
                    let height = radius * (0.3 + i as f32 * 0.3);
                    painter.rect_filled(egui::Rect::from_min_size(egui::Pos2::new(x, center.y + radius*0.5 - height), egui::Vec2::new(bar_width, height)), 2.0, color);
                }
            }
            _ => {
                // Default: draw a circle
                painter.circle_filled(center, radius * 0.3, color);
            }
        }
    }
    
    // Helper function to create icon buttons
    fn icon_button(ui: &mut egui::Ui, icon_type: &str, text: &str) -> egui::Response {
        ui.horizontal(|ui| {
            Self::draw_icon(ui, icon_type, 16.0, ui.style().visuals.text_color());
            ui.button(text)
        }).inner
    }
    
    // Helper function to create small icon buttons
    fn small_icon_button(ui: &mut egui::Ui, icon_type: &str, text: &str) -> egui::Response {
        ui.horizontal(|ui| {
            Self::draw_icon(ui, icon_type, 12.0, ui.style().visuals.text_color());
            ui.small_button(text)
        }).inner
    }
    
    // Helper function to create icon-only buttons
    fn icon_only_button(ui: &mut egui::Ui, icon_type: &str, tooltip: &str) -> egui::Response {
        let (response, _painter) = ui.allocate_painter(egui::Vec2::splat(20.0), egui::Sense::click());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        Self::draw_icon(ui, icon_type, 16.0, ui.style().visuals.text_color());
        response.on_hover_text(tooltip)
    }

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
        
        // First, get column information to detect enums and cast them to text
        let column_info = client.query(
            "SELECT c.column_name, c.data_type, c.udt_name, t.typtype
             FROM information_schema.columns c
             LEFT JOIN pg_type t ON t.typname = c.udt_name
             WHERE c.table_schema = $1 AND c.table_name = $2 
             ORDER BY c.ordinal_position",
            &[&table.schema, &table.name],
        ).await?;

        // Build a query that casts enum columns to text
        let mut select_parts = Vec::new();
        for row in &column_info {
            let column_name: String = row.get(0);
            let data_type: String = row.get(1);
            let typtype: Option<i8> = row.get(3);
            
            // If it's a user-defined type (likely an enum), cast to text
            if data_type == "USER-DEFINED" && typtype == Some(101) { // 'e' as i8 is 101
                select_parts.push(format!("{}::text as {}", column_name, column_name));
            } else {
                select_parts.push(column_name);
            }
        }
        
        let query = format!("SELECT {} FROM {}.{} LIMIT 1000", 
                           select_parts.join(", "), table.schema, table.name);
        let rows = client.query(&query, &[]).await?;
        
        let columns: Vec<String> = if !rows.is_empty() {
            rows[0].columns().iter().map(|col| col.name().to_string()).collect()
        } else {
            Vec::new()
        };
        
        // Create a mapping of column names to their original types for enum display
        let mut enum_types: HashMap<String, String> = HashMap::new();
        for row in &column_info {
            let column_name: String = row.get(0);
            let data_type: String = row.get(1);
            let udt_name: String = row.get(2);
            let typtype: Option<i8> = row.get(3);
            
            if data_type == "USER-DEFINED" && typtype == Some(101) {
                enum_types.insert(column_name, udt_name);
            }
        }

        let data: Vec<HashMap<String, String>> = rows
            .iter()
            .map(|row| {
                let mut map = HashMap::new();
                for (i, column) in row.columns().iter().enumerate() {
                    let column_name = column.name();
                    let mut value = Self::format_cell_value(row, i);
                    
                    // If this was originally an enum, add the type info
                    if let Some(enum_type) = enum_types.get(column_name) {
                        if value != "NULL" {
                            value = format!("{} ({})", value, enum_type);
                        }
                    }
                    
                    map.insert(column_name.to_string(), value);
                }
                map
            })
            .collect();
        
        Ok((data, columns))
    }

    async fn load_table_schema(client: &Arc<Mutex<Client>>, table: &TableInfo) -> Result<Vec<ColumnInfo>> {
        let client = client.lock().await;
        
        // Enhanced query to get more type information including enum values
        let rows = client
            .query(
                "SELECT 
                    c.column_name, 
                    c.data_type,
                    c.is_nullable, 
                    c.column_default,
                    c.udt_name,
                    CASE 
                        WHEN t.typtype = 'e' THEN 
                            (SELECT string_agg(e.enumlabel, ', ' ORDER BY e.enumsortorder) 
                             FROM pg_enum e WHERE e.enumtypid = t.oid)
                        ELSE NULL 
                    END as enum_values
                 FROM information_schema.columns c
                 LEFT JOIN pg_type t ON t.typname = c.udt_name
                 WHERE c.table_schema = $1 AND c.table_name = $2 
                 ORDER BY c.ordinal_position",
                &[&table.schema, &table.name],
            )
            .await?;

        let columns = rows
            .iter()
            .map(|row| {
                let data_type: String = row.get(1);
                let udt_name: String = row.get(4);
                let enum_values: Option<String> = row.get(5);
                
                // If it's a user-defined type (enum), show the enum values
                let display_type = if data_type == "USER-DEFINED" && enum_values.is_some() {
                    format!("{} ({})", udt_name, enum_values.unwrap_or_default())
                } else if data_type == "USER-DEFINED" {
                    udt_name
                } else {
                    data_type
                };
                
                ColumnInfo {
                    name: row.get(0),
                    data_type: display_type,
                    is_nullable: row.get::<_, String>(2) == "YES",
                    default_value: row.get(3),
                }
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

    fn is_builtin_type(type_name: &str) -> bool {
        matches!(type_name,
            "bool" | "bytea" | "char" | "name" | "int8" | "int2" | "int2vector" |
            "int4" | "regproc" | "text" | "oid" | "tid" | "xid" | "cid" |
            "oidvector" | "pg_type" | "pg_attribute" | "pg_proc" | "pg_class" |
            "json" | "xml" | "pg_node_tree" | "smgr" | "point" | "lseg" |
            "path" | "box" | "polygon" | "line" | "float4" | "float8" |
            "abstime" | "reltime" | "tinterval" | "unknown" | "circle" |
            "cash" | "macaddr" | "inet" | "cidr" | "aclitem" | "bpchar" |
            "varchar" | "date" | "time" | "timestamp" | "timestamptz" |
            "interval" | "timetz" | "bit" | "varbit" | "numeric" | "refcursor" |
            "regprocedure" | "regoper" | "regoperator" | "regclass" | "regtype" |
            "uuid" | "txid_snapshot" | "pg_lsn" | "tsvector" | "tsquery" |
            "gtsvector" | "regconfig" | "regdictionary" | "jsonb" | "int4range" |
            "numrange" | "tsrange" | "tstzrange" | "daterange" | "int8range" |
            "record" | "cstring" | "any" | "anyarray" | "void" | "trigger" |
            "language_handler" | "internal" | "opaque" | "anyelement" | "anynonarray" |
            "anyenum" | "fdw_handler" | "anyrange" | "pg_ddl_command"
        )
    }

    fn format_cell_value(row: &Row, index: usize) -> String {
        // Handle different PostgreSQL types
        
        // Get column type information for better formatting
        let column_type = row.columns().get(index).map(|col| col.type_());
        
        // String types (TEXT, VARCHAR, CHAR, etc.) and ENUMs
        if let Ok(val) = row.try_get::<_, Option<String>>(index) {
            let result = val.unwrap_or_else(|| "NULL".to_string());
            
            // Special formatting for enum types
            if let Some(col_type) = column_type {
                let type_name = col_type.name();
                // Check if this might be an enum type (custom types are usually enums)
                if !Self::is_builtin_type(type_name) && result != "NULL" {
                    return format!("{} ({})", result, type_name);
                }
            }
            
            return result;
        }
        
        // UUID type
        if let Ok(val) = row.try_get::<_, Option<uuid::Uuid>>(index) {
            return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        // Timestamp types
        if let Ok(val) = row.try_get::<_, Option<DateTime<Utc>>>(index) {
            return val.map(|v| v.format("%Y-%m-%d %H:%M:%S UTC").to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        if let Ok(val) = row.try_get::<_, Option<NaiveDateTime>>(index) {
            return val.map(|v| v.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        if let Ok(val) = row.try_get::<_, Option<NaiveDate>>(index) {
            return val.map(|v| v.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        if let Ok(val) = row.try_get::<_, Option<NaiveTime>>(index) {
            return val.map(|v| v.format("%H:%M:%S").to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        // Numeric types
        if let Ok(val) = row.try_get::<_, Option<i16>>(index) {
            return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        if let Ok(val) = row.try_get::<_, Option<i32>>(index) {
            return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        if let Ok(val) = row.try_get::<_, Option<i64>>(index) {
            return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        // Floating point types
        if let Ok(val) = row.try_get::<_, Option<f32>>(index) {
            return val.map(|v| {
                if v.fract() == 0.0 && v.abs() < 1e10 {
                    format!("{:.0}", v)
                } else {
                    format!("{}", v)
                }
            }).unwrap_or_else(|| "NULL".to_string());
        }
        
        if let Ok(val) = row.try_get::<_, Option<f64>>(index) {
            return val.map(|v| {
                if v.fract() == 0.0 && v.abs() < 1e15 {
                    format!("{:.0}", v)
                } else {
                    format!("{}", v)
                }
            }).unwrap_or_else(|| "NULL".to_string());
        }
        
        // Decimal/Numeric type (handle as string for now)
        // PostgreSQL NUMERIC/DECIMAL types can be very large, so we'll handle them as strings
        
        // Boolean type
        if let Ok(val) = row.try_get::<_, Option<bool>>(index) {
            return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        // Byte arrays (BYTEA)
        if let Ok(val) = row.try_get::<_, Option<Vec<u8>>>(index) {
            return val.map(|v| {
                if v.len() <= 16 {
                    format!("\\x{}", hex::encode(&v))
                } else {
                    format!("\\x{}... ({} bytes)", hex::encode(&v[..8]), v.len())
                }
            }).unwrap_or_else(|| "NULL".to_string());
        }
        
        // JSON/JSONB types
        if let Ok(val) = row.try_get::<_, Option<serde_json::Value>>(index) {
            return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
        }
        
        // Array types (basic support)
        if let Ok(val) = row.try_get::<_, Option<Vec<String>>>(index) {
            return val.map(|v| format!("[{}]", v.join(", "))).unwrap_or_else(|| "NULL".to_string());
        }
        
        if let Ok(val) = row.try_get::<_, Option<Vec<i32>>>(index) {
            return val.map(|v| format!("[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))).unwrap_or_else(|| "NULL".to_string());
        }
        
        // Fallback: try to handle as enum or unknown type
        if let Some(column) = row.columns().get(index) {
            let type_name = column.type_().name();
            
            // For custom types (like enums), try to get them as strings
            if !Self::is_builtin_type(type_name) {
                // Try to get the raw value as a string representation
                if let Ok(val) = row.try_get::<_, Option<&str>>(index) {
                    if let Some(str_val) = val {
                        return format!("{} ({})", str_val, type_name);
                    } else {
                        return "NULL".to_string();
                    }
                }
                
                // If that fails, try getting it as bytes and convert to string
                if let Ok(val) = row.try_get::<_, Option<&[u8]>>(index) {
                    if let Some(bytes) = val {
                        if let Ok(str_val) = std::str::from_utf8(bytes) {
                            return format!("{} ({})", str_val, type_name);
                        }
                    } else {
                        return "NULL".to_string();
                    }
                }
            }
            
            format!("<{}> (unsupported type)", type_name)
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

        // Top panel with connect/disconnect button
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.current_connection.is_some() {
                    if Self::icon_button(ui, "disconnect", "Disconnect").clicked() {
                        // Disconnect from database
                        self.current_connection = None;
                        self.db_sender = None;
                        self.tables.clear();
                        self.tabs.clear();
                        self.active_tab_index = None;
                        self.loading = false;
                        // Restart the database worker for future connections
                        self.start_database_worker(ctx);
                    }
                } else {
                    if Self::icon_button(ui, "connect", "Connect").clicked() {
                        self.show_connection_wizard = true;
                        self.connection_wizard = ConnectionWizard::default();
                    }
                }

                if let Some(conn) = &self.current_connection {
                    ui.separator();
                    ui.horizontal(|ui| {
                        Self::draw_icon(ui, "connect", 16.0, ui.style().visuals.text_color());
                        ui.label(format!("Connected to: {}", conn.name));
                    });
                }

                if self.loading {
                    ui.separator();
                    ui.spinner();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if Self::icon_button(ui, "add", "New Query").clicked() {
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
            ui.horizontal(|ui| {
                Self::draw_icon(ui, "table", 16.0, ui.style().visuals.text_color());
                ui.heading("Tables");
            });
            ui.separator();

            if self.current_connection.is_some() {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for table in &self.tables.clone() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                Self::draw_icon(ui, "table", 14.0, ui.style().visuals.text_color());
                                ui.label(format!("{}.{}", table.schema, table.name));
                            });
                            
                            ui.horizontal(|ui| {
                                if Self::small_icon_button(ui, "data", "Data").clicked() {
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
                                
                                if Self::small_icon_button(ui, "schema", "Schema").clicked() {
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
                    ui.horizontal(|ui| {
                        Self::draw_icon(ui, "error", 16.0, ui.style().visuals.text_color());
                        ui.label("No connection");
                    });
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
                            
                            if Self::icon_only_button(ui, "close", "Close tab").clicked() {
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
                                ui.horizontal(|ui| {
                                    ui.add(egui::Image::from_bytes("data_heading_icon", DATA_ICON)
                                        .fit_to_exact_size(egui::Vec2::new(18.0, 18.0)));
                                    ui.heading(format!("Data: {}.{}", table.schema, table.name));
                                });
                                
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
                                ui.horizontal(|ui| {
                                    ui.add(egui::Image::from_bytes("schema_heading_icon", SCHEMA_ICON)
                                        .fit_to_exact_size(egui::Vec2::new(18.0, 18.0)));
                                    ui.heading(format!("Schema: {}.{}", table.schema, table.name));
                                });
                                
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
                                ui.horizontal(|ui| {
                                    ui.add(egui::Image::from_bytes("query_heading_icon", QUERY_ICON)
                                        .fit_to_exact_size(egui::Vec2::new(18.0, 18.0)));
                                    ui.heading("Query");
                                });
                                
                                ui.horizontal(|ui| {
                                    if Self::icon_button(ui, PLAY_ICON, "Execute").clicked() && !sql.is_empty() {
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
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Image::from_bytes("error_result_icon", ERROR_ICON)
                                            .fit_to_exact_size(egui::Vec2::new(16.0, 16.0)));
                                        ui.colored_label(egui::Color32::RED, format!("Error: {}", error_msg));
                                    });
                                } else if let Some(data) = results {
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Image::from_bytes("results_icon", RESULTS_ICON)
                                            .fit_to_exact_size(egui::Vec2::new(16.0, 16.0)));
                                        ui.label(format!("Results ({} rows):", data.len()));
                                    });
                                    
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
                        ui.heading("PostgreSQL GUI");
                        ui.add_space(20.0);
                        ui.label("Welcome! Get started by:");
                        ui.horizontal(|ui| {
                            ui.add(egui::Image::from_bytes("welcome_connect_icon", CONNECT_ICON)
                                .fit_to_exact_size(egui::Vec2::new(16.0, 16.0)));
                            ui.label("1. Connecting to a database");
                        });
                        ui.horizontal(|ui| {
                            ui.add(egui::Image::from_bytes("welcome_tables_icon", TABLE_ICON)
                                .fit_to_exact_size(egui::Vec2::new(16.0, 16.0)));
                            ui.label("2. Browsing tables in the sidebar");
                        });
                        ui.horizontal(|ui| {
                            ui.add(egui::Image::from_bytes("welcome_query_icon", ADD_ICON)
                                .fit_to_exact_size(egui::Vec2::new(16.0, 16.0)));
                            ui.label("3. Creating a new query tab");
                        });
                    });
                });
            }
        });

        // Connection wizard modal
        if self.show_connection_wizard {
            egui::Window::new("Connection Manager")
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

impl PostgresGuiApp {
    fn show_connection_wizard_ui(&mut self, ui: &mut egui::Ui) {
        // Connection list
        ui.horizontal(|ui| {
            ui.add(egui::Image::from_bytes("saved_connections_icon", SAVE_ICON)
                .fit_to_exact_size(egui::Vec2::new(18.0, 18.0)));
            ui.heading("Saved Connections");
        });
        ui.separator();

        let mut connection_to_edit = None;
        let mut connection_to_delete = None;
        let mut connection_to_connect = None;

        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for conn in &self.connections {
                ui.group(|ui| {
                    let response = ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(&conn.name);
                            ui.label(&conn.url);
                        });
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if Self::icon_button(ui, CONNECT_ICON, "Connect").clicked() {
                                connection_to_connect = Some(conn.clone());
                            }

                            if Self::icon_button(ui, DELETE_ICON, "Delete").clicked() {
                                connection_to_delete = Some(conn.id.clone());
                            }

                            if Self::icon_button(ui, EDIT_ICON, "Edit").clicked() {
                                connection_to_edit = Some(conn.clone());
                            }
                        });
                    });
                    
                    // Handle double-click to connect
                    if response.response.double_clicked() {
                        connection_to_connect = Some(conn.clone());
                    }
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
        ui.horizontal(|ui| {
            let icon = if self.connection_wizard.editing_id.is_some() { EDIT_ICON } else { ADD_ICON };
            let text = if self.connection_wizard.editing_id.is_some() { "Edit Connection" } else { "New Connection" };
            ui.add(egui::Image::from_bytes("connection_form_icon", icon)
                .fit_to_exact_size(egui::Vec2::new(18.0, 18.0)));
            ui.heading(text);
        });

        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.connection_wizard.name);
        });

        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.text_edit_singleline(&mut self.connection_wizard.url);
            
            if ui.button("Paste example").clicked() {
                self.connection_wizard.url = "postgres://postgres:postgres@localhost:5432/database".to_string();
            }
        });

        ui.collapsing("Connection URL Examples", |ui| {
            ui.label("Local: postgres://username:password@localhost:5432/database");
            ui.label("Remote: postgres://user:pass@host:port/db");
            ui.label("With SSL: postgres://user:pass@host:port/db?sslmode=require");
        });

        ui.horizontal(|ui| {
            if Self::icon_button(ui, TEST_ICON, "Test Connection").clicked() && !self.connection_wizard.url.is_empty() {
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
            if Self::icon_button(ui, SAVE_ICON, "Save").clicked() && !self.connection_wizard.name.is_empty() && !self.connection_wizard.url.is_empty() {
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

            if Self::icon_button(ui, CANCEL_ICON, "Cancel").clicked() {
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

// PostgreSQL enum types are now fully supported!
// Enum values will be displayed as "Home (team_type)" in table data
// Enum schemas will show "team_type (Home, Away)" in schema view

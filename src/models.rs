use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConnection {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub enum TabContent {
    TableData { 
        table: TableInfo, 
        data: Vec<HashMap<String, String>>,
        columns: Vec<String>,
        loading: bool,
        total_rows: Option<i64>,
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
pub struct Tab {
    pub id: String,
    pub title: String,
    pub content: TabContent,
}

#[derive(Debug)]
pub enum DatabaseMessage {
    Connect(DatabaseConnection),
    LoadTableData(TableInfo, String), // table, tab_id
    LoadTableSchema(TableInfo, String), // table, tab_id
    ExecuteQuery(String, String), // sql, tab_id
    TestConnection(String), // url
    GetTableRowCount(TableInfo, String), // table, tab_id
}

#[derive(Debug)]
pub enum DatabaseResponse {
    Connected(Vec<TableInfo>),
    TableDataLoaded(String, Vec<HashMap<String, String>>, Vec<String>), // tab_id, data, columns
    TableSchemaLoaded(String, Vec<ColumnInfo>), // tab_id, columns
    QueryResult(String, Vec<HashMap<String, String>>, Vec<String>), // tab_id, data, columns
    Error(String),
    ConnectionTestResult(bool, String),
    TableRowCount(String, i64), // tab_id, count
}

#[derive(Default)]
pub struct ConnectionWizard {
    pub editing_id: Option<String>,
    pub name: String,
    pub url: String,
    pub test_result: Option<String>,
    pub testing: bool,
}

pub struct PostgresGuiApp {
    pub connections: Vec<DatabaseConnection>,
    pub show_connection_wizard: bool,
    pub connection_wizard: ConnectionWizard,
    pub current_connection: Option<DatabaseConnection>,
    pub tables: Vec<TableInfo>,
    pub tabs: Vec<Tab>,
    pub active_tab_index: Option<usize>,
    pub loading: bool,
    pub error_message: Option<String>,
    
    // Async communication
    pub db_sender: Option<mpsc::UnboundedSender<DatabaseMessage>>,
    pub db_receiver: mpsc::UnboundedReceiver<DatabaseResponse>,
} 
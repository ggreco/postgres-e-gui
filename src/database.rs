use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_postgres::{Client, NoTls, Row};
use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, NaiveDate, NaiveTime, Utc};

use crate::models::{TableInfo, ColumnInfo, DatabaseMessage, DatabaseResponse};

pub async fn start_database_worker(
    mut db_receiver: mpsc::UnboundedReceiver<DatabaseMessage>,
    response_sender: mpsc::UnboundedSender<DatabaseResponse>,
    ctx: eframe::egui::Context,
) {
    let mut client: Option<Arc<Mutex<Client>>> = None;

    while let Some(message) = db_receiver.recv().await {
        let response = match message {
            DatabaseMessage::Connect(conn) => {
                match connect_to_database(&conn.url).await {
                    Ok(new_client) => {
                        client = Some(Arc::new(Mutex::new(new_client)));
                        match load_tables(&client).await {
                            Ok(tables) => DatabaseResponse::Connected(tables),
                            Err(e) => DatabaseResponse::Error(format!("Failed to load tables: {}", e)),
                        }
                    }
                    Err(e) => DatabaseResponse::Error(format!("Connection failed: {}", e)),
                }
            }
            DatabaseMessage::LoadTableData(table, tab_id) => {
                if let Some(ref client) = client {
                    match load_table_data(client, &table).await {
                        Ok((data, columns)) => DatabaseResponse::TableDataLoaded(tab_id, data, columns),
                        Err(e) => DatabaseResponse::Error(format!("Failed to load table data: {}", e)),
                    }
                } else {
                    DatabaseResponse::Error("Not connected to database".to_string())
                }
            }
            DatabaseMessage::LoadTableSchema(table, tab_id) => {
                if let Some(ref client) = client {
                    match load_table_schema(client, &table).await {
                        Ok(columns) => DatabaseResponse::TableSchemaLoaded(tab_id, columns),
                        Err(e) => DatabaseResponse::Error(format!("Failed to load table schema: {}", e)),
                    }
                } else {
                    DatabaseResponse::Error("Not connected to database".to_string())
                }
            }
            DatabaseMessage::ExecuteQuery(sql, tab_id) => {
                if let Some(ref client) = client {
                    match execute_query(client, &sql).await {
                        Ok((data, columns)) => DatabaseResponse::QueryResult(tab_id, data, columns),
                        Err(e) => DatabaseResponse::Error(format!("Query failed: {}", e)),
                    }
                } else {
                    DatabaseResponse::Error("Not connected to database".to_string())
                }
            }
            DatabaseMessage::TestConnection(url) => {
                match test_connection(&url).await {
                    Ok(_) => DatabaseResponse::ConnectionTestResult(true, "Connection successful!".to_string()),
                    Err(e) => DatabaseResponse::ConnectionTestResult(false, format!("Connection failed: {}", e)),
                }
            }
            DatabaseMessage::GetTableRowCount(table, tab_id) => {
                if let Some(ref client) = client {
                    match get_table_row_count(client, &table).await {
                        Ok(count) => DatabaseResponse::TableRowCount(tab_id, count),
                        Err(e) => DatabaseResponse::Error(format!("Failed to get row count: {}", e)),
                    }
                } else {
                    DatabaseResponse::Error("Not connected to database".to_string())
                }
            }
        };

        if response_sender.send(response).is_err() {
            break;
        }
        ctx.request_repaint();
    }
}

async fn connect_to_database(url: &str) -> Result<Client> {
    let (client, connection) = tokio_postgres::connect(url, NoTls).await?;
    
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Database connection error: {}", e);
        }
    });
    
    Ok(client)
}

async fn test_connection(url: &str) -> Result<()> {
    let (client, connection) = tokio_postgres::connect(url, NoTls).await?;
    
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Test connection error: {}", e);
        }
    });
    
    let _ = client.simple_query("SELECT 1").await?;
    Ok(())
}

async fn load_tables(client: &Option<Arc<Mutex<Client>>>) -> Result<Vec<TableInfo>> {
    if let Some(client) = client {
        let client = client.lock().await;
        let rows = client.query(
            "SELECT schemaname, tablename 
             FROM pg_tables 
             WHERE schemaname NOT IN ('information_schema', 'pg_catalog', 'pg_toast')
             ORDER BY schemaname, tablename",
            &[]
        ).await?;
        
        let tables = rows.into_iter().map(|row| TableInfo {
            schema: row.get(0),
            name: row.get(1),
        }).collect();
        
        Ok(tables)
    } else {
        Err(anyhow::anyhow!("No database connection"))
    }
}

async fn load_table_data(client: &Arc<Mutex<Client>>, table: &TableInfo) -> Result<(Vec<HashMap<String, String>>, Vec<String>)> {
    let client = client.lock().await;
    
    // First, get column information to detect enum types
    let column_query = "
        SELECT c.column_name, c.data_type, t.typname as user_type_name
        FROM information_schema.columns c
        LEFT JOIN pg_type t ON c.udt_name = t.typname
        WHERE c.table_schema = $1 AND c.table_name = $2
        ORDER BY c.ordinal_position";
    
    let column_rows = client.query(column_query, &[&table.schema, &table.name]).await?;
    
    // Build the SELECT query with proper casting for enum types
    let mut select_parts = Vec::new();
    let mut columns = Vec::new();
    
    for row in &column_rows {
        let column_name: String = row.get(0);
        let data_type: String = row.get(1);
        let user_type_name: Option<String> = row.get(2);
        
        columns.push(column_name.clone());
        
        // Check if this is a user-defined type (likely an enum)
        if data_type == "USER-DEFINED" && user_type_name.is_some() {
            // Cast enum to text for proper display
            select_parts.push(format!("{}::text", column_name));
        } else {
            select_parts.push(column_name);
        }
    }
    
    let select_clause = select_parts.join(", ");
    let query = format!("SELECT {} FROM {}.{} LIMIT 1000", select_clause, table.schema, table.name);
    
    let rows = client.query(&query, &[]).await?;
    
    let mut data = Vec::new();
    for row in rows {
        let mut row_data = HashMap::new();
        for (i, column) in columns.iter().enumerate() {
            let value = format_cell_value(&row, i);
            row_data.insert(column.clone(), value);
        }
        data.push(row_data);
    }
    
    Ok((data, columns))
}

async fn load_table_schema(client: &Arc<Mutex<Client>>, table: &TableInfo) -> Result<Vec<ColumnInfo>> {
    let client = client.lock().await;
    
    // Enhanced query to get enum values for user-defined types
    let query = "
        SELECT 
            c.column_name,
            CASE 
                WHEN c.data_type = 'USER-DEFINED' THEN 
                    c.udt_name || ' (' || 
                    COALESCE(
                        (SELECT string_agg(e.enumlabel, ', ' ORDER BY e.enumsortorder)
                         FROM pg_type t 
                         JOIN pg_enum e ON t.oid = e.enumtypid 
                         WHERE t.typname = c.udt_name), 
                        'enum'
                    ) || ')'
                ELSE c.data_type 
            END as data_type,
            c.is_nullable,
            c.column_default
        FROM information_schema.columns c
        WHERE c.table_schema = $1 AND c.table_name = $2
        ORDER BY c.ordinal_position";
    
    let rows = client.query(query, &[&table.schema, &table.name]).await?;
    
    let columns = rows.into_iter().map(|row| ColumnInfo {
        name: row.get(0),
        data_type: row.get(1),
        is_nullable: row.get::<_, String>(2) == "YES",
        default_value: row.get(3),
    }).collect();
    
    Ok(columns)
}

async fn execute_query(client: &Arc<Mutex<Client>>, sql: &str) -> Result<(Vec<HashMap<String, String>>, Vec<String>)> {
    let client = client.lock().await;
    let rows = client.query(sql, &[]).await?;
    
    if rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    
    let columns: Vec<String> = rows[0].columns().iter().map(|col| col.name().to_string()).collect();
    
    let mut data = Vec::new();
    for row in rows {
        let mut row_data = HashMap::new();
        for (i, column) in columns.iter().enumerate() {
            let value = format_cell_value(&row, i);
            row_data.insert(column.clone(), value);
        }
        data.push(row_data);
    }
    
    Ok((data, columns))
}

async fn get_table_row_count(client: &Arc<Mutex<Client>>, table: &TableInfo) -> Result<i64> {
    let client = client.lock().await;
    let query = format!("SELECT COUNT(*) FROM {}.{}", table.schema, table.name);
    let rows = client.query(&query, &[]).await?;
    
    if let Some(row) = rows.first() {
        let count: i64 = row.get(0);
        Ok(count)
    } else {
        Ok(0)
    }
}

fn is_builtin_type(type_name: &str) -> bool {
    matches!(type_name.to_lowercase().as_str(),
        "boolean" | "bool" |
        "smallint" | "int2" |
        "integer" | "int" | "int4" |
        "bigint" | "int8" |
        "decimal" | "numeric" |
        "real" | "float4" |
        "double precision" | "float8" |
        "smallserial" | "serial2" |
        "serial" | "serial4" |
        "bigserial" | "serial8" |
        "money" |
        "character varying" | "varchar" |
        "character" | "char" |
        "text" |
        "bytea" |
        "timestamp" | "timestamp without time zone" |
        "timestamp with time zone" | "timestamptz" |
        "date" |
        "time" | "time without time zone" |
        "time with time zone" | "timetz" |
        "interval" |
        "uuid" |
        "json" |
        "jsonb" |
        "xml" |
        "inet" |
        "cidr" |
        "macaddr" |
        "macaddr8"
    )
}

pub fn format_cell_value(row: &Row, index: usize) -> String {
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
            if !is_builtin_type(type_name) && result != "NULL" {
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
        if !is_builtin_type(type_name) {
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
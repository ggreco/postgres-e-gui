# PostgreSQL E-GUI

A modern PostgreSQL database GUI built with Rust and egui, inspired by Sequel Pro for Mac.

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![PostgreSQL](https://img.shields.io/badge/postgresql-%23316192.svg?style=for-the-badge&logo=postgresql&logoColor=white)

## Features

### 🔌 Connection Management
- **Connection Wizard**: Easy-to-use modal for creating database connections
- **Save/Load Connections**: Persistent storage of connection configurations
- **Double-click to Connect**: Quick connection from saved configurations
- **Test Connection**: Verify connection settings before saving

### 📊 Database Browser
- **Table Browser**: Sidebar with expandable database schema
- **Tabbed Interface**: Separate tabs for Data, Schema, and Query views
- **Real-time Data**: Live table data with proper PostgreSQL type support
- **Row Count Status**: Shows current rows displayed and total table rows

### 🗃️ Data Types Support
- **UUID**: Proper UUID formatting and display
- **Timestamps**: TIMESTAMPTZ, TIMESTAMP, DATE, TIME with readable formatting
- **Numeric Types**: Smart formatting for integers, floats, and decimals
- **Enums**: Custom PostgreSQL enum types with value display
- **Arrays**: PostgreSQL array type support
- **JSON/JSONB**: Formatted JSON data display
- **Binary Data**: Hex representation of BYTEA fields

### 🎨 Modern UI
- **Material Design Icons**: Clean, professional icon set
- **Dark/Light Theme**: Automatic theme support via egui
- **Responsive Layout**: Adapts to different window sizes
- **Icon Caching**: Optimized performance with pointer-based icon cache
- **Two-line Table Layout**: Prevents UI overlap on smaller windows

### ⚡ Performance
- **Async Operations**: Non-blocking database queries using tokio
- **Efficient Rendering**: Icon caching and optimized UI updates
- **Memory Management**: Smart data handling for large result sets

## Installation

### Prerequisites
- Rust (latest stable version)
- PostgreSQL database to connect to

### Building from Source
```bash
git clone https://github.com/ggreco/postgres-e-gui.git
cd postgres-e-gui
cargo build --release
```

### Running
```bash
cargo run
```

## Usage

### First Time Setup
1. Launch the application
2. Click the "Connect" button to open the connection wizard
3. Fill in your PostgreSQL connection details:
   - **Name**: A friendly name for your connection
   - **URL**: PostgreSQL connection string (e.g., `postgres://user:password@host:port/database`)
4. Click "Test Connection" to verify
5. Click "Save" to store the connection

### Connecting to Database
- **From Connection Wizard**: Click "Connect" after entering details
- **From Saved Connections**: Double-click any saved connection
- **Quick Connect**: Select a connection and click "Connect"

### Browsing Data
1. **Table List**: Browse tables in the left sidebar
2. **Data Tab**: View table contents with pagination
3. **Schema Tab**: Inspect table structure and column details
4. **Query Tab**: Write and execute custom SQL queries

### Example Connection Strings
```
# Local PostgreSQL
postgres://postgres:password@localhost:5432/mydb

# Remote PostgreSQL
postgres://user:pass@example.com:5432/production_db

# With SSL
postgres://user:pass@host:5432/db?sslmode=require
```

## Architecture

The application is built with a modular architecture:

- **`src/main.rs`**: Application entry point and main UI logic
- **`src/models.rs`**: Data structures and type definitions
- **`src/database.rs`**: Async database operations and PostgreSQL integration
- **`src/config.rs`**: Configuration management and persistence
- **`src/ui/`**: UI components and utilities
  - **`icons.rs`**: Icon management with caching system
  - **`connection_wizard.rs`**: Connection dialog UI
  - **`mod.rs`**: UI module organization

## Dependencies

- **[eframe](https://crates.io/crates/eframe)**: egui framework for native apps
- **[egui](https://crates.io/crates/egui)**: Immediate mode GUI library
- **[tokio](https://crates.io/crates/tokio)**: Async runtime
- **[tokio-postgres](https://crates.io/crates/tokio-postgres)**: PostgreSQL driver
- **[serde](https://crates.io/crates/serde)**: Serialization framework
- **[uuid](https://crates.io/crates/uuid)**: UUID support
- **[chrono](https://crates.io/crates/chrono)**: Date/time handling
- **[dirs](https://crates.io/crates/dirs)**: Platform-specific directories
- **[anyhow](https://crates.io/crates/anyhow)**: Error handling
- **[hex](https://crates.io/crates/hex)**: Binary data encoding

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test thoroughly
5. Submit a pull request

## License

This project is open source. Please check the LICENSE file for details.

## Roadmap

- [ ] SQL syntax highlighting
- [ ] Query history
- [ ] Export data (CSV, JSON)
- [ ] Multiple database connections simultaneously
- [ ] Table editing capabilities
- [ ] Performance monitoring
- [ ] Connection pooling
- [ ] Custom themes

## Screenshots

*Screenshots coming soon...*

## Support

If you encounter any issues or have questions, please open an issue on GitHub.

---

Built with ❤️ in Rust

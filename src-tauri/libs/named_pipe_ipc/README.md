# Named Pipe IPC Library

A Rust library for inter-process communication using Windows Named Pipes with Tokio async runtime and optional ChaCha20Poly1305 encryption.

## Features

- **Async/Await Support**: Built on top of Tokio for high-performance async I/O
- **Encryption Support**: Optional ChaCha20Poly1305 AEAD encryption for secure communication
- **Flexible Key Management**: Use custom keys or secure compile-time generated default keys
- **Multiple Connections**: Server can handle multiple concurrent client connections
- **Error Handling**: Comprehensive error types with detailed error information
- **Type Safety**: Strongly typed interfaces for reliable communication
- **Connection Management**: Automatic connection lifecycle management

## Quick Start

### Adding to Your Project

Add this to your `Cargo.toml`:

```toml
[dependencies]
named_pipe_ipc = { path = "path/to/this/library" }
tokio = { version = "1.0", features = ["full"] }
# Optional: only if you want to serialize your own data
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Basic Server Example

```rust
use named_pipe_ipc::{NamedPipeServerStruct, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Create server without encryption
    let mut server = NamedPipeServerStruct::new("my_pipe");
    
    // Or create server with default encryption (compile-time generated key)
    let mut encrypted_server = NamedPipeServerStruct::new_encrypted("my_pipe", None);
    
    // Or create server with custom encryption key
    let custom_key = [1u8; 32]; // Your 32-byte key
    let mut custom_encrypted_server = NamedPipeServerStruct::new_encrypted("my_pipe", Some(custom_key));
    
    // Start the server with a connection handler
    server.start(|mut connection| async move {
        while let Ok(data) = connection.receive_bytes().await {
            println!("Received: {:?}", data);
            connection.send_bytes(b"Echo response").await?;
        }
        Ok(())
    }).await?;
    
    Ok(())
}
```

### Basic Client Example

```rust
use named_pipe_ipc::{NamedPipeClientStruct, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Create client without encryption
    let mut client = NamedPipeClientStruct::new("my_pipe");
    
    // Or create client with default encryption (compile-time generated key)
    let mut encrypted_client = NamedPipeClientStruct::new_encrypted("my_pipe", None);
    
    // Or create client with custom encryption key
    let custom_key = [1u8; 32]; // Your 32-byte key
    let mut custom_encrypted_client = NamedPipeClientStruct::new_encrypted("my_pipe", Some(&custom_key));
    
    // Connect and communicate
    client.connect().await?;
    client.send_bytes(b"Hello, server!").await?;
    let response = client.receive_bytes().await?;
    println!("Server responded: {:?}", response);
    
    Ok(())
}
```

## Encryption Features

The library provides three ways to use encryption:

### 1. No Encryption (Default)
```rust
let server = NamedPipeServerStruct::new("my_pipe");
let client = NamedPipeClientStruct::new("my_pipe");
```

### 2. Default Encryption (Compile-time Generated Key)
```rust
// Both server and client automatically use the same compile-time generated key
let server = NamedPipeServerStruct::new_encrypted("my_pipe", None);
let client = NamedPipeClientStruct::new_encrypted("my_pipe", None);
```

### 3. Custom Key Encryption
```rust
let custom_key = [1u8; 32]; // Your 32-byte key
let server = NamedPipeServerStruct::new_encrypted("my_pipe", Some(custom_key));
let client = NamedPipeClientStruct::new_encrypted("my_pipe", Some(&custom_key));
```

**Key Points:**
- 🔒 **Automatic**: Encryption/decryption is completely transparent
- 🚀 **Fast**: Uses ChaCha20Poly1305 AEAD cipher
- 🔐 **Secure**: Each message gets a unique random nonce  
- 📦 **Simple**: Same `send_bytes()`/`receive_bytes()` API
- 🔑 **Flexible**: Use default keys or provide your own

## API Documentation

### NamedPipeServerStruct

The server struct handles incoming connections and manages multiple clients.

#### Methods

- `new(pipe_name: &str) -> Self` - Create a new unencrypted server instance
- `new_encrypted(pipe_name: &str, key: Option<[u8; 32]>) -> Self` - Create a new encrypted server instance. If key is `None`, uses a compile-time generated default key. If key is `Some(key)`, uses the provided custom key.
- `start<F, Fut>(&mut self, handler: F) -> Result<()>` - Start the server with a connection handler
- `stop(&mut self) -> Result<()>` - Stop the server
- `is_running(&self) -> bool` - Check if server is running
- `pipe_name(&self) -> &str` - Get the pipe name

### NamedPipeClientStruct

The client struct for connecting to named pipe servers.

#### Methods

- `new(pipe_name: &str) -> Self` - Create a new unencrypted client instance
- `new_encrypted(pipe_name: &str, key: Option<&[u8; 32]>) -> Self` - Create a new encrypted client instance. If key is `None`, uses a compile-time generated default key. If key is `Some(key)`, uses the provided custom key.
- `connect(&mut self) -> Result<()>` - Connect to the server
- `send_bytes(&mut self, data: &[u8]) -> Result<()>` - Send raw bytes (automatically encrypted if encryption is enabled)
- `receive_bytes(&mut self) -> Result<Vec<u8>>` - Receive raw bytes (automatically decrypted if encryption is enabled)
- `is_connected(&self) -> bool` - Check if connected
- `disconnect(&mut self)` - Disconnect from server

### NamedPipeConnection

Represents a connection between server and client.

#### Methods

- `id(&self) -> usize` - Get connection ID
- `send_bytes(&mut self, data: &[u8]) -> Result<()>` - Send raw bytes (automatically encrypted if encryption is enabled)
- `receive_bytes(&mut self) -> Result<Vec<u8>>` - Receive raw bytes (automatically decrypted if encryption is enabled)

## Examples

The library comes with several examples demonstrating different usage patterns:

### Running Examples

1. **Basic Communication**:
   ```bash
   cargo run --example basic_communication
   ```

2. **Encryption Options** (shows both default and custom key encryption):
   ```bash
   cargo run --example encryption_options
   ```

3. **Event-Driven Server** (advanced server with callbacks and shutdown control):
   ```bash
   # Terminal 1: Start the server
   cargo run --example event_driven_server
   
   # Terminal 2: Test with client
   cargo run --example event_client
   ```

### Event-Driven Server Example

The event-driven server example demonstrates advanced features:

- **Event-based architecture** with callback handlers
- **Proper shutdown handling** using atomic variables
- **Connection lifecycle management** with detailed events
- **Message processing events** with timestamps
- **Graceful cleanup** and resource management

```rust
// Event handler trait for custom event processing
pub trait EventHandler: Send + Sync {
    fn handle_event(&self, event: ServerEvent);
}

// Server events
#[derive(Debug, Clone)]
pub enum ServerEvent {
    ClientConnected { client_id: usize, timestamp: u64 },
    ClientDisconnected { client_id: usize, timestamp: u64 },
    MessageReceived { client_id: usize, message: String, timestamp: u64 },
    ServerStarted { pipe_name: String, timestamp: u64 },
    ServerShutdown { timestamp: u64 },
    Error { client_id: Option<usize>, error: String, timestamp: u64 },
}

// Server state with shutdown control
pub struct ServerState {
    pub is_running: Arc<AtomicBool>,
    pub clients: Arc<Mutex<HashMap<usize, ClientInfo>>>,
    pub event_handler: Arc<dyn EventHandler>,
}

// Usage
let event_handler = Arc::new(ConsoleEventHandler);
let mut server = EventDrivenServer::new(event_handler);
let server_state = server.get_state().clone();

// Start server
server.start("my_pipe").await?;

// Shutdown from anywhere in your application
server_state.shutdown();
```

The event-driven server supports:
- **Controllable shutdown**: Use `server_state.shutdown()` to gracefully stop
- **Event callbacks**: Implement `EventHandler` for custom event processing
- **Connection tracking**: Monitor client connections and statistics
- **JSON and text messages**: Automatic message format detection
- **Error handling**: Detailed error events with client context

### Example: JSON Communication

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct MyMessage {
    id: u32,
    content: String,
}

// Server side
let message: MyMessage = connection.receive_json().await?;
connection.send_json(&MyMessage {
    id: message.id,
    content: format!("Processed: {}", message.content),
}).await?;

// Client side
client.send_json(&MyMessage {
    id: 1,
    content: "Hello".to_string(),
}).await?;
let response: MyMessage = client.receive_json().await?;
```

## Error Handling

The library provides comprehensive error handling through the `NamedPipeError` enum:

```rust
use named_pipe_ipc::{NamedPipeError, Result};

match client.connect().await {
    Ok(_) => println!("Connected successfully"),
    Err(NamedPipeError::Io(e)) => println!("IO error: {}", e),
    Err(NamedPipeError::NotConnected) => println!("Not connected"),
    Err(e) => println!("Other error: {}", e),
}
```

## Protocol Details

The library uses a simple length-prefixed protocol:

1. **Length Header**: 4 bytes (little-endian u32) indicating message length
2. **Message Data**: The actual message bytes

This ensures reliable message framing and prevents partial message reads.

## Platform Support

Currently supports Windows only (uses Windows Named Pipes). Future versions may add Unix domain socket support for cross-platform compatibility.

## Thread Safety

- Server can handle multiple concurrent connections
- Each connection runs in its own tokio task
- All operations are async and non-blocking

## Performance Considerations

- Messages are buffered and sent atomically
- JSON serialization adds overhead - use raw bytes for high-performance scenarios
- Connection pooling on client side may be beneficial for frequent communications

## Testing

Run the test suite:

```bash
cargo test
```

Run examples to test functionality:

```bash
# Terminal - Test with basic client
cargo run --example basic_communication
```

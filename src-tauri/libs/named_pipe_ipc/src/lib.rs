//! Named Pipe IPC Library
//! 
//! This library provides a simple and efficient way to implement inter-process communication
//! using Windows Named Pipes with Tokio async runtime.
//! 
//! # Features
//! 
//! - Async/await support using Tokio
//! - JSON serialization support
//! - Connection management
//! - Error handling
//! - Multiple connection support for servers
//! 
//! # Examples
//! 
//! ## Basic Server
//! 
//! ```rust,no_run
//! use named_pipe_ipc::NamedPipeServerStruct;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut server = NamedPipeServerStruct::new("my_pipe");
//!     
//!     server.start(|mut connection| async move {
//!         while let Ok(message) = connection.receive_string().await {
//!             println!("Received: {}", message);
//!             connection.send_string("Echo: ").await?;
//!         }
//!         Ok(())
//!     }).await?;
//!     
//!     Ok(())
//! }
//! ```
//! 
//! ## Basic Client
//! 
//! ```rust,no_run
//! use named_pipe_ipc::NamedPipeClientStruct;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = NamedPipeClientStruct::new("my_pipe");
//!     client.connect().await?;
//!     
//!     client.send_string("Hello, Server!").await?;
//!     let response = client.receive_string().await?;
//!     println!("Server responded: {}", response);
//!     
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod server;

#[cfg(test)]
mod tests;

pub use client::NamedPipeClientStruct;
pub use error::{NamedPipeError, Result};
pub use server::{NamedPipeConnection, NamedPipeServerStruct};

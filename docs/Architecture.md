# Fenris Architecture Documentation

## 1. System Overview

**Fenris** (Fast Encrypted Networked Robust Information Storage) is a secure, asynchronous, client-server file transfer system implemented in Rust. It emphasizes security, performance, and modularity, utilizing modern Rust practices and the Tokio async runtime.

The system consists of three main crates:
*   `client`: A TUI-based terminal client for interacting with the server.
*   `server`: A concurrent, asynchronous server that handles file operations.
*   `common`: Shared libraries for protocols, cryptography, compression, and file abstractions.

---

## 2. Common Library (`common`)

The `common` crate acts as the foundation for the system, defining how components communicate and secure data.

### 2.1 Protocol (`proto`)
Communication relies on **Protocol Buffers** (via `prost`) for strong typing and efficient serialization.
*   **Request**: Encapsulates commands (`RequestType`), target paths, and payloads.
*   **Response**: Encapsulates success status, error messages, data payloads, and structured details (like `FileInfo` or `DirectoryListing`).

### 2.2 Secure Channel (`secure_channel`)
The `SecureChannel` struct wraps a raw `TcpStream` to provide a secure transport layer. It handles the lifecycle of a connection:
1.  **Handshake**: Performs an **ECDH** (Elliptic-Curve Diffie-Hellman) key exchange using **X25519**.
2.  **Key Derivation**: Derives a symmetric session key from the shared secret using **HKDF-SHA256**.
3.  **Transport**: Implements a pipeline for every message:
    *   **Sending**: Serialize $\to$ Compress $\to$ Encrypt (Seal) $\to$ Frame $\to$ Send.
    *   **Receiving**: Receive Frame $\to$ Decrypt (Open) $\to$ Decompress $\to$ Deserialize.

### 2.3 Cryptography (`crypto`)
The system uses a trait-based approach to allow algorithm agility. Defaults (`DefaultCrypto`) are:
*   **Encryption**: **AES-256-GCM** (Authenticated Encryption).
*   **Key Exchange**: **X25519** (static-static or ephemeral-static depending on usage).
*   **Key Derivation**: **HKDF** over **SHA-256**.

### 2.4 File Operations (`file_ops`)
File system interaction is abstracted through the `FileOperations` trait.
*   **DefaultFileOperations**: The concrete implementation using `tokio::fs`.
*   **Security**: Implements path sanitization (traversal prevention) to ensure clients cannot access files outside the designated root directory.

---

## 3. Client Architecture

The client is a TUI (Terminal User Interface) application built with `ratatui` and `tokio`.

### 3.1 Component Structure

*   **Client (`client.rs`)**: The entry point and main event loop. It coordinates between user input (keyboard events) and network activity.
*   **App (`app.rs`)**: A state container for the UI. It holds the current screen, input buffers, command history, and message logs. It is strictly for *presentation state*.
*   **ConnectionManager (`connection_manager.rs`)**: Manages the `SecureChannel` lifecycle.
    *   Maintains the active connection state.
    *   Coordinates the **RequestManager** and **ResponseManager**.
*   **RequestManager (`request_manager.rs`)**: Parses user text input (e.g., `ls /tmp`) and constructs the appropriate Protobuf `Request` object.
*   **ResponseManager (`response_manager.rs`)**: Interprets the raw Protobuf `Response` from the server and formats it into user-friendly text or structured data for the `App` to display.

### 3.2 UI Layer (`ui/`)
*   **Screens**:
    *   `Connection`: Input form for server address and port.
    *   `Command`: The primary interface, featuring a command input line and a scrolling log of operation results.
    *   `Help`: A reference screen for available commands.
*   **Components**: Reusable widgets for headers, input fields, and message lists.

---

## 4. Server Architecture

The server is designed for high concurrency and robustness, built on the `tokio` runtime.

### 4.1 Core Components

*   **Server (`server.rs`)**: The main listener loop.
    *   Binds to a TCP port.
    *   Uses a `Semaphore` to enforce a configurable limit on concurrent connections.
    *   Spawns a new independent task for each accepted connection.
*   **Connection (`connection.rs`)**: Represents a single active client session.
    *   Performs the initial Handshake.
    *   Maintains session state (like the `current_dir` for relative paths).
    *   Runs a read-eval-print loop: Receives Request $\to$ Process $\to$ Send Response.
*   **RequestHandler (`request_handler.rs`)**: The business logic layer.
    *   Stateless (mostly) processor that dispatches `RequestType` to specific methods.
    *   Resolves paths against the session's current directory.
    *   Invokes the injected `FileOperations` implementation to touch the disk.

### 4.2 Configuration (`config.rs`)
The server supports a builder pattern for configuration, controlling:
*   Max connections.
*   Handshake timeouts (prevention of slow-loris attacks).
*   Idle timeouts (cleanup of inactive sessions).

---

## 5. Data Flow Example: `read file.txt`

1.  **Client User** types `read file.txt`.
2.  **Client `RequestManager`** parses this and builds a `Request { type: ReadFile, filename: "file.txt" }`.
3.  **Client `SecureChannel`** serializes, compresses, encrypts, and sends the packet.
4.  **Server `Connection`** receives, decrypts, and deserializes the request.
5.  **Server `RequestHandler`** resolves `file.txt` against the current directory (e.g., `/root/user/`).
6.  **Server `FileOperations`** reads the bytes from disk (checking for path traversal).
7.  **Server** constructs a `Response` containing the file bytes.
8.  **Server `SecureChannel`** encrypts and sends the response back.
9.  **Client `ResponseManager`** formats the received bytes (e.g., showing a preview or saving to disk).
10. **Client `App`** updates the UI log with "File read successfully".
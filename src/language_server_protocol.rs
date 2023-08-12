use lsp_types::{ClientCapabilities, InitializeParams, ServerCapabilities};
use std::error::Error;

use lsp_server::{Connection, Message, Request, RequestId, Response};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // ... Run main loop ...

    Ok(())
}

pub(crate) fn start() {
    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server
    let (id, params) = connection.initialize_start().unwrap();

    let init_params: InitializeParams = serde_json::from_value(params).unwrap();
    let client_capabilities: ClientCapabilities = init_params.capabilities;
    let server_capabilities = ServerCapabilities::default();

    let initialize_data = serde_json::json!({
        "capabilities": server_capabilities,
        "serverInfo": {
            "name": "Phanalist",
            "version": "0.1"
       }
    });

    connection.initialize_finish(id, initialize_data).unwrap();
}

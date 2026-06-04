//! mcp/tools — MCP tool route definitions.
//!
//! Each submodule exposes one or more `route()` functions returning a
//! [`ToolRoute`] ready to be registered on the server.

pub mod read;
pub mod references;
pub mod search;
pub mod shell;
pub mod shell_output;
pub mod status;

pub(crate) use super::BuiltinServer;

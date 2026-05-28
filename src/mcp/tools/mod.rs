//! mcp/tools — MCP tool route definitions.
//!
//! Each submodule exposes one or more `route()` functions returning a
//! [`ToolRoute`] ready to be registered on the server.

pub mod events;
pub mod read;
pub mod search;
pub mod status;
pub mod unwatch;
pub mod watch;

pub(crate) use super::BuiltinServer;

//! MCPStore PyO3 Bindings
//!
//! Unified Python interface to the Rust core.
//! Exposes:
//! - MCPStore
//! - PerspectiveResolver
//!
//! Built with PyO3 + maturin. Target module name: `mcpstore._rust`

use pyo3::prelude::*;

mod core_store;
mod perspective;

/// Python module initialization.
#[pymodule]
fn _rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    tracing_subscriber::fmt::init();

    m.add_class::<core_store::PyMCPStore>()?;
    m.add_class::<perspective::PyPerspectiveResolver>()?;

    Ok(())
}

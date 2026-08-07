//! MCPStore PyO3 Bindings
//!
//! Unified Python interface to the Rust core.
//! Exposes:
//! - MCPStore (sync) + AsyncMCPStore (async)
//! - PerspectiveResolver
//!
//! Built with PyO3 + maturin. Target module name: `mcpstore._rust`

use pyo3::prelude::*;

mod async_store;
mod core_store;
mod perspective;
mod py_value;

/// Python module initialization.
#[pymodule]
fn _rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    tracing_subscriber::fmt::init();

    m.add_class::<core_store::PyMCPStore>()?;
    m.add_class::<core_store::PyScopeContext>()?;
    m.add_class::<core_store::PyService>()?;
    m.add_class::<core_store::PyTool>()?;
    m.add_class::<core_store::PyPrompt>()?;
    m.add_class::<core_store::PyResource>()?;
    m.add_class::<core_store::PyResourceTemplate>()?;
    m.add_class::<async_store::PyAsyncMCPStore>()?;
    m.add_class::<async_store::PyAsyncScopeContext>()?;
    m.add_class::<async_store::PyAsyncService>()?;
    m.add_class::<async_store::PyAsyncTool>()?;
    m.add_class::<async_store::PyAsyncPrompt>()?;
    m.add_class::<async_store::PyAsyncResource>()?;
    m.add_class::<async_store::PyAsyncResourceTemplate>()?;
    m.add_class::<perspective::PyPerspectiveResolver>()?;

    Ok(())
}

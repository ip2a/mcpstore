mod control_plane;
mod execution;
mod persistence;
mod runtime_state;

pub(crate) use control_plane::ControlPlane;
pub(crate) use execution::ExecutionEngine;
pub(crate) use persistence::PersistenceRouter;
pub(crate) use runtime_state::RuntimeState;

pub(crate) struct StoreKernel {
    pub(crate) execution: ExecutionEngine,
    pub(crate) control: ControlPlane,
    pub(crate) persistence: PersistenceRouter,
    pub(crate) runtime: RuntimeState,
}

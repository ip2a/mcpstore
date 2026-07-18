mod config;
mod coordinator;
mod credentials;
mod key;
mod keyring;
mod lifecycle;
mod models;
mod state;

pub use config::{
    AuthConfig, AuthorizationCodeClientAuthMethod, ClientCredentialsAuthMethod,
    JwtSigningAlgorithm, OAuthAuthorizationCodeConfig, OAuthClientCredentialsConfig,
};
#[cfg(test)]
pub(crate) use coordinator::test_state_manager;
pub use coordinator::AuthCoordinator;
pub use credentials::{
    ClientSecret, KeyringClientSecretStore, KeyringCredentialStore, KeyringPrivateKeyStore,
    PrivateKey,
};
pub use key::AuthCredentialKey;
pub(crate) use keyring::SystemKeyring;
pub use models::{
    AuthError, AuthFlow, AuthRequired, AuthStatus, AuthStatusView, AuthorizationStart,
};
pub use state::KeyringStateStore;

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

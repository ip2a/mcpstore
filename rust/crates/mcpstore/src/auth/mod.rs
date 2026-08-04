mod callback_listener;
mod config;
mod coordinator;
mod credentials;
mod discovery;
mod key;
mod keyring;
mod lifecycle;
mod models;
mod state;

pub use callback_listener::{LocalCallbackListener, OAuthCallback};
pub use config::{
    default_oauth_redirect_uri, AuthConfig, AuthorizationCodeClientAuthMethod,
    ClientCredentialsAuthMethod, JwtSigningAlgorithm, OAuthAuthorizationCodeConfig,
    OAuthClientCredentialsConfig, DEFAULT_OAUTH_REDIRECT_URI,
};
#[cfg(test)]
pub(crate) use coordinator::test_state_manager;
pub use coordinator::AuthCoordinator;
pub use credentials::{
    ClientSecret, KeyringClientSecretStore, KeyringCredentialStore, KeyringPrivateKeyStore,
    PrivateKey,
};
pub use discovery::http_endpoint_requires_oauth;
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

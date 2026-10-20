//! Altertable authentication: translates a `profiles.yml` target into ADBC database
//! options for the Apache Arrow Flight SQL driver (`adbc_driver_flightsql`).
//!
//! Profile keys mirror the Python `dbt-altertable` adapter so existing profiles keep
//! working unchanged:
//!
//! ```yaml
//! type: altertable
//! host: flight.altertable.ai  # default
//! port: 443                   # default
//! username: dbt
//! password: secret
//! database: analytics         # alias: catalog
//! schema: main
//! tls: true                   # default
//! ```
use crate::{AdapterConfig, Auth, AuthError, AuthWarningPrinter, auth_configure_pipeline};
use database::Builder as DatabaseBuilder;
use dbt_adbc::{Backend, database};
use std::borrow::Cow;

const DEFAULT_HOST: &str = "flight.altertable.ai";
const DEFAULT_PORT: &str = "443";

#[derive(Debug)]
struct AltertableAuthIR<'a> {
    host: Cow<'a, str>,
    port: Cow<'a, str>,
    username: Cow<'a, str>,
    password: Cow<'a, str>,
    tls: bool,
}

impl AltertableAuthIR<'_> {
    pub fn apply(
        self,
        mut builder: DatabaseBuilder,
        _warning_printer: &dyn AuthWarningPrinter,
    ) -> Result<DatabaseBuilder, AuthError> {
        let scheme = if self.tls { "grpc+tls" } else { "grpc" };
        builder.with_parse_uri(format!("{scheme}://{}:{}", self.host, self.port))?;
        builder.with_username(self.username.as_ref());
        builder.with_password(self.password.as_ref());
        Ok(builder)
    }
}

/// `tls` accepts a YAML boolean or its string spelling, which is what a rendered
/// `"{{ env_var('ALTERTABLE_TLS', 'True') | as_bool }}"` produces.
fn parse_tls(config: &AdapterConfig) -> Result<bool, AuthError> {
    if let Some(tls) = config.get_bool("tls") {
        return Ok(tls);
    }
    match config.get_string("tls") {
        None => Ok(true),
        Some(value) if value.eq_ignore_ascii_case("true") => Ok(true),
        Some(value) if value.eq_ignore_ascii_case("false") => Ok(false),
        Some(value) => Err(AuthError::config(format!(
            "Altertable profile field 'tls' must be a boolean, got '{value}'"
        ))),
    }
}

fn required<'a>(config: &'a AdapterConfig, field: &str) -> Result<Cow<'a, str>, AuthError> {
    config
        .get_string(field)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AuthError::config(format!(
                "Altertable requires '{field}' in profile configuration"
            ))
        })
}

fn parse_auth<'a>(
    config: &'a AdapterConfig,
    _warning_printer: &dyn AuthWarningPrinter,
) -> Result<AltertableAuthIR<'a>, AuthError> {
    Ok(AltertableAuthIR {
        host: config
            .get_string("host")
            .unwrap_or(Cow::Borrowed(DEFAULT_HOST)),
        port: config
            .get_string("port")
            .unwrap_or(Cow::Borrowed(DEFAULT_PORT)),
        username: required(config, "username")?,
        password: required(config, "password")?,
        tls: parse_tls(config)?,
    })
}

fn apply_connection_args(
    _config: &AdapterConfig,
    builder: DatabaseBuilder,
    _warning_printer: &dyn AuthWarningPrinter,
) -> Result<DatabaseBuilder, AuthError> {
    Ok(builder)
}

pub struct AltertableAuth {
    pub warning_printer: Box<dyn AuthWarningPrinter>,
}

impl AltertableAuth {
    pub fn new(warning_printer: Box<dyn AuthWarningPrinter>) -> Self {
        Self { warning_printer }
    }
}

impl Auth for AltertableAuth {
    fn backend(&self) -> Backend {
        Backend::Altertable
    }

    fn configure(&self, config: &AdapterConfig) -> Result<DatabaseBuilder, AuthError> {
        auth_configure_pipeline!(self, config, parse_auth, apply_connection_args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_options::uri_value;
    use adbc_core::options::{OptionDatabase, OptionValue};
    use dbt_test_primitives::assert_contains;
    use dbt_yaml::Mapping;

    fn configure(yaml: &str) -> Result<DatabaseBuilder, AuthError> {
        let config: Mapping = dbt_yaml::from_str(yaml).expect("parse yaml");
        AltertableAuth::new(Box::new(crate::NoopAuthWarningPrinter))
            .configure(&AdapterConfig::new(config))
    }

    fn option_value(builder: &DatabaseBuilder, key: OptionDatabase) -> Option<String> {
        builder
            .clone()
            .into_iter()
            .find_map(|(k, v)| match (k == key, v) {
                (true, OptionValue::String(s)) => Some(s),
                _ => None,
            })
    }

    #[test]
    fn test_defaults_to_altertable_cloud_over_tls() {
        let builder = configure("username: dbt\npassword: secret").expect("configure");

        assert_eq!(uri_value(&builder), "grpc+tls://flight.altertable.ai:443");
        assert_eq!(
            option_value(&builder, OptionDatabase::Username).as_deref(),
            Some("dbt")
        );
        assert_eq!(
            option_value(&builder, OptionDatabase::Password).as_deref(),
            Some("secret")
        );
    }

    #[test]
    fn test_plaintext_from_boolean_or_rendered_string() {
        for tls in ["false", "'False'"] {
            let builder = configure(&format!(
                "host: localhost\nport: 15002\ntls: {tls}\nusername: dbt\npassword: secret"
            ))
            .expect("configure");
            assert_eq!(uri_value(&builder), "grpc://localhost:15002");
        }
    }

    #[test]
    fn test_invalid_tls_returns_error() {
        let err = configure("tls: maybe\nusername: dbt\npassword: secret").unwrap_err();
        assert_contains!(err.msg(), "'tls'");
    }

    #[test]
    fn test_missing_password_returns_error() {
        let err = configure("username: dbt").unwrap_err();
        assert_contains!(err.msg(), "'password'");
    }
}

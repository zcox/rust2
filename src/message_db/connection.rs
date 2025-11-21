use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

use crate::message_db::error::{Error, Result};

/// Configuration for Message DB client connection
#[derive(Debug, Clone)]
pub struct MessageDbConfig {
    /// PostgreSQL host
    pub host: String,

    /// PostgreSQL port
    pub port: u16,

    /// Database name (typically "message_store")
    pub database: String,

    /// Username
    pub user: String,

    /// Password
    pub password: String,

    /// Schema name (default: "message_store")
    pub schema_name: String,

    /// Maximum number of connections in the pool
    pub max_pool_size: usize,

    /// Command timeout in milliseconds
    pub command_timeout_ms: u64,
}

impl Default for MessageDbConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "message_store".to_string(),
            user: "postgres".to_string(),
            password: "".to_string(),
            schema_name: "message_store".to_string(),
            max_pool_size: 16,
            command_timeout_ms: 30000,
        }
    }
}

impl MessageDbConfig {
    /// Create a new configuration with the given connection string
    ///
    /// # Example
    ///
    /// ```
    /// use rust2::message_db::connection::MessageDbConfig;
    ///
    /// let config = MessageDbConfig::from_connection_string(
    ///     "postgresql://postgres:password@localhost:5432/message_store"
    /// ).unwrap();
    /// ```
    pub fn from_connection_string(connection_string: &str) -> Result<Self> {
        // Parse the connection string
        // Format: postgresql://user:password@host:port/database
        let url = connection_string
            .strip_prefix("postgresql://")
            .or_else(|| connection_string.strip_prefix("postgres://"))
            .ok_or_else(|| {
                Error::ValidationError("Invalid connection string format".to_string())
            })?;

        // Split into auth and location parts
        let parts: Vec<&str> = url.split('@').collect();
        if parts.len() != 2 {
            return Err(Error::ValidationError(
                "Invalid connection string format".to_string(),
            ));
        }

        // Parse auth (user:password)
        let auth_parts: Vec<&str> = parts[0].split(':').collect();
        if auth_parts.len() != 2 {
            return Err(Error::ValidationError(
                "Invalid connection string format".to_string(),
            ));
        }
        let user = auth_parts[0].to_string();
        let password = auth_parts[1].to_string();

        // Parse location (host:port/database)
        let location_parts: Vec<&str> = parts[1].split('/').collect();
        if location_parts.len() != 2 {
            return Err(Error::ValidationError(
                "Invalid connection string format".to_string(),
            ));
        }

        let host_port: Vec<&str> = location_parts[0].split(':').collect();
        let host = host_port[0].to_string();
        let port = if host_port.len() > 1 {
            host_port[1].parse::<u16>().map_err(|_| {
                Error::ValidationError("Invalid port number".to_string())
            })?
        } else {
            5432
        };

        let database = location_parts[1].to_string();

        Ok(Self {
            host,
            port,
            database,
            user,
            password,
            ..Default::default()
        })
    }

    /// Build a connection pool from this configuration
    pub fn build_pool(&self) -> Result<Pool> {
        let mut cfg = tokio_postgres::Config::new();
        cfg.host(&self.host);
        cfg.port(self.port);
        cfg.dbname(&self.database);
        cfg.user(&self.user);
        cfg.password(&self.password);

        // Set search_path to include message_store schema
        // This is critical for Message DB functions to work properly
        cfg.options(&format!("-c search_path={},public", self.schema_name));

        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };

        let manager = Manager::from_config(cfg, NoTls, manager_config);

        let pool = Pool::builder(manager)
            .max_size(self.max_pool_size)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| Error::ConnectionError(e.to_string()))?;

        Ok(pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MessageDbConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, "message_store");
        assert_eq!(config.schema_name, "message_store");
        assert_eq!(config.max_pool_size, 16);
    }

    #[test]
    fn test_from_connection_string() {
        let config = MessageDbConfig::from_connection_string(
            "postgresql://testuser:testpass@testhost:5433/testdb",
        )
        .unwrap();

        assert_eq!(config.host, "testhost");
        assert_eq!(config.port, 5433);
        assert_eq!(config.database, "testdb");
        assert_eq!(config.user, "testuser");
        assert_eq!(config.password, "testpass");
    }

    #[test]
    fn test_from_connection_string_default_port() {
        let config =
            MessageDbConfig::from_connection_string("postgresql://user:pass@host/db").unwrap();

        assert_eq!(config.host, "host");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, "db");
    }

    #[test]
    fn test_from_connection_string_with_postgres_prefix() {
        let config =
            MessageDbConfig::from_connection_string("postgres://user:pass@host:1234/db").unwrap();

        assert_eq!(config.host, "host");
        assert_eq!(config.port, 1234);
    }

    #[test]
    fn test_from_connection_string_invalid() {
        assert!(MessageDbConfig::from_connection_string("invalid").is_err());
        assert!(MessageDbConfig::from_connection_string("http://host/db").is_err());
    }
}

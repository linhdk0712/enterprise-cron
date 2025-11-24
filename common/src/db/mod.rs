// Database layer module
// Requirements: 12.4 - PostgreSQL connection pool with compile-time query checking

pub mod pool;
pub mod redis;
pub mod repositories;

pub use pool::DbPool;
pub use redis::RedisPool;

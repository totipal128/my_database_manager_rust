pub mod databases;

pub use databases::driver::DatabaseConfig;
pub use databases::create_databases::setup_database;
pub use databases::create_table::{Model, create_table, sync_table};
pub use databases::orm::{
    OrmModel, DbValue, PaginatedResult, WithChildren, QueryFilter,
    insert, update, delete,
    find_one, find_all, find_paginated,
    find_by, find_by_paginated,
    find_related, find_many_to_many,
    find_one_with_related,
};

// Re-export custom derive macro
pub use my_database_manager_macros::Model;

use sqlx::any::AnyRow;
use sqlx::{Any, Row, pool::Pool};

use crate::databases::create_table::Model;

#[derive(Debug, Clone)]
pub enum DbValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

// ─────────────────────────────────────────────────────────────────────────────
// TRAIT OrmModel
// ─────────────────────────────────────────────────────────────────────────────

/// Extension trait di atas `Model` yang menyediakan kemampuan CRUD generik.
///
/// Implementasikan trait ini pada Struct untuk dapat menggunakan fungsi-fungsi
/// ORM (`insert`, `update`, `delete`, `find_one`, `find_all`, `find_paginated`).
///
/// # Contoh
/// ```rust
/// use my_database_manager::{Model, OrmModel, DbValue};
/// use sqlx::any::AnyRow;
/// use sqlx::Row;
///
/// #[derive(Debug, Clone, Model)]
/// struct User {
///     id: i32,
///     username: String,
///     email: Option<String>,
/// }
///
/// impl OrmModel for User {
///     fn get_id(&self) -> i64 { self.id as i64 }
///
///     fn insert_values(&self) -> Vec<DbValue> {
///         vec![
///             DbValue::String(self.username.clone()),
///             DbValue::String(self.email.clone().unwrap_or_default()),
///         ]
///     }
///
///     fn update_values(&self) -> Vec<DbValue> {
///         self.insert_values()
///     }
///
///     fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
///         Ok(User {
///             id: row.try_get("id")?,
///             username: row.try_get("username")?,
///             email: row.try_get("email").ok(),
///         })
///     }
/// }
/// ```
pub trait OrmModel: Model {
    /// Mengembalikan nilai primary key (`id`) dari record ini.
    fn get_id(&self) -> i64;

    /// Mengembalikan nilai-nilai untuk INSERT, sesuai urutan `FIELDS_INSERT`.
    fn insert_values(&self) -> Vec<DbValue>;

    /// Mengembalikan nilai-nilai untuk UPDATE, sesuai urutan `FIELDS_INSERT`.
    /// Secara default sama dengan `insert_values()`.
    fn update_values(&self) -> Vec<DbValue> {
        self.insert_values()
    }

    /// Membangun instance Struct dari sebuah baris hasil query `sqlx::AnyRow`.
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error>;
}

// ─────────────────────────────────────────────────────────────────────────────
// HELPER: placeholder builder
// ─────────────────────────────────────────────────────────────────────────────

/// Menghasilkan placeholder SQL berdasarkan driver.
/// - SQLite / MySQL : `?, ?, ?`
/// - PostgreSQL     : `$1, $2, $3`
fn make_placeholders(count: usize, driver: &str) -> String {
    (1..=count)
        .map(|i| {
            ph(i, driver)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Menghasilkan satu placeholder SQL.
/// - PostgreSQL: `$1`, `$2`, dll
/// - Lainnya (SQLite/MySQL): `?`
fn ph(index: usize, driver: &str) -> String {
    if driver == "postgres" || driver == "postgresql" {
        format!("${}", index)
    } else {
        "?".to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// INSERT
// ─────────────────────────────────────────────────────────────────────────────

/// Menyisipkan satu record baru ke dalam database.
///
/// Nilai diambil dari `model.insert_values()` sesuai urutan `FIELDS_INSERT`.
pub async fn insert<T: OrmModel>(
    pool: &Pool<Any>,
    model: &T,
    driver: &str,
) -> Result<(), sqlx::Error> {
    let table = T::TABLE;
    let fields = T::FIELDS_INSERT.join(", ");
    let values = model.insert_values();
    let placeholders = make_placeholders(values.len(), driver);

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table, fields, placeholders
    );

    // println!("[ORM] FIELDS INSERT SQL: {}", fields);
    // println!("[ORM] VALUES INSERT SQL: {:?}", values);
    println!("[ORM] INSERT SQL: {}", sql);

    let mut query = sqlx::query(&sql);
    for val in &values {
        query = match val {
            DbValue::String(v) => query.bind(v),

            DbValue::Int(v) => query.bind(v),

            DbValue::Float(v) => query.bind(v),

            DbValue::Bool(v) => {
                if driver == "mysql" {
                    query.bind(if *v { 1i32 } else { 0i32 })
                } else {
                    query.bind(v)
                }
            }

            DbValue::Null => query.bind(None::<String>),
        };
    }
    query.execute(pool).await?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// UPDATE
// ─────────────────────────────────────────────────────────────────────────────

/// Memperbarui record berdasarkan `id` yang ada di struct.
///
/// Semua field pada `FIELDS_INSERT` akan di-update.
pub async fn update<T: OrmModel>(
    pool: &Pool<Any>,
    model: &T,
    driver: &str,
) -> Result<(), sqlx::Error> {
    let table = T::TABLE;
    let values = model.update_values();
    let id = model.get_id();

    let set_clauses: Vec<String> = T::FIELDS_INSERT
        .iter()
        .enumerate()
        .map(|(i, field)| {
            if driver == "postgres" {
                format!("{} = ${}", field, i + 1)
            } else {
                format!("{} = ?", field)
            }
        })
        .collect();

    let id_placeholder = if driver == "postgres" {
        format!("${}", values.len() + 1)
    } else {
        "?".to_string()
    };

    let sql = format!(
        "UPDATE {} SET {} WHERE id = {}",
        table,
        set_clauses.join(", "),
        id_placeholder
    );

    println!("[ORM] UPDATE SQL: {}", sql);

    let mut query = sqlx::query(&sql);
    for val in &values {
        query = match val {
            DbValue::String(v) => query.bind(v),

            DbValue::Int(v) => query.bind(v),

            DbValue::Float(v) => query.bind(v),

            DbValue::Bool(v) => {
                if driver == "mysql" {
                    query.bind(if *v { 1i32 } else { 0i32 })
                } else {
                    query.bind(v)
                }
            }

            DbValue::Null => query.bind(None::<String>),
        };
    }
    query = query.bind(id);
    query.execute(pool).await?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// DELETE
// ─────────────────────────────────────────────────────────────────────────────

/// Menghapus record berdasarkan `id`.
pub async fn delete<T: OrmModel>(pool: &Pool<Any>, driver: &str, id: i64) -> Result<(), sqlx::Error> {
    let sql = format!("DELETE FROM {} WHERE id = {}", T::TABLE, ph(1, driver));
    println!("[ORM] DELETE SQL: {}", sql);

    sqlx::query(&sql).bind(id).execute(pool).await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND ONE
// ─────────────────────────────────────────────────────────────────────────────

/// Mencari satu record berdasarkan `id`.
///
/// Mengembalikan `None` jika tidak ditemukan.
pub async fn find_one<T: OrmModel>(pool: &Pool<Any>, driver: &str, id: i64) -> Result<Option<T>, sqlx::Error> {
    let sql = format!("SELECT * FROM {} WHERE id = {} LIMIT 1", T::TABLE, ph(1, driver));
    println!("[ORM] FIND ONE SQL: {} | {}  ", sql, id);

    let row = sqlx::query(&sql).bind(id).fetch_optional(pool).await?;

    match row {
        Some(r) => Ok(Some(T::from_row(r)?)),
        None => Ok(None),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND ALL
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// FILTER & SEARCH
// ─────────────────────────────────────────────────────────────────────────────    /// Menampung kondisi pencarian dan filter untuk query ORM.
#[derive(Debug, Default, Clone)]
pub struct QueryFilter {
    /// Kondisi `column = value`.
    pub exact_match: Vec<(String, DbValue)>,
    /// Kondisi `column LIKE %value%` (case-sensitive sesuai database).
    pub search_like: Vec<(String, String)>,
    /// Kondisi `LOWER(column) LIKE %value%` (case-insensitive).
    pub search_like_lower: Vec<(String, String)>,
    /// Kondisi `ORDER BY column ASC/DESC`.
    pub order_by: Option<String>,
}

impl QueryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Menambahkan filter pencocokan persis teks (`column = value`).
    pub fn exact(mut self, column: &str, value: &str) -> Self {
        self.exact_match
            .push((column.to_string(), DbValue::String(value.to_string())));
        self
    }

    /// Menambahkan filter pencocokan persis integer (`column = value`).
    pub fn exact_int(mut self, column: &str, value: i64) -> Self {
        self.exact_match
            .push((column.to_string(), DbValue::Int(value)));
        self
    }

    /// Menambahkan filter pencocokan persis boolean (`column = value`).
    pub fn exact_bool(mut self, column: &str, value: bool) -> Self {
        self.exact_match
            .push((column.to_string(), DbValue::Bool(value)));
        self
    }

    /// Menambahkan filter pencarian string (`column LIKE %value%`).
    pub fn like(mut self, column: &str, value: &str) -> Self {
        self.search_like
            .push((column.to_string(), value.to_string()));
        self
    }

    /// Menambahkan filter pencarian case-insensitive (`LOWER(column) LIKE %value%`).
    pub fn like_lower(mut self, column: &str, value: &str) -> Self {
        self.search_like_lower
            .push((column.to_string(), value.to_lowercase()));
        self
    }

    /// Menentukan urutan hasil (misalnya: `"id DESC"`).
    pub fn order(mut self, order: &str) -> Self {
        self.order_by = Some(order.to_string());
        self
    }

    /// Membangun string `WHERE` dan daftar *binding values*.
    /// Parameter `start_index` digunakan untuk PostgreSQL (`$1`, `$2`, dll).
    pub fn build_where_clause(
        &self,
        driver: &str,
        mut start_index: usize,
    ) -> (String, Vec<DbValue>, usize) {
        let mut clauses = Vec::new();
        let mut bindings = Vec::new();
        let is_pg = driver == "postgres" || driver == "postgresql";

        for (col, val) in &self.exact_match {
            let ph = if is_pg { format!("${}", start_index) } else { "?".to_string() };
            let clause = match val {
                DbValue::Bool(_) => {
                    if is_pg {
                        format!("{} = CAST({} AS BOOLEAN)", col, ph)
                    } else {
                        format!("{} = {}", col, ph)
                    }
                }
                DbValue::Int(_) => {
                    if is_pg {
                        format!("{} = CAST({} AS INTEGER)", col, ph)
                    } else {
                        format!("{} = {}", col, ph)
                    }
                }
                _ => format!("{} = {}", col, ph),
            };
            clauses.push(clause);
            bindings.push(val.clone());
            start_index += 1;
        }

        for (col, val) in &self.search_like {
            if is_pg {
                clauses.push(format!("{} LIKE ${}", col, start_index));
            } else {
                clauses.push(format!("{} LIKE ?", col));
            }
            // Tambahkan wildcards untuk LIKE
            bindings.push(DbValue::String(format!("%{}%", val)));
            start_index += 1;
        }

        for (col, val) in &self.search_like_lower {
            if is_pg {
                clauses.push(format!("LOWER({}) LIKE ${}", col, start_index));
            } else {
                clauses.push(format!("LOWER({}) LIKE ?", col));
            }
            bindings.push(DbValue::String(format!("%{}%", val)));
            start_index += 1;
        }

        let where_clause = if clauses.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", clauses.join(" AND "))
        };

        (where_clause, bindings, start_index)
    }

    pub fn build_order_clause(&self) -> String {
        match &self.order_by {
            Some(o) => format!("ORDER BY {}", o),
            None => "".to_string(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND ALL
// ─────────────────────────────────────────────────────────────────────────────

/// Mengambil semua record dari tabel dengan opsional filter dan pencarian.
pub async fn find_all<T: OrmModel>(
    pool: &Pool<Any>,
    driver: &str,
    filter: Option<&QueryFilter>,
) -> Result<Vec<T>, sqlx::Error> {
    let table = T::TABLE;

    let (where_clause, bindings, _) = if let Some(f) = filter {
        f.build_where_clause(driver, 1)
    } else {
        ("".to_string(), vec![], 1)
    };

    let order_clause = filter.map(|f| f.build_order_clause()).unwrap_or_default();

    let sql = format!("SELECT * FROM {} {} {}", table, where_clause, order_clause);
    println!("[ORM] FIND ALL SQL: {}", sql);
    println!("[ORM] FIND ALL BINDINGS: {:?}", bindings);

    let mut query = sqlx::query(&sql);
    for b in bindings {
        query = match b {
            DbValue::String(v) => query.bind(v),
            DbValue::Int(v) => query.bind(v),
            DbValue::Float(v) => query.bind(v),
            DbValue::Bool(v) => {
                if driver == "mysql" {
                    query.bind(if v { 1i32 } else { 0i32 })
                } else {
                    query.bind(v)
                }
            }
            DbValue::Null => query.bind(None::<String>),
        };
    }

    let rows = query.fetch_all(pool).await?;
    let results = rows
        .into_iter()
        .map(|r| T::from_row(r))
        .collect::<Result<Vec<T>, _>>()?;

    Ok(results)
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND PAGINATED
// ─────────────────────────────────────────────────────────────────────────────

/// Hasil query dengan paginasi.
#[derive(Debug)]
pub struct PaginatedResult<T> {
    pub data: Vec<T>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
}

/// Mengambil record dengan paginasi dan filter opsional.
pub async fn find_paginated<T: OrmModel>(
    pool: &Pool<Any>,
    driver: &str,
    page: u64,
    per_page: u64,
    filter: Option<&QueryFilter>,
) -> Result<PaginatedResult<T>, sqlx::Error> {
    let table = T::TABLE;

    let (where_clause, bindings, next_index) = if let Some(f) = filter {
        f.build_where_clause(driver, 1)
    } else {
        ("".to_string(), vec![], 1)
    };
    let order_clause = filter.map(|f| f.build_order_clause()).unwrap_or_default();

    // 1. Hitung total record sesuai filter
    let count_sql = format!("SELECT COUNT(*) as count FROM {} {}", table, where_clause);
    let mut count_query = sqlx::query(&count_sql);
    for b in &bindings {
        count_query = match b {
            DbValue::String(v) => count_query.bind(v),
            DbValue::Int(v) => count_query.bind(v),
            DbValue::Float(v) => count_query.bind(v),
            DbValue::Bool(v) => {
                if driver == "mysql" {
                    count_query.bind(if *v { 1i32 } else { 0i32 })
                } else {
                    count_query.bind(*v)
                }
            }
            DbValue::Null => count_query.bind(None::<String>),
        };
    }
    let count_row = count_query.fetch_one(pool).await?;
    let total: i64 = count_row.try_get("count")?;
    let total = total as u64;

    // 2. Hitung limit dan offset
    let page = page.max(1);
    let offset = (page - 1) * per_page;
    let total_pages = total.div_ceil(per_page);

    // 3. Query data
    let (limit_ph, offset_ph) = if driver == "postgres" {
        let l = format!("${}", next_index);
        let o = format!("${}", next_index + 1);
        (l, o)
    } else {
        ("?".to_string(), "?".to_string())
    };

    let sql = format!(
        "SELECT * FROM {} {} {} LIMIT {} OFFSET {}",
        table, where_clause, order_clause, limit_ph, offset_ph
    );
    println!("[ORM] PAGINATED SQL: {}", sql);

    let mut data_query = sqlx::query(&sql);
    for b in bindings {
        data_query = match b {
            DbValue::String(v) => data_query.bind(v),
            DbValue::Int(v) => data_query.bind(v),
            DbValue::Float(v) => data_query.bind(v),
            DbValue::Bool(v) => {
                if driver == "mysql" {
                    data_query.bind(if v { 1i32 } else { 0i32 })
                } else {
                    data_query.bind(v)
                }
            }
            DbValue::Null => data_query.bind(None::<String>),
        };
    }
    data_query = data_query.bind(per_page as i64);
    data_query = data_query.bind(offset as i64);

    let rows = data_query.fetch_all(pool).await?;
    let data = rows
        .into_iter()
        .map(|r| T::from_row(r))
        .collect::<Result<Vec<T>, _>>()?;

    Ok(PaginatedResult {
        data,
        page,
        per_page,
        total,
        total_pages,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// STRUCT: WithChildren — Parent + Related Children
// ─────────────────────────────────────────────────────────────────────────────

/// Struct yang menampung satu record parent beserta daftar record child yang berelasi.
///
/// Digunakan oleh `find_one_with_related` untuk relasi One-to-Many.
#[derive(Debug)]
pub struct WithChildren<P, C> {
    /// Record parent (misalnya: User).
    pub parent: P,
    /// Daftar record anak yang berelasi (misalnya: semua Post milik User).
    pub children: Vec<C>,
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND BY COLUMN — Filter berdasarkan nilai kolom tertentu
// ─────────────────────────────────────────────────────────────────────────────

/// Mengambil semua record dari tabel T di mana `column = value`.
///
/// Cocok untuk filter sederhana, termasuk filter berdasarkan *foreign key*.
///
/// # Contoh
/// ```ignore
/// // Ambil semua Post dengan user_id = 5
/// let posts = find_by::<Post>(&pool, "user_id", "5").await?;
/// ```
/// Mengambil semua record dari tabel T di mana `column = value`.
///
/// Untuk PostgreSQL, kolom di-cast ke TEXT agar kompatibel dengan berbagai tipe data (integer, boolean, dll).
pub async fn find_by<T: OrmModel>(
    pool: &Pool<Any>,
    driver: &str,
    column: &str,
    value: &str,
) -> Result<Vec<T>, sqlx::Error> {
    let is_pg = driver == "postgres" || driver == "postgresql";
    let col_expr = if is_pg {
        format!("{}::TEXT", column)
    } else {
        column.to_string()
    };
    let sql = format!("SELECT * FROM {} WHERE {} = {}", T::TABLE, col_expr, ph(1, driver));
    println!("[ORM] FIND BY SQL: {} ({}={})", sql, column, value);

    let rows = sqlx::query(&sql).bind(value).fetch_all(pool).await?;

    rows.into_iter()
        .map(|r| T::from_row(r))
        .collect::<Result<Vec<T>, _>>()
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND BY COLUMN — Dengan Paginasi
// ─────────────────────────────────────────────────────────────────────────────

/// Mengambil record berdasarkan filter kolom dengan dukungan paginasi.
///
/// # Contoh
/// ```ignore
/// // Ambil halaman 1 dari semua Post milik user_id = 5 (10 per halaman)
/// let result = find_by_paginated::<Post>(&pool, "user_id", "5", 1, 10).await?;
/// ```
pub async fn find_by_paginated<T: OrmModel>(
    pool: &Pool<Any>,
    driver: &str,
    column: &str,
    value: &str,
    page: u64,
    per_page: u64,
) -> Result<PaginatedResult<T>, sqlx::Error> {
    let table = T::TABLE;
    let is_pg = driver == "postgres" || driver == "postgresql";
    let col_expr = if is_pg {
        format!("{}::TEXT", column)
    } else {
        column.to_string()
    };

    // Total record yang cocok
    let count_sql = format!(
        "SELECT COUNT(*) as count FROM {} WHERE {} = {}",
        table, col_expr, ph(1, driver)
    );
    let count_row = sqlx::query(&count_sql).bind(value).fetch_one(pool).await?;
    let total = count_row.try_get::<i64, _>("count")? as u64;

    let page = page.max(1);
    let offset = (page - 1) * per_page;
    let total_pages = total.div_ceil(per_page);

    let (limit_ph, offset_ph) = if is_pg {
        (ph(2, driver), ph(3, driver))
    } else {
        ("?".to_string(), "?".to_string())
    };

    let sql = format!(
        "SELECT * FROM {} WHERE {} = {} LIMIT {} OFFSET {}",
        table, col_expr, ph(1, driver), limit_ph, offset_ph
    );
    println!(
        "[ORM] FIND BY PAGINATED: {} ({}={}, limit={}, offset={})",
        sql, column, value, per_page, offset
    );

    let rows = sqlx::query(&sql)
        .bind(value)
        .bind(per_page as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

    let data = rows
        .into_iter()
        .map(|r| T::from_row(r))
        .collect::<Result<Vec<T>, _>>()?;

    Ok(PaginatedResult {
        data,
        page,
        per_page,
        total,
        total_pages,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND RELATED — One-to-Many
// ─────────────────────────────────────────────────────────────────────────────

/// Mengambil semua record `Child` yang berelasi ke `parent_id` via `fk_column`.
///
/// Digunakan untuk relasi **One-to-Many**: satu parent memiliki banyak child.
///
/// # Argumen
/// * `fk_column` - Nama kolom foreign key di tabel Child (misal: `"user_id"`).
/// * `parent_id` - Nilai id dari record parent.
///
/// # Contoh
/// ```ignore
/// // Ambil semua Post milik User dengan id = 3
/// let posts = find_related::<Post>(&pool, "user_id", 3).await?;
/// ```
pub async fn find_related<Child: OrmModel>(
    pool: &Pool<Any>,
    driver: &str,
    fk_column: &str,
    parent_id: i64,
) -> Result<Vec<Child>, sqlx::Error> {
    let sql = format!("SELECT * FROM {} WHERE {} = {}", Child::TABLE, fk_column, ph(1, driver));
    println!(
        "[ORM] FIND RELATED SQL: {} ({}={})",
        sql, fk_column, parent_id
    );

    let rows = sqlx::query(&sql).bind(parent_id).fetch_all(pool).await?;

    rows.into_iter()
        .map(|r| Child::from_row(r))
        .collect::<Result<Vec<Child>, _>>()
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND MANY TO MANY — via Pivot Table
// ─────────────────────────────────────────────────────────────────────────────

/// Mengambil semua record `T` yang terhubung melalui sebuah tabel pivot (join table).
///
/// Digunakan untuk relasi **Many-to-Many**.
///
/// # Argumen
/// * `pivot_table` - Nama tabel perantara (misal: `"user_roles"`).
/// * `pivot_fk` - Kolom di tabel pivot yang menunjuk ke record yang dicari (misal: `"role_id"`).
/// * `pivot_ref` - Kolom di tabel pivot yang menjadi filter (misal: `"user_id"`).
/// * `ref_id` - Nilai id untuk `pivot_ref` (misal: id user = 5).
///
/// # Contoh
/// ```ignore
/// // Ambil semua Role milik User dengan id = 5
/// // (melalui tabel pivot "user_roles" yang punya kolom user_id dan role_id)
/// let roles = find_many_to_many::<Role>(&pool, "user_roles", "role_id", "user_id", 5).await?;
/// ```
pub async fn find_many_to_many<T: OrmModel>(
    pool: &Pool<Any>,
    driver: &str,
    pivot_table: &str,
    pivot_fk: &str,
    pivot_ref: &str,
    ref_id: i64,
) -> Result<Vec<T>, sqlx::Error> {
    let target_table = T::TABLE;
    let ph1 = ph(1, driver);

    // JOIN tabel target dengan pivot, filter berdasarkan ref_id
    let sql = format!(
        "SELECT {target}.* FROM {target} \
         INNER JOIN {pivot} ON {pivot}.{fk} = {target}.id \
         WHERE {pivot}.{pref} = {ph}",
        target = target_table,
        pivot = pivot_table,
        fk = pivot_fk,
        pref = pivot_ref,
        ph = ph1,
    );
    println!("[ORM] MANY-TO-MANY SQL: {} ({}={})", sql, pivot_ref, ref_id);

    let rows = sqlx::query(&sql).bind(ref_id).fetch_all(pool).await?;

    rows.into_iter()
        .map(|r| T::from_row(r))
        .collect::<Result<Vec<T>, _>>()
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND ONE WITH RELATED — Parent + Children sekaligus
// ─────────────────────────────────────────────────────────────────────────────

/// Mencari satu record Parent berdasarkan `id`, lalu memuat semua Child-nya sekaligus.
///
/// Mengembalikan `None` jika parent tidak ditemukan.
///
/// # Argumen
/// * `parent_id` - Id dari record parent.
/// * `fk_column` - Nama kolom foreign key di tabel Child yang menunjuk ke parent (misal: `"user_id"`).
///
/// # Contoh
/// ```ignore
/// // Ambil User id=1 beserta semua Post miliknya
/// let result: Option<WithChildren<User, Post>> =
///     find_one_with_related::<User, Post>(&pool, 1, "user_id").await?;
///
/// if let Some(data) = result {
///     println!("User: {:?}", data.parent);
///     println!("Posts ({}):", data.children.len());
///     for post in data.children {
///         println!("  - {:?}", post);
///     }
/// }
/// ```
pub async fn find_one_with_related<P: OrmModel, C: OrmModel>(
    pool: &Pool<Any>,
    driver: &str,
    parent_id: i64,
    fk_column: &str,
) -> Result<Option<WithChildren<P, C>>, sqlx::Error> {
    // 1. Cari parent terlebih dahulu
    let parent = find_one::<P>(pool, driver, parent_id).await?;
    let Some(parent) = parent else {
        return Ok(None);
    };

    // 2. Cari semua child yang ber-relasi
    let children = find_related::<C>(pool, driver, fk_column, parent_id).await?;

    Ok(Some(WithChildren { parent, children }))
}

// ─────────────────────────────────────────────────────────────────────────────
// UNIT TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ph_sqlite() {
        assert_eq!(ph(1, "sqlite"), "?");
        assert_eq!(ph(5, "sqlite"), "?");
        assert_eq!(ph(100, "sqlite"), "?");
    }

    #[test]
    fn test_ph_mysql() {
        assert_eq!(ph(1, "mysql"), "?");
        assert_eq!(ph(3, "mysql"), "?");
    }

    #[test]
    fn test_ph_postgres() {
        assert_eq!(ph(1, "postgres"), "$1");
        assert_eq!(ph(3, "postgres"), "$3");
        assert_eq!(ph(10, "postgres"), "$10");
        assert_eq!(ph(1, "postgresql"), "$1");
        assert_eq!(ph(2, "postgresql"), "$2");
    }

    #[test]
    fn test_make_placeholders_sqlite() {
        assert_eq!(make_placeholders(1, "sqlite"), "?");
        assert_eq!(make_placeholders(3, "sqlite"), "?, ?, ?");
        assert_eq!(make_placeholders(5, "sqlite"), "?, ?, ?, ?, ?");
    }

    #[test]
    fn test_make_placeholders_mysql() {
        assert_eq!(make_placeholders(2, "mysql"), "?, ?");
        assert_eq!(make_placeholders(4, "mysql"), "?, ?, ?, ?");
    }

    #[test]
    fn test_make_placeholders_postgres() {
        assert_eq!(make_placeholders(1, "postgres"), "$1");
        assert_eq!(make_placeholders(3, "postgres"), "$1, $2, $3");
        assert_eq!(make_placeholders(2, "postgresql"), "$1, $2");
    }

    #[test]
    fn test_make_placeholders_empty() {
        assert_eq!(make_placeholders(0, "sqlite"), "");
        assert_eq!(make_placeholders(0, "postgres"), "");
    }
}

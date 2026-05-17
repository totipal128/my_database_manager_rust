use sqlx::{pool::Pool, Any, Row};
use sqlx::any::AnyRow;

use crate::databases::create_table::Model;

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
/// use my_database_manager::{Model, OrmModel};
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
///     fn insert_values(&self) -> Vec<String> {
///         vec![self.username.clone(), self.email.clone().unwrap_or_default()]
///     }
///
///     fn update_values(&self) -> Vec<String> {
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
    fn insert_values(&self) -> Vec<String>;

    /// Mengembalikan nilai-nilai untuk UPDATE, sesuai urutan `FIELDS_INSERT`.
    /// Secara default sama dengan `insert_values()`.
    fn update_values(&self) -> Vec<String> {
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
            if driver == "postgres" {
                format!("${}", i)
            } else {
                "?".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
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

    println!("[ORM] INSERT SQL: {}", sql);

    let mut query = sqlx::query(&sql);
    for val in &values {
        query = query.bind(val.as_str());
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
        query = query.bind(val.as_str());
    }
    query = query.bind(id);
    query.execute(pool).await?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// DELETE
// ─────────────────────────────────────────────────────────────────────────────

/// Menghapus record berdasarkan `id`.
pub async fn delete<T: OrmModel>(
    pool: &Pool<Any>,
    id: i64,
) -> Result<(), sqlx::Error> {
    let sql = format!("DELETE FROM {} WHERE id = ?", T::TABLE);
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
pub async fn find_one<T: OrmModel>(
    pool: &Pool<Any>,
    id: i64,
) -> Result<Option<T>, sqlx::Error> {
    let sql = format!("SELECT * FROM {} WHERE id = ? LIMIT 1", T::TABLE);
    println!("[ORM] FIND ONE SQL: {}", sql);

    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(r) => Ok(Some(T::from_row(r)?)),
        None => Ok(None),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FIND ALL
// ─────────────────────────────────────────────────────────────────────────────

/// Mengambil semua record dari tabel.
pub async fn find_all<T: OrmModel>(
    pool: &Pool<Any>,
) -> Result<Vec<T>, sqlx::Error> {
    let sql = format!("SELECT * FROM {}", T::TABLE);
    println!("[ORM] FIND ALL SQL: {}", sql);

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
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
    /// Data pada halaman ini.
    pub data: Vec<T>,
    /// Halaman saat ini (dimulai dari 1).
    pub page: u64,
    /// Jumlah record per halaman.
    pub per_page: u64,
    /// Total record yang tersedia.
    pub total: u64,
    /// Total halaman yang tersedia.
    pub total_pages: u64,
}

/// Mengambil record dengan paginasi.
///
/// # Arguments
/// * `page` - Nomor halaman, dimulai dari 1.
/// * `per_page` - Jumlah data per halaman.
pub async fn find_paginated<T: OrmModel>(
    pool: &Pool<Any>,
    page: u64,
    per_page: u64,
) -> Result<PaginatedResult<T>, sqlx::Error> {
    let table = T::TABLE;

    // Hitung total record
    let count_sql = format!("SELECT COUNT(*) as count FROM {}", table);
    let count_row = sqlx::query(&count_sql).fetch_one(pool).await?;
    let total: i64 = count_row.try_get("count")?;
    let total = total as u64;

    // Hitung offset
    let page = page.max(1);
    let offset = (page - 1) * per_page;
    let total_pages = total.div_ceil(per_page);

    // Query data
    let sql = format!(
        "SELECT * FROM {} LIMIT ? OFFSET ?",
        table
    );
    println!("[ORM] PAGINATED SQL: {} (limit={}, offset={})", sql, per_page, offset);

    let rows = sqlx::query(&sql)
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
pub async fn find_by<T: OrmModel>(
    pool: &Pool<Any>,
    column: &str,
    value: &str,
) -> Result<Vec<T>, sqlx::Error> {
    let sql = format!("SELECT * FROM {} WHERE {} = ?", T::TABLE, column);
    println!("[ORM] FIND BY SQL: {} ({}={})", sql, column, value);

    let rows = sqlx::query(&sql)
        .bind(value)
        .fetch_all(pool)
        .await?;

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
    column: &str,
    value: &str,
    page: u64,
    per_page: u64,
) -> Result<PaginatedResult<T>, sqlx::Error> {
    let table = T::TABLE;

    // Total record yang cocok
    let count_sql = format!("SELECT COUNT(*) as count FROM {} WHERE {} = ?", table, column);
    let count_row = sqlx::query(&count_sql).bind(value).fetch_one(pool).await?;
    let total = count_row.try_get::<i64, _>("count")? as u64;

    let page = page.max(1);
    let offset = (page - 1) * per_page;
    let total_pages = total.div_ceil(per_page);

    let sql = format!("SELECT * FROM {} WHERE {} = ? LIMIT ? OFFSET ?", table, column);
    println!("[ORM] FIND BY PAGINATED: {} ({}={}, limit={}, offset={})", sql, column, value, per_page, offset);

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

    Ok(PaginatedResult { data, page, per_page, total, total_pages })
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
    fk_column: &str,
    parent_id: i64,
) -> Result<Vec<Child>, sqlx::Error> {
    let sql = format!(
        "SELECT * FROM {} WHERE {} = ?",
        Child::TABLE, fk_column
    );
    println!("[ORM] FIND RELATED SQL: {} ({}={})", sql, fk_column, parent_id);

    let rows = sqlx::query(&sql)
        .bind(parent_id)
        .fetch_all(pool)
        .await?;

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
    pivot_table: &str,
    pivot_fk: &str,
    pivot_ref: &str,
    ref_id: i64,
) -> Result<Vec<T>, sqlx::Error> {
    let target_table = T::TABLE;

    // JOIN tabel target dengan pivot, filter berdasarkan ref_id
    let sql = format!(
        "SELECT {t}.* FROM {t} \
         INNER JOIN {pivot} ON {pivot}.{fk} = {t}.id \
         WHERE {pivot}.{pref} = ?",
        t = target_table,
        pivot = pivot_table,
        fk = pivot_fk,
        pref = pivot_ref,
    );
    println!("[ORM] MANY-TO-MANY SQL: {} ({}={})", sql, pivot_ref, ref_id);

    let rows = sqlx::query(&sql)
        .bind(ref_id)
        .fetch_all(pool)
        .await?;

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
    parent_id: i64,
    fk_column: &str,
) -> Result<Option<WithChildren<P, C>>, sqlx::Error> {
    // 1. Cari parent terlebih dahulu
    let parent = find_one::<P>(pool, parent_id).await?;
    let Some(parent) = parent else { return Ok(None) };

    // 2. Cari semua child yang ber-relasi
    let children = find_related::<C>(pool, fk_column, parent_id).await?;

    Ok(Some(WithChildren { parent, children }))
}

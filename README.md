# My Database Manager

Sebuah library Rust untuk mempermudah manajemen koneksi dan pembuatan skema database secara terpusat untuk berbagai jenis driver (PostgreSQL, MySQL, SQLite) menggunakan `sqlx`.

## Fitur Utama

- **Multi-Driver Connection**: Mendukung berbagai database dengan satu konfigurasi (`sqlx::any`).
- **Auto Create Database**: Otomatis membuat database fisik jika belum ada saat inisialisasi.
- **`#[derive(Model)]` Macro**: Deklarasikan skema tabel langsung dari field struct — tanpa menulis SQL secara manual.
- **Auto Type Mapping**: Tipe Rust dikonversi ke SQL secara otomatis (`i32` → `INT`, `Option<String>` → nullable `VARCHAR`, dst.).
- **Auto Schema Sync**: Kolom ditambah/dihapus secara otomatis saat struct berubah (`ALTER TABLE`).
- **ORM Generik**: Operasi CRUD lengkap (`insert`, `update`, `delete`, `find_one`, `find_all`, `find_paginated`) via trait `OrmModel`.
- **Relational Support**: Mendukung deklarasi *Foreign Key* untuk relasi `One-to-One`, `One-to-Many`, dan `Many-to-Many`.

---

## Cara Penggunaan

### 1. Inisialisasi Koneksi dan Pembuatan Database

Library ini menyediakan `DatabaseConfig` dan `setup_database` untuk mengatur database di awal.

```rust
use my_database_manager::{DatabaseConfig, setup_database};

#[tokio::main]
async fn main() {
    // Driver yang didukung: "postgres", "mysql", "sqlite"
    let config = DatabaseConfig::new(
        "postgres", "localhost", 5432, "postgres", "password123", "my_app_db"
    );

    // Otomatis membuat database jika belum ada di server
    setup_database(&config).await.expect("Gagal menyiapkan database");

    // Mendapatkan object pool connection dari sqlx
    let pool = config.connect().await.expect("Gagal terhubung ke database");
    println!("Database siap!");
}
```

---

### 2. Membuat Model dengan `#[derive(Model)]` *(Cara Direkomendasikan)*

Cara termudah adalah menggunakan derive macro. Library akan otomatis membaca field struct Anda dan menghasilkan deklarasi SQL yang sesuai.

```rust
use my_database_manager::Model;

#[derive(Debug, Clone, Model)]
#[table("users")]           // ← Opsional. Default: nama struct lowercase + "s" → "users"
pub struct User {
    id: i32,                // → id INT PRIMARY KEY (otomatis, dikecualikan dari INSERT)
    username: String,       // → username VARCHAR(255) NOT NULL
    email: Option<String>,  // → email VARCHAR(255)  (nullable, karena Option)
    age: i32,               // → age INT NOT NULL
    score: Option<f64>,     // → score DOUBLE        (nullable)
    is_active: bool,        // → is_active BOOLEAN NOT NULL
}
```

Macro secara otomatis menghasilkan:

- `TABLE` → nama tabel dari atribut `#[table("...")]` atau nama struct lowercase + "s"
- `FIELDS_INSERT` → semua field **selain `id`**
- `FIELDS_DECLARATION` → deklarasi SQL tiap kolom lengkap dengan tipe dan constraint
- `FOREIGN_KEYS` → kosong secara default (bisa di-override secara manual jika diperlukan)

#### Pemetaan Tipe Rust → SQL

| Tipe Rust | SQL | Nullable? |
| --- | --- | --- |
| `i8`, `i16`, `i32`, `u8`, `u16`, `u32` | `INT` | ❌ |
| `i64`, `u64`, `isize`, `usize` | `BIGINT` | ❌ |
| `f32` | `FLOAT` | ❌ |
| `f64` | `DOUBLE` | ❌ |
| `bool` | `BOOLEAN` | ❌ |
| `String` | `VARCHAR(255)` | ❌ |
| `NaiveDate` | `DATE` | ❌ |
| `NaiveDateTime`, `DateTime` | `DATETIME` | ❌ |
| `Uuid` | `VARCHAR(36)` | ❌ |
| `Option<T>` | *(tipe dalam T)* | ✅ |

> **Catatan:** Field bernama `id` selalu dijadikan `PRIMARY KEY` dan tidak dimasukkan ke dalam `FIELDS_INSERT` (cocok untuk auto-increment).

---

### 3. Membuat Model Secara Manual *(Cara Lanjutan)*

Jika Anda membutuhkan kontrol penuh atas deklarasi SQL (misal: `UNIQUE`, `DEFAULT`, tipe kustom), Anda bisa mengimplementasikan trait `Model` secara manual.

#### Model Sederhana

```rust
use my_database_manager::Model;

#[derive(Debug, Clone)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub email: String,
}

impl Model for User {
    const TABLE: &'static str = "users";
    const FIELDS_INSERT: &'static [&'static str] = &["username", "email"];

    const FIELDS_DECLARATION: &'static [&'static str] = &[
        "id INT PRIMARY KEY",
        "username VARCHAR(100) NOT NULL",
        "email VARCHAR(255) UNIQUE NOT NULL",
    ];

    const FOREIGN_KEYS: &'static [&'static str] = &[];
}
```

#### Relasi One-to-Many / One-to-One

```rust
impl Model for Post {
    const TABLE: &'static str = "posts";
    const FIELDS_INSERT: &'static [&'static str] = &["title", "user_id"];

    const FIELDS_DECLARATION: &'static [&'static str] = &[
        "id INT PRIMARY KEY",
        "title VARCHAR(255) NOT NULL",
        "user_id INT NOT NULL",
    ];

    const FOREIGN_KEYS: &'static [&'static str] = &[
        "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE",
    ];
}
```

#### Relasi Many-to-Many (Tabel Perantara)

```rust
impl Model for UserRole {
    const TABLE: &'static str = "user_roles";
    const FIELDS_INSERT: &'static [&'static str] = &["user_id", "role_id"];

    const FIELDS_DECLARATION: &'static [&'static str] = &[
        "user_id INT NOT NULL",
        "role_id INT NOT NULL",
        "PRIMARY KEY (user_id, role_id)",  // composite PK
    ];

    const FOREIGN_KEYS: &'static [&'static str] = &[
        "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE",
        "FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE",
    ];
}
```

---

### 4. Eksekusi Create Table dan Sinkronisasi Otomatis

Gunakan `sync_table` agar skema tabel selalu sinkron dengan struct Anda. Jika ada kolom baru di struct, akan di-`ADD COLUMN`. Jika ada kolom yang dihapus dari struct, akan di-`DROP COLUMN`.

```rust
use my_database_manager::{create_table, sync_table};

// Di dalam main() setelah mendapatkan pool...

// Pastikan urutan: tabel parent (tanpa FK) dibuat lebih dulu
sync_table::<User>(&pool, &config.driver).await.unwrap();
sync_table::<Post>(&pool, &config.driver).await.unwrap();
sync_table::<UserRole>(&pool, &config.driver).await.unwrap();
```

> **⚠️ Warning:** Hati-hati saat menggunakan `sync_table` di database *Production*. Penghapusan field pada Struct akan mengeksekusi `DROP COLUMN` yang **menghapus data** pada kolom tersebut secara permanen.

---

### 5. Menjalankan Unit Test

Library ini dilengkapi dengan integration test di folder `tests/`. Untuk menjalankannya:

```bash
cargo test
```

Output yang diharapkan:

```text
running 4 tests
test test_auto_derive_creates_correct_columns ... ok
test test_sync_adds_new_columns              ... ok
test test_sync_drops_removed_columns         ... ok
test test_database_manager_full_flow         ... ok

test result: ok. 4 passed; 0 failed
```

---

---

## Dependency

Tambahkan ke `Cargo.toml` proyek Anda:

```toml
[dependencies]
my_database_manager = { path = "../my_database_manager" }
tokio = { version = "1", features = ["full"] }
```

---

## 6. ORM Generik (CRUD Otomatis)

Selain membuat dan menyinkronkan skema, library ini juga menyediakan ORM generik melalui trait `OrmModel`.
Implementasikan trait ini pada struct Anda untuk mendapatkan akses ke fungsi `insert`, `update`, `delete`, `find_one`, `find_all`, dan `find_paginated`.

### Implementasi `OrmModel`

```rust
use my_database_manager::{Model, OrmModel};
use sqlx::{Row, any::AnyRow};

#[derive(Debug, Clone, Model)]
#[table("products")]
pub struct Product {
    id: i32,
    name: String,
    price: Option<f64>,
    stock: i32,
}

impl OrmModel for Product {
    // Kembalikan nilai id sebagai i64
    fn get_id(&self) -> i64 {
        self.id as i64
    }

    // Nilai untuk INSERT — urutannya harus sesuai FIELDS_INSERT
    fn insert_values(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.price.map(|p| p.to_string()).unwrap_or_default(),
            self.stock.to_string(),
        ]
    }

    // Bangun struct dari baris hasil query
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Product {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            price: row.try_get::<f64, _>("price").ok(),
            stock: row.try_get("stock")?,
        })
    }
}
```

### Menggunakan Fungsi ORM

```rust
use my_database_manager::{
    insert, update, delete,
    find_one, find_all, find_paginated, QueryFilter,
};

// INSERT
let p = Product { id: 0, name: "Laptop".into(), price: Some(1500.0), stock: 10 };
insert(&pool, &p, &config.driver).await?;

// UPDATE
let updated = Product { id: 1, name: "Laptop Pro".into(), price: Some(1800.0), stock: 5 };
update(&pool, &updated, &config.driver).await?;

// DELETE berdasarkan id
delete::<Product>(&pool, 1).await?;

// FIND ONE berdasarkan id
let result: Option<Product> = find_one::<Product>(&pool, 1).await?;

// FIND ALL (tanpa filter)
let all: Vec<Product> = find_all::<Product>(&pool, &config.driver, None).await?;

// FIND PAGINATED DENGAN FILTER & SEARCH
// Mencari produk yang mengandung kata "Apple", stok persis 10, urutkan harga menurun
let filter = QueryFilter::new()
    .like("name", "Apple")
    .exact("stock", "10")
    .order("price DESC");

// Ambil halaman ke-1, 10 data per halaman
let page = find_paginated::<Product>(&pool, &config.driver, 1, 10, Some(&filter)).await?;
println!("Total: {}, Halaman: {}/{}", page.total, page.page, page.total_pages);
for item in page.data {
    println!("  - {:?}", item);
}
```

### Struct `PaginatedResult<T>`

| Field | Tipe | Keterangan |
| --- | --- | --- |
| `data` | `Vec<T>` | Data pada halaman ini |
| `page` | `u64` | Nomor halaman saat ini |
| `per_page` | `u64` | Jumlah data per halaman |
| `total` | `u64` | Total seluruh record |
| `total_pages` | `u64` | Total halaman tersedia |

> **Catatan:** Nilai pada `insert_values()` dan `update_values()` dikirim sebagai `String` dan akan di-cast oleh database. Untuk performa optimal di PostgreSQL, gunakan tipe yang sesuai.

---

## 7. ORM Relasional — Get Data Berdasarkan Relasi

Library ini menyediakan fungsi-fungsi untuk mengambil data berdasarkan relasi antar tabel:
**One-to-Many**, **Many-to-Many**, dan **eager loading** (parent + children sekaligus).

### Setup Contoh Model

```rust
use my_database_manager::{Model, OrmModel};
use sqlx::{Row, any::AnyRow};

// ── Parent: User ────────────────────────────────────────────────
#[derive(Debug, Clone, Model)]
#[table("users")]
pub struct User {
    id: i32,
    name: String,
}

impl OrmModel for User {
    fn get_id(&self) -> i64 { self.id as i64 }
    fn insert_values(&self) -> Vec<String> { vec![self.name.clone()] }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(User { id: row.try_get("id")?, name: row.try_get("name")? })
    }
}

// ── Child: Post (berelasi ke User via user_id) ───────────────────
#[derive(Debug, Clone, Model)]
#[table("posts")]
pub struct Post {
    id: i32,
    title: String,
    user_id: i32,
}

impl OrmModel for Post {
    fn get_id(&self) -> i64 { self.id as i64 }
    fn insert_values(&self) -> Vec<String> {
        vec![self.title.clone(), self.user_id.to_string()]
    }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Post {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            user_id: row.try_get("user_id")?,
        })
    }
}

// ── Role (untuk relasi Many-to-Many dengan User) ─────────────────
#[derive(Debug, Clone, Model)]
#[table("roles")]
pub struct Role {
    id: i32,
    name: String,
}

impl OrmModel for Role {
    fn get_id(&self) -> i64 { self.id as i64 }
    fn insert_values(&self) -> Vec<String> { vec![self.name.clone()] }
    fn from_row(row: AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Role { id: row.try_get("id")?, name: row.try_get("name")? })
    }
}
```

---

### `find_by` — Filter Berdasarkan Kolom (termasuk FK)

Mengambil semua record di mana `column = value`. Paling sederhana untuk filter FK.

```rust
use my_database_manager::find_by;

// Ambil semua Post milik user_id = 5
let posts: Vec<Post> = find_by::<Post>(&pool, "user_id", "5").await?;

// Ambil semua User dengan nama "Alice"
let users: Vec<User> = find_by::<User>(&pool, "name", "Alice").await?;
```

---

### `find_by_paginated` — Filter dengan Paginasi

```rust
use my_database_manager::find_by_paginated;

// Ambil halaman 1 dari semua Post milik user_id = 5 (10 per halaman)
let result = find_by_paginated::<Post>(&pool, "user_id", "5", 1, 10).await?;

println!("Post user 5 — halaman {}/{}", result.page, result.total_pages);
println!("Total: {} post", result.total);
for post in result.data {
    println!("  - {}", post.title);
}
```

---

### `find_related` — One-to-Many

Mengambil semua record *Child* yang berelasi ke satu *Parent* melalui *foreign key*.

```rust
use my_database_manager::find_related;

// Ambil semua Post milik User dengan id = 3
let posts: Vec<Post> = find_related::<Post>(&pool, "user_id", 3).await?;

println!("User 3 memiliki {} post:", posts.len());
for p in &posts {
    println!("  - {} (id={})", p.title, p.id);
}
```

---

### `find_one_with_related` — Eager Load (Parent + Children)

Mengambil satu record *Parent* sekaligus semua *Children* miliknya dalam satu panggilan.
Hasilnya dikemas dalam struct `WithChildren<P, C>`.

```rust
use my_database_manager::{find_one_with_related, WithChildren};

// Ambil User id=1 beserta semua Post miliknya
let result: Option<WithChildren<User, Post>> =
    find_one_with_related::<User, Post>(&pool, 1, "user_id").await?;

match result {
    None => println!("User tidak ditemukan"),
    Some(data) => {
        println!("User  : {:?}", data.parent);
        println!("Posts ({}):", data.children.len());
        for post in &data.children {
            println!("  - {}", post.title);
        }
    }
}
```

#### Struct `WithChildren<P, C>`

| Field | Tipe | Keterangan |
| --- | --- | --- |
| `parent` | `P` | Record parent (misalnya: `User`) |
| `children` | `Vec<C>` | Semua child yang berelasi (misalnya: `Vec<Post>`) |

---

### `find_many_to_many` — Many-to-Many via Pivot Table

Mengambil semua record `T` yang terhubung melalui tabel pivot.

**Skema pivot yang diperlukan:**

```sql
CREATE TABLE user_roles (
    user_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, role_id)
);
```

```rust
use my_database_manager::find_many_to_many;

// Ambil semua Role milik User id = 5
// pivot_table = "user_roles"
// pivot_fk    = "role_id"   ← kolom yang menunjuk ke tabel Role
// pivot_ref   = "user_id"   ← kolom yang menjadi filter
let roles: Vec<Role> = find_many_to_many::<Role>(
    &pool,
    "user_roles",   // tabel pivot
    "role_id",      // FK ke tabel target (roles.id)
    "user_id",      // kolom filter
    5,              // nilai user_id yang dicari
).await?;

println!("User 5 memiliki {} role:", roles.len());
for r in &roles {
    println!("  - {}", r.name);
}
```

---

### Ringkasan Fungsi Relasional

| Fungsi | Relasi | Keterangan |
| --- | --- | --- |
| `find_by::<T>(pool, column, value)` | Filter kolom apa saja | Cari semua T di mana kolom = nilai |
| `find_by_paginated::<T>(pool, col, val, page, size)` | Filter + paginasi | Versi paginated dari `find_by` |
| `find_related::<Child>(pool, fk_column, parent_id)` | One-to-Many | Ambil semua Child milik satu Parent |
| `find_one_with_related::<P, C>(pool, id, fk)` | One-to-Many eager load | Parent + semua Children sekaligus |
| `find_many_to_many::<T>(pool, pivot, pivot_fk, pivot_ref, id)` | Many-to-Many | Ambil T melalui tabel pivot |

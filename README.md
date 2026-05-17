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
    find_one, find_all, find_paginated,
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

// FIND ALL
let all: Vec<Product> = find_all::<Product>(&pool).await?;

// FIND PAGINATED (halaman ke-2, 10 data per halaman)
let page = find_paginated::<Product>(&pool, 2, 10).await?;
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

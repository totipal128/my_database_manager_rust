extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

/// Ekstrak nama tipe paling dalam dari suatu tipe Rust.
/// Contoh: "i32" → "i32", "Option<String>" → ("String", true), "Option<i32>" → ("i32", true)
fn extract_type_info(ty: &Type) -> (&'static str, bool) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();

            // Cek apakah ini Option<T>
            if type_name == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        let (inner_sql, _) = extract_type_info(inner_ty);
                        return (inner_sql, true); // nullable = true
                    }
                }
                return ("VARCHAR(255)", true);
            }

            // Mapping tipe Rust → SQL
            let sql = match type_name.as_str() {
                "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => "INT",
                "i64" | "u64" | "isize" | "usize" => "BIGINT",
                "f32" => "FLOAT",
                "f64" => "DOUBLE",
                "bool" => "BOOLEAN",
                "String" | "str" => "VARCHAR(255)",
                "NaiveDate" => "DATE",
                "NaiveDateTime" | "DateTime" => "DATETIME",
                "Uuid" => "VARCHAR(36)",
                "Vec" => "TEXT",
                _ => "TEXT",
            };
            return (sql, false); // nullable = false
        }
    }
    ("TEXT", false)
}

/// Derive macro untuk trait Model.
///
/// Secara otomatis menghasilkan implementasi `Model` berdasarkan field-field dalam struct:
/// - Field `id` dijadikan `PRIMARY KEY` dan dikecualikan dari `FIELDS_INSERT`
/// - Tipe Rust dikonversi ke tipe SQL secara otomatis
/// - Field `Option<T>` menghasilkan kolom nullable (tanpa NOT NULL)
/// - Nama tabel secara default adalah nama struct lowercase + "s", dapat di-override dengan `#[table("nama_tabel")]`
///
/// # Contoh
/// ```rust
/// #[derive(Debug, Clone, Model)]
/// #[table("users")]
/// struct User {
///     id: i32,
///     name: String,
///     email: Option<String>,
///     age: i32,
/// }
/// ```
#[proc_macro_derive(Model, attributes(table))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    // Default table name: nama struct lowercase + "s"
    let mut table_name = name.to_string().to_lowercase() + "s";

    // Cek atribut #[table("nama_custom")]
    for attr in &input.attrs {
        if attr.path().is_ident("table") {
            if let Ok(lit) = attr.parse_args::<syn::LitStr>() {
                table_name = lit.value();
            }
        }
    }

    let mut fields_insert: Vec<String> = Vec::new();
    let mut fields_decl: Vec<String> = Vec::new();

    if let Data::Struct(data) = input.data {
        if let Fields::Named(fields) = data.fields {
            for field in fields.named {
                let field_name = field.ident.unwrap().to_string();
                let (sql_type, is_nullable) = extract_type_info(&field.ty);

                if field_name == "id" {
                    // id selalu PRIMARY KEY, tidak masuk FIELDS_INSERT
                    fields_decl.push(format!("id INT PRIMARY KEY"));
                } else {
                    fields_insert.push(field_name.clone());
                    let nullability = if is_nullable { "" } else { " NOT NULL" };
                    fields_decl.push(format!("{} {}{}", field_name, sql_type, nullability));
                }
            }
        }
    }

    let expanded = quote! {
        impl Model for #name {
            const TABLE: &'static str = #table_name;
            const FIELDS_INSERT: &'static [&'static str] = &[
                #( #fields_insert ),*
            ];
            const FIELDS_DECLARATION: &'static [&'static str] = &[
                #( #fields_decl ),*
            ];
            const FOREIGN_KEYS: &'static [&'static str] = &[];
        }
    };

    TokenStream::from(expanded)
}

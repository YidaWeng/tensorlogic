//! SQLite database backend for schema storage.

use rusqlite::{params, Connection, Result as SqliteResult};

use super::{SchemaDatabase, SchemaDatabaseSQL, SchemaId, SchemaMetadata, SchemaVersion};
use crate::{AdapterError, SymbolTable};

mod io;

/// SQLite database backend for schema storage.
///
/// This implementation provides persistent storage using SQLite.
/// The database schema is automatically created on first use.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "sqlite")]
/// # {
/// use tensorlogic_adapters::{SQLiteDatabase, SchemaDatabase, SymbolTable, DomainInfo};
///
/// let mut db = SQLiteDatabase::new(":memory:").expect("open");
/// let mut table = SymbolTable::new();
/// table.add_domain(DomainInfo::new("Person", 100)).expect("add");
///
/// let id = db.store_schema("test", &table).expect("store");
/// let loaded = db.load_schema(id).expect("load");
/// # }
/// ```
pub struct SQLiteDatabase {
    pub(super) conn: Connection,
}

impl SQLiteDatabase {
    /// Create a new SQLite database at the given path.
    ///
    /// Use `:memory:` for an in-memory database (testing).
    pub fn new(path: &str) -> Result<Self, AdapterError> {
        let conn = Connection::open(path).map_err(|e| {
            AdapterError::InvalidOperation(format!("Failed to open SQLite database: {}", e))
        })?;

        let mut db = Self { conn };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Initialize the database schema (create tables if they don't exist).
    fn initialize_schema(&mut self) -> Result<(), AdapterError> {
        for sql in SchemaDatabaseSQL::create_tables_sql() {
            self.conn.execute(&sql, []).map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to create tables: {}", e))
            })?;
        }
        Ok(())
    }
}

impl SchemaDatabase for SQLiteDatabase {
    fn store_schema(&mut self, name: &str, table: &SymbolTable) -> Result<SchemaId, AdapterError> {
        let (schema_id, _version) = io::store_schema_metadata(&mut self.conn, name)?;

        io::store_domains(&mut self.conn, schema_id, table)?;
        io::store_predicates(&mut self.conn, schema_id, table)?;
        io::store_variables(&mut self.conn, schema_id, table)?;

        Ok(SchemaId(schema_id as u64))
    }

    fn load_schema(&self, id: SchemaId) -> Result<SymbolTable, AdapterError> {
        let schema_id = id.0 as i64;

        // Verify schema exists
        let _: i64 = self
            .conn
            .query_row(
                "SELECT id FROM schemas WHERE id = ?",
                params![schema_id],
                |row| row.get(0),
            )
            .map_err(|_| {
                AdapterError::InvalidOperation(format!("Schema with ID {:?} not found", id))
            })?;

        let mut table = SymbolTable::new();
        table.domains = io::load_domains(&self.conn, schema_id)?;
        table.predicates = io::load_predicates(&self.conn, schema_id)?;
        table.variables = io::load_variables(&self.conn, schema_id)?;

        Ok(table)
    }

    fn load_schema_by_name(&self, name: &str) -> Result<SymbolTable, AdapterError> {
        let schema_id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM schemas WHERE name = ? ORDER BY version DESC LIMIT 1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|_| AdapterError::InvalidOperation(format!("Schema '{}' not found", name)))?;

        self.load_schema(SchemaId(schema_id as u64))
    }

    fn list_schemas(&self) -> Result<Vec<SchemaMetadata>, AdapterError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT s.id, s.name, s.version, s.created_at, s.updated_at, s.description,
                       (SELECT COUNT(*) FROM domains WHERE schema_id = s.id) as num_domains,
                       (SELECT COUNT(*) FROM predicates WHERE schema_id = s.id) as num_predicates,
                       (SELECT COUNT(*) FROM variables WHERE schema_id = s.id) as num_variables
                FROM schemas s
                ORDER BY s.name, s.version DESC
                "#,
            )
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to prepare query: {}", e))
            })?;

        let schemas = stmt
            .query_map([], |row| {
                Ok(SchemaMetadata {
                    id: SchemaId(row.get::<_, i64>(0)? as u64),
                    name: row.get(1)?,
                    version: row.get(2)?,
                    created_at: row.get::<_, i64>(3)? as u64,
                    updated_at: row.get::<_, i64>(4)? as u64,
                    num_domains: row.get::<_, i64>(6)? as usize,
                    num_predicates: row.get::<_, i64>(7)? as usize,
                    num_variables: row.get::<_, i64>(8)? as usize,
                    description: row.get(5)?,
                })
            })
            .map_err(|e| AdapterError::InvalidOperation(format!("Failed to query schemas: {}", e)))?
            .collect::<SqliteResult<_>>()
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to collect schemas: {}", e))
            })?;

        Ok(schemas)
    }

    fn delete_schema(&mut self, id: SchemaId) -> Result<(), AdapterError> {
        let schema_id = id.0 as i64;
        let affected = self
            .conn
            .execute("DELETE FROM schemas WHERE id = ?", params![schema_id])
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to delete schema: {}", e))
            })?;

        if affected == 0 {
            return Err(AdapterError::InvalidOperation(format!(
                "Schema with ID {:?} not found",
                id
            )));
        }

        Ok(())
    }

    fn search_schemas(&self, pattern: &str) -> Result<Vec<SchemaMetadata>, AdapterError> {
        let search_pattern = format!("%{}%", pattern);
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT s.id, s.name, s.version, s.created_at, s.updated_at, s.description,
                       (SELECT COUNT(*) FROM domains WHERE schema_id = s.id) as num_domains,
                       (SELECT COUNT(*) FROM predicates WHERE schema_id = s.id) as num_predicates,
                       (SELECT COUNT(*) FROM variables WHERE schema_id = s.id) as num_variables
                FROM schemas s
                WHERE s.name LIKE ?
                ORDER BY s.name, s.version DESC
                "#,
            )
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to prepare query: {}", e))
            })?;

        let schemas = stmt
            .query_map(params![search_pattern], |row| {
                Ok(SchemaMetadata {
                    id: SchemaId(row.get::<_, i64>(0)? as u64),
                    name: row.get(1)?,
                    version: row.get(2)?,
                    created_at: row.get::<_, i64>(3)? as u64,
                    updated_at: row.get::<_, i64>(4)? as u64,
                    num_domains: row.get::<_, i64>(6)? as usize,
                    num_predicates: row.get::<_, i64>(7)? as usize,
                    num_variables: row.get::<_, i64>(8)? as usize,
                    description: row.get(5)?,
                })
            })
            .map_err(|e| AdapterError::InvalidOperation(format!("Failed to query schemas: {}", e)))?
            .collect::<SqliteResult<_>>()
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to collect schemas: {}", e))
            })?;

        Ok(schemas)
    }

    fn get_schema_history(&self, name: &str) -> Result<Vec<SchemaVersion>, AdapterError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT version, created_at, id
                FROM schemas
                WHERE name = ?
                ORDER BY version ASC
                "#,
            )
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to prepare query: {}", e))
            })?;

        let versions: Vec<SchemaVersion> = stmt
            .query_map(params![name], |row| {
                let version: u32 = row.get(0)?;
                let timestamp: i64 = row.get(1)?;
                let schema_id: i64 = row.get(2)?;

                Ok(SchemaVersion {
                    version,
                    timestamp: timestamp as u64,
                    description: format!("Version {}", version),
                    schema_id: SchemaId(schema_id as u64),
                })
            })
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to query versions: {}", e))
            })?
            .collect::<SqliteResult<_>>()
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to collect versions: {}", e))
            })?;

        if versions.is_empty() {
            return Err(AdapterError::InvalidOperation(format!(
                "Schema '{}' not found",
                name
            )));
        }

        Ok(versions)
    }
}

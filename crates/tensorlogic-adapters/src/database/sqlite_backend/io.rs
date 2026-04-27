//! Internal SQLite row serialization and deserialization helpers.

use rusqlite::{params, Connection, Result as SqliteResult};

use super::SchemaDatabaseSQL;
use crate::{AdapterError, DomainInfo, PredicateInfo, SymbolTable};

/// Get current timestamp (Unix epoch seconds).
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("SystemTime after UNIX_EPOCH")
        .as_secs()
}

/// Store schema metadata and return schema_id.
pub(super) fn store_schema_metadata(
    conn: &mut Connection,
    name: &str,
) -> Result<(i64, u32), AdapterError> {
    let now = current_timestamp() as i64;

    // Check if schema exists
    let existing_version: Option<u32> = conn
        .query_row(
            "SELECT MAX(version) FROM schemas WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let version = existing_version.map(|v| v + 1).unwrap_or(1);

    conn.execute(
        "INSERT INTO schemas (name, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![name, version, now, now],
    )
    .map_err(|e| AdapterError::InvalidOperation(format!("Failed to insert schema: {}", e)))?;

    let schema_id = conn.last_insert_rowid();
    Ok((schema_id, version))
}

/// Store domains for a schema.
pub(super) fn store_domains(
    conn: &mut Connection,
    schema_id: i64,
    table: &SymbolTable,
) -> Result<(), AdapterError> {
    for (name, domain) in &table.domains {
        let metadata_json = serde_json::to_string(&domain.metadata).ok();
        conn.execute(
            SchemaDatabaseSQL::insert_domain_sql(),
            params![
                schema_id,
                name,
                domain.cardinality as i64,
                domain.description.as_ref(),
                metadata_json
            ],
        )
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to insert domain: {}", e)))?;
    }
    Ok(())
}

/// Store predicates for a schema.
pub(super) fn store_predicates(
    conn: &mut Connection,
    schema_id: i64,
    table: &SymbolTable,
) -> Result<(), AdapterError> {
    for (name, predicate) in &table.predicates {
        let constraints_json = serde_json::to_string(&predicate.constraints).ok();
        let metadata_json = serde_json::to_string(&predicate.metadata).ok();

        conn.execute(
            SchemaDatabaseSQL::insert_predicate_sql(),
            params![
                schema_id,
                name,
                predicate.arg_domains.len() as i64,
                predicate.description.as_ref(),
                constraints_json,
                metadata_json
            ],
        )
        .map_err(|e| {
            AdapterError::InvalidOperation(format!("Failed to insert predicate: {}", e))
        })?;

        let predicate_id = conn.last_insert_rowid();

        // Store argument domains
        for (position, domain_name) in predicate.arg_domains.iter().enumerate() {
            conn.execute(
                SchemaDatabaseSQL::insert_predicate_arg_sql(),
                params![predicate_id, position as i64, domain_name],
            )
            .map_err(|e| {
                AdapterError::InvalidOperation(format!(
                    "Failed to insert predicate argument: {}",
                    e
                ))
            })?;
        }
    }
    Ok(())
}

/// Store variables for a schema.
pub(super) fn store_variables(
    conn: &mut Connection,
    schema_id: i64,
    table: &SymbolTable,
) -> Result<(), AdapterError> {
    for (var_name, domain_name) in &table.variables {
        conn.execute(
            SchemaDatabaseSQL::insert_variable_sql(),
            params![schema_id, var_name, domain_name],
        )
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to insert variable: {}", e)))?;
    }
    Ok(())
}

/// Load domains for a schema.
pub(super) fn load_domains(
    conn: &Connection,
    schema_id: i64,
) -> Result<indexmap::IndexMap<String, DomainInfo>, AdapterError> {
    let mut stmt = conn
        .prepare(SchemaDatabaseSQL::select_domains_sql())
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to prepare query: {}", e)))?;

    let domains = stmt
        .query_map(params![schema_id], |row| {
            let name: String = row.get(0)?;
            let cardinality: i64 = row.get(1)?;
            let description: Option<String> = row.get(2)?;
            let metadata_json: Option<String> = row.get(3)?;

            let mut domain = DomainInfo::new(&name, cardinality as usize);
            if let Some(desc) = description {
                domain = domain.with_description(desc);
            }
            if let Some(meta_str) = metadata_json {
                if let Ok(metadata) = serde_json::from_str(&meta_str) {
                    domain.metadata = metadata;
                }
            }

            Ok((name, domain))
        })
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to query domains: {}", e)))?
        .collect::<SqliteResult<_>>()
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to collect domains: {}", e)))?;

    Ok(domains)
}

/// Load predicates for a schema.
pub(super) fn load_predicates(
    conn: &Connection,
    schema_id: i64,
) -> Result<indexmap::IndexMap<String, PredicateInfo>, AdapterError> {
    let mut stmt = conn
        .prepare(SchemaDatabaseSQL::select_predicates_sql())
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to prepare query: {}", e)))?;

    let predicates = stmt
        .query_map(params![schema_id], |row| {
            let predicate_id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let _arity: i64 = row.get(2)?;
            let description: Option<String> = row.get(3)?;
            let constraints_json: Option<String> = row.get(4)?;
            let metadata_json: Option<String> = row.get(5)?;

            Ok((
                predicate_id,
                name,
                description,
                constraints_json,
                metadata_json,
            ))
        })
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to query predicates: {}", e)))?
        .collect::<SqliteResult<Vec<_>>>()
        .map_err(|e| {
            AdapterError::InvalidOperation(format!("Failed to collect predicates: {}", e))
        })?;

    let mut result = indexmap::IndexMap::new();

    for (predicate_id, name, description, constraints_json, metadata_json) in predicates {
        // Load argument domains
        let mut arg_stmt = conn
            .prepare(SchemaDatabaseSQL::select_predicate_args_sql())
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to prepare query: {}", e))
            })?;

        let arg_domains: Vec<String> = arg_stmt
            .query_map(params![predicate_id], |row| {
                let _position: i64 = row.get(0)?;
                let domain_name: String = row.get(1)?;
                Ok(domain_name)
            })
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to query predicate args: {}", e))
            })?
            .collect::<SqliteResult<_>>()
            .map_err(|e| {
                AdapterError::InvalidOperation(format!("Failed to collect predicate args: {}", e))
            })?;

        let mut predicate = PredicateInfo::new(&name, arg_domains);
        if let Some(desc) = description {
            predicate = predicate.with_description(desc);
        }
        if let Some(constraints_str) = constraints_json {
            if let Ok(constraints) = serde_json::from_str(&constraints_str) {
                predicate.constraints = constraints;
            }
        }
        if let Some(meta_str) = metadata_json {
            if let Ok(metadata) = serde_json::from_str(&meta_str) {
                predicate.metadata = metadata;
            }
        }

        result.insert(name, predicate);
    }

    Ok(result)
}

/// Load variables for a schema.
pub(super) fn load_variables(
    conn: &Connection,
    schema_id: i64,
) -> Result<indexmap::IndexMap<String, String>, AdapterError> {
    let mut stmt = conn
        .prepare("SELECT name, domain_name FROM variables WHERE schema_id = ?")
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to prepare query: {}", e)))?;

    let variables = stmt
        .query_map(params![schema_id], |row| {
            let name: String = row.get(0)?;
            let domain_name: String = row.get(1)?;
            Ok((name, domain_name))
        })
        .map_err(|e| AdapterError::InvalidOperation(format!("Failed to query variables: {}", e)))?
        .collect::<SqliteResult<_>>()
        .map_err(|e| {
            AdapterError::InvalidOperation(format!("Failed to collect variables: {}", e))
        })?;

    Ok(variables)
}

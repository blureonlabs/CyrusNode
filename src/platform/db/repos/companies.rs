//! Companies repository — tenant-scoped CRUD for the `companies` table.

use sqlx::PgPool;
use uuid::Uuid;

use crate::platform::db::{CompanyRow, DbError};

/// Read/write access to the `companies` table. Cheap to clone.
#[derive(Debug, Clone)]
pub struct CompaniesRepo {
    pool: PgPool,
}

impl CompaniesRepo {
    /// Construct a repo backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a company, or return the existing one (matched on `(tenant_id, host)`).
    ///
    /// Idempotent: submitting the same host twice never creates two rows.
    /// On conflict we do a no-op `SET url = companies.url` so `RETURNING *`
    /// fires for both insert and update paths.
    pub async fn create_or_get(
        &self,
        tenant_id: Uuid,
        url: &str,
        host: &str,
    ) -> Result<CompanyRow, DbError> {
        let row = sqlx::query_as::<_, CompanyRow>(
            r#"
            INSERT INTO companies (tenant_id, url, host, status)
            VALUES ($1, $2, $3, 'discovered')
            ON CONFLICT (tenant_id, host) DO UPDATE
                SET url = companies.url -- no-op to force RETURNING
            RETURNING *
            "#,
        )
        .bind(tenant_id)
        .bind(url)
        .bind(host)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Fetch a company by id within the tenant. Excludes soft-deleted rows.
    /// Returns `DbError::NotFound` if no live row matches.
    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> Result<CompanyRow, DbError> {
        let row = sqlx::query_as::<_, CompanyRow>(
            "SELECT * FROM companies WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;
        Ok(row)
    }

    /// List companies for a tenant filtered by `status`, newest-first by
    /// `first_seen_at`. Excludes soft-deleted rows.
    pub async fn list_by_status(
        &self,
        tenant_id: Uuid,
        status: &str,
        limit: i64,
    ) -> Result<Vec<CompanyRow>, DbError> {
        let rows = sqlx::query_as::<_, CompanyRow>(
            r#"
            SELECT * FROM companies
            WHERE tenant_id = $1 AND status = $2 AND deleted_at IS NULL
            ORDER BY first_seen_at DESC
            LIMIT $3
            "#,
        )
        .bind(tenant_id)
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update the company's status. Returns `DbError::NotFound` if no row matched
    /// (e.g. wrong tenant, or already hard-deleted).
    pub async fn set_status(&self, tenant_id: Uuid, id: Uuid, status: &str) -> Result<(), DbError> {
        let n = sqlx::query(
            "UPDATE companies SET status = $3, updated_at = now() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(id)
        .bind(status)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if n == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }
}

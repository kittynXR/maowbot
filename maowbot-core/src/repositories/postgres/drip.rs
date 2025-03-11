// maowbot-core/src/repositories/postgres/drip.rs
//
// A more complete reference implementation for storing Drip data in the new tables.
// Uses actual sqlx queries. Adjust as needed for your real environment.

use std::sync::Mutex;
use uuid::Uuid;
use chrono::{Utc, DateTime};
use sqlx::{Pool, Postgres, Row};
use crate::Error;
use crate::models::{DripAvatar, DripFitParam, DripProp, DripFit};

/// Drip repository with a reference to the Postgres pool, plus an optional
/// in-memory "current avatar" concept to illustrate how you might track
/// whichever avatar is actively worn.
pub struct DripRepository {
    pool: Pool<Postgres>,
    current_avatar: Mutex<Option<crate::models::drip::DripAvatar>>,
}

impl DripRepository {
    /// Create a new repository, passing in the sqlx Postgres pool.
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self {
            pool,
            current_avatar: Mutex::new(None),
        }
    }

    /// Get or set the current avatar in memory. In a real build, you might
    /// set this whenever `/avatar/change` is detected.
    pub fn current_avatar(&self) -> Result<Option<DripAvatar>, Error> {
        Ok(self.current_avatar.lock().unwrap().clone())
    }

    /// For demonstration: set a new "current avatar" in memory and upsert
    /// into drip_avatars if needed. This is not in the original code but
    /// can help you illustrate detection of the "active" avatar.
    pub async fn set_current_avatar(
        &self,
        user_id: Uuid,
        vrchat_avatar_id: &str,
        vrchat_avatar_name: &str,
    ) -> Result<(), Error> {
        // 1) Check if we already have an entry in drip_avatars
        let row_opt = sqlx::query(
            r#"
            SELECT drip_avatar_id, user_id, vrchat_avatar_id, vrchat_avatar_name, local_name
            FROM drip_avatars
            WHERE vrchat_avatar_id = $1 AND user_id = $2
            "#,
        )
            .bind(vrchat_avatar_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        let drip_avatar = if let Some(row) = row_opt {
            // We already have it
            DripAvatar {
                drip_avatar_id: row.try_get("drip_avatar_id")?,
                user_id: row.try_get("user_id")?,
                vrchat_avatar_id: row.try_get("vrchat_avatar_id")?,
                vrchat_avatar_name: row.try_get("vrchat_avatar_name")?,
                local_name: row.try_get("local_name")?,
            }
        } else {
            // Insert a new row
            let inserted = sqlx::query(
                r#"
                INSERT INTO drip_avatars (
                  drip_avatar_id, user_id, vrchat_avatar_id, vrchat_avatar_name,
                  local_name, created_at, updated_at
                )
                VALUES (uuid_generate_v4(), $1, $2, $3, NULL, now(), now())
                RETURNING drip_avatar_id, user_id, vrchat_avatar_id, vrchat_avatar_name, local_name
                "#,
            )
                .bind(user_id)
                .bind(vrchat_avatar_id)
                .bind(vrchat_avatar_name)
                .fetch_one(&self.pool)
                .await?;

            DripAvatar {
                drip_avatar_id: inserted.try_get("drip_avatar_id")?,
                user_id: inserted.try_get("user_id")?,
                vrchat_avatar_id: inserted.try_get("vrchat_avatar_id")?,
                vrchat_avatar_name: inserted.try_get("vrchat_avatar_name")?,
                local_name: inserted.try_get("local_name")?,
            }
        };

        // 2) Update memory
        *self.current_avatar.lock().unwrap() = Some(drip_avatar);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // PREFIX RULES
    // ─────────────────────────────────────────────────────────────────────────────

    /// Insert a prefix rule to ignore
    pub async fn add_prefix_rule_ignore(&self, avatar_id: &Uuid, prefix: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO drip_avatar_prefix_rules (
              drip_avatar_prefix_rule_id, drip_avatar_id, rule_type, prefix,
              created_at, updated_at
            )
            VALUES (uuid_generate_v4(), $1, 'ignore', $2, now(), now())
            "#,
        )
            .bind(avatar_id)
            .bind(prefix)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Insert a prefix rule to strip
    pub async fn add_prefix_rule_strip(&self, avatar_id: &Uuid, prefix: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO drip_avatar_prefix_rules (
              drip_avatar_prefix_rule_id, drip_avatar_id, rule_type, prefix,
              created_at, updated_at
            )
            VALUES (uuid_generate_v4(), $1, 'strip', $2, now(), now())
            "#,
        )
            .bind(avatar_id)
            .bind(prefix)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // AVATAR METHODS
    // ─────────────────────────────────────────────────────────────────────────────

    /// Update local avatar name for the given drip_avatar_id
    pub async fn update_local_avatar_name(&self, avatar_id: &Uuid, new_name: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE drip_avatars
            SET local_name = $1,
                updated_at = now()
            WHERE drip_avatar_id = $2
            "#,
        )
            .bind(new_name)
            .bind(avatar_id)
            .execute(&self.pool)
            .await?;

        // If that matches the "current" in memory, update that too:
        if let Some(mut av) = self.current_avatar.lock().unwrap().clone() {
            if av.drip_avatar_id == *avatar_id {
                av.local_name = Some(new_name.to_string());
                *self.current_avatar.lock().unwrap() = Some(av);
            }
        }

        Ok(())
    }

    /// List all avatars for the user from the DB. If you want only the
    /// "current" user, pass user_id. Or omit for all.
    /// Here, we just select for all (or you can limit by user_id).
    pub async fn list_avatars(&self) -> Result<Vec<DripAvatar>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT drip_avatar_id, user_id, vrchat_avatar_id, vrchat_avatar_name, local_name
            FROM drip_avatars
            ORDER BY created_at DESC
            "#,
        )
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(DripAvatar {
                drip_avatar_id: r.try_get("drip_avatar_id")?,
                user_id: r.try_get("user_id")?,
                vrchat_avatar_id: r.try_get("vrchat_avatar_id")?,
                vrchat_avatar_name: r.try_get("vrchat_avatar_name")?,
                local_name: r.try_get("local_name")?,
            });
        }

        Ok(out)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // FITS
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create a new fit row in drip_fits
    pub async fn create_fit(&self, avatar_id: &Uuid, fit_name: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO drip_fits (
              drip_fit_id, drip_avatar_id, fit_name,
              created_at, updated_at
            )
            VALUES (uuid_generate_v4(), $1, $2, now(), now())
            "#,
        )
            .bind(avatar_id)
            .bind(fit_name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Insert a param into drip_fit_params, referencing an existing fit by name.
    /// In practice we must find the drip_fit_id first.
    pub async fn add_fit_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<(), Error> {
        // 1) find the fit
        let fit_row = sqlx::query(
            r#"
            SELECT drip_fit_id FROM drip_fits
            WHERE fit_name = $1
            LIMIT 1
            "#,
        )
            .bind(fit_name)
            .fetch_optional(&self.pool)
            .await?;

        let fit_id = match fit_row {
            Some(r) => r.try_get::<Uuid, _>("drip_fit_id")?,
            None => {
                // If you prefer an error if the fit doesn't exist:
                // return Err(Error::Platform(format!("Fit '{}' not found.", fit_name)));
                // Or automatically create a fit if you want:
                return Err(Error::Platform(format!("Fit '{}' does not exist.", fit_name)));
            }
        };

        // 2) insert param
        sqlx::query(
            r#"
            INSERT INTO drip_fit_params (
              drip_fit_param_id, drip_fit_id, param_name, param_value,
              created_at, updated_at
            )
            VALUES (uuid_generate_v4(), $1, $2, $3, now(), now())
            "#,
        )
            .bind(fit_id)
            .bind(param_name)
            .bind(param_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Delete a param from a fit by param name/value.
    /// In practice, you might only match param_name to remove that param entirely,
    /// ignoring param_value. Adjust as you wish.
    pub async fn del_fit_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<(), Error> {
        // 1) find fit
        let fit_row = sqlx::query(
            r#"
            SELECT drip_fit_id FROM drip_fits
            WHERE fit_name = $1
            LIMIT 1
            "#,
        )
            .bind(fit_name)
            .fetch_optional(&self.pool)
            .await?;

        let fit_id = match fit_row {
            Some(r) => r.try_get::<Uuid, _>("drip_fit_id")?,
            None => {
                return Err(Error::Platform(format!("Fit '{}' not found.", fit_name)));
            }
        };

        // 2) delete
        sqlx::query(
            r#"
            DELETE FROM drip_fit_params
            WHERE drip_fit_id = $1
              AND param_name = $2
              AND param_value = $3
            "#,
        )
            .bind(fit_id)
            .bind(param_name)
            .bind(param_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Retrieve the param_name/param_value pairs from drip_fit_params for the given fit.
    pub async fn get_fit_params(&self, fit_name: &str) -> Result<Vec<DripFitParam>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT p.param_name, p.param_value
            FROM drip_fit_params p
            JOIN drip_fits f ON p.drip_fit_id = f.drip_fit_id
            WHERE f.fit_name = $1
            ORDER BY p.created_at
            "#,
        )
            .bind(fit_name)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::new();
        for row in rows {
            out.push(DripFitParam {
                param_name: row.try_get("param_name")?,
                param_value: row.try_get("param_value")?,
            });
        }
        Ok(out)
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // PARAM DISCOVERY
    // ─────────────────────────────────────────────────────────────────────────────

    /// Check if a param is known for the "current" avatar. In a real build, you'd
    /// do: SELECT 1 FROM drip_avatar_params WHERE drip_avatar_id=... AND param_name=...
    pub async fn is_param_known_for_current_avatar(&self, param_name: String) -> Result<bool, Error> {
        let maybe_av = self.current_avatar()?;
        let av = match maybe_av {
            Some(a) => a,
            None => {
                // If there's no current avatar, we can't confirm;
                // return false or an error:
                return Ok(false);
            }
        };

        let row = sqlx::query(
            r#"
            SELECT 1
            FROM drip_avatar_params
            WHERE drip_avatar_id = $1
              AND param_name = $2
            LIMIT 1
            "#,
        )
            .bind(av.drip_avatar_id)
            .bind(param_name.clone())
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.is_some())
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // PROPS
    // ─────────────────────────────────────────────────────────────────────────────

    /// Insert or find the drip_prop_id for the given prop_name.
    async fn get_or_create_prop_id(&self, prop_name: &str) -> Result<Uuid, Error> {
        // 1) see if it exists
        let existing = sqlx::query(
            r#"
            SELECT drip_prop_id
            FROM drip_props
            WHERE prop_name = $1
            LIMIT 1
            "#,
        )
            .bind(prop_name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = existing {
            return Ok(row.try_get("drip_prop_id")?);
        }

        // 2) otherwise, create a new one
        let inserted = sqlx::query(
            r#"
            INSERT INTO drip_props (
              drip_prop_id, prop_name,
              created_at, updated_at
            )
            VALUES (uuid_generate_v4(), $1, now(), now())
            RETURNING drip_prop_id
            "#,
        )
            .bind(prop_name)
            .fetch_one(&self.pool)
            .await?;

        let prop_id: Uuid = inserted.try_get("drip_prop_id")?;
        Ok(prop_id)
    }

    pub async fn add_prop_param(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<(), Error> {
        let prop_id = self.get_or_create_prop_id(prop_name).await?;

        sqlx::query(
            r#"
            INSERT INTO drip_prop_params (
              drip_prop_param_id, drip_prop_id, param_name, param_value,
              created_at, updated_at
            )
            VALUES (uuid_generate_v4(), $1, $2, $3, now(), now())
            "#,
        )
            .bind(prop_id)
            .bind(param_name)
            .bind(param_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn del_prop_param(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<(), Error> {
        // find prop ID
        let prop_row = sqlx::query(
            r#"
            SELECT drip_prop_id
            FROM drip_props
            WHERE prop_name = $1
            LIMIT 1
            "#,
        )
            .bind(prop_name)
            .fetch_optional(&self.pool)
            .await?;

        let prop_id = match prop_row {
            Some(r) => r.try_get::<Uuid, _>("drip_prop_id")?,
            None => {
                // If prop doesn't exist, no param to delete
                return Ok(()); // or return an Error if you prefer
            }
        };

        // delete param
        sqlx::query(
            r#"
            DELETE FROM drip_prop_params
            WHERE drip_prop_id = $1
              AND param_name = $2
              AND param_value = $3
            "#,
        )
            .bind(prop_id)
            .bind(param_name)
            .bind(param_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Add a timer for a prop. For simplicity, store the `timer_data` as JSONB in drip_prop_timers.
    /// If the prop doesn't exist, create it. If you want multiple timers per prop,
    /// then each call is a new row. Or you can upsert. Adjust to your preference.
    pub async fn add_prop_timer(&self, prop_name: &str, timer_data: &str) -> Result<(), Error> {
        let prop_id = self.get_or_create_prop_id(prop_name).await?;

        sqlx::query(
            r#"
            INSERT INTO drip_prop_timers (
              drip_prop_timer_id, drip_prop_id, timer_data,
              created_at, updated_at
            )
            VALUES (uuid_generate_v4(), $1, $2::jsonb, now(), now())
            "#,
        )
            .bind(prop_id)
            .bind(timer_data)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

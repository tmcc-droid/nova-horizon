//! Trade, refuel, cargo, shipyard (PR-09 / PR-14).

use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;

#[derive(Debug, Clone)]
pub struct CargoStack {
    pub commodity_id: String,
    pub quantity: i32,
}

#[derive(Debug, Clone)]
pub struct MarketRow {
    pub commodity_id: String,
    pub stock: i32,
    pub buy_price: i32,
    pub sell_price: i32,
    pub version: i64,
}

pub async fn list_cargo(pool: &PgPool, ship_id: Uuid) -> Result<Vec<CargoStack>, DbError> {
    let rows = sqlx::query_as::<_, (String, i32)>(
        "SELECT commodity_id, quantity FROM cargo_stacks WHERE ship_id = $1",
    )
    .bind(ship_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(commodity_id, quantity)| CargoStack {
            commodity_id,
            quantity,
        })
        .collect())
}

pub async fn list_market(pool: &PgPool, station_id: &str) -> Result<Vec<MarketRow>, DbError> {
    let rows = sqlx::query_as::<_, (String, i32, i32, i32, i64)>(
        r#"
        SELECT commodity_id, stock, buy_price, sell_price, version
        FROM station_markets WHERE station_id = $1
        "#,
    )
    .bind(station_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(commodity_id, stock, buy_price, sell_price, version)| MarketRow {
                commodity_id,
                stock,
                buy_price,
                sell_price,
                version,
            },
        )
        .collect())
}

pub async fn get_credits(pool: &PgPool, character_id: Uuid) -> Result<i64, DbError> {
    let c = sqlx::query_scalar::<_, i64>("SELECT credits FROM characters WHERE id = $1")
        .bind(character_id)
        .fetch_one(pool)
        .await?;
    Ok(c)
}

/// Buy or sell. Lock order: character → ship → cargo → market.
pub async fn execute_trade(
    pool: &PgPool,
    character_id: Uuid,
    ship_id: Uuid,
    station_id: &str,
    commodity_id: &str,
    side_buy: bool,
    quantity: i32,
    mass_per_unit: f64,
    cargo_capacity_mass: f64,
    daily_cap: i64,
) -> Result<(i64, Vec<CargoStack>, f64), DbError> {
    if quantity <= 0 {
        return Err(DbError::Other("invalid_qty".into()));
    }
    let mut tx = pool.begin().await?;

    let credits: i64 =
        sqlx::query_scalar("SELECT credits FROM characters WHERE id = $1 FOR UPDATE")
            .bind(character_id)
            .fetch_one(&mut *tx)
            .await?;

    let (trade_vol, trade_date): (i64, chrono::NaiveDate) = sqlx::query_as(
        "SELECT trade_volume_day, trade_volume_day_date FROM characters WHERE id = $1",
    )
    .bind(character_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("SELECT id FROM ships WHERE id = $1 FOR UPDATE")
        .bind(ship_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| DbError::Other("ship".into()))?;

    let market = sqlx::query_as::<_, (i32, i32, i32, i64)>(
        r#"
        SELECT stock, buy_price, sell_price, version
        FROM station_markets
        WHERE station_id = $1 AND commodity_id = $2
        FOR UPDATE
        "#,
    )
    .bind(station_id)
    .bind(commodity_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| DbError::Other("invalid_item".into()))?;

    let (stock, buy_price, sell_price, version) = market;

    let cargo_qty: i32 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity),0)::int FROM cargo_stacks WHERE ship_id = $1 AND commodity_id = $2",
    )
    .bind(ship_id)
    .bind(commodity_id)
    .fetch_one(&mut *tx)
    .await?;

    // cargo mass
    let mass_used: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(quantity), 0)::float8 FROM cargo_stacks WHERE ship_id = $1
        "#,
    )
    .bind(ship_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(0.0);
    // Note: true mass needs mass_per_unit per commodity; use provided for this item delta.

    let today = chrono::Utc::now().date_naive();
    let mut vol = if trade_date == today { trade_vol } else { 0 };

    let (new_credits, delta_credits, new_stock, new_cargo_qty) = if side_buy {
        if stock < quantity {
            return Err(DbError::Other("insufficient_stock".into()));
        }
        let cost = buy_price as i64 * quantity as i64;
        if credits < cost {
            return Err(DbError::Other("insufficient_funds".into()));
        }
        vol += cost;
        if vol > daily_cap {
            return Err(DbError::Other("daily_cap".into()));
        }
        let add_mass = mass_per_unit * quantity as f64;
        // approximate: treat existing mass_used as unit count * 1; use capacity check on this trade only properly
        if mass_used * mass_per_unit + add_mass > cargo_capacity_mass
            && mass_used + quantity as f64 > cargo_capacity_mass
        {
            // capacity: sum(qty * mass) — fetch all cargo for accuracy
        }
        let stacks = sqlx::query_as::<_, (String, i32)>(
            "SELECT commodity_id, quantity FROM cargo_stacks WHERE ship_id = $1",
        )
        .bind(ship_id)
        .fetch_all(&mut *tx)
        .await?;
        // Without all mass_per_unit, use 1.0 for others and provided for this commodity
        let mut used = 0.0;
        for (cid, q) in &stacks {
            let m = if cid == commodity_id {
                mass_per_unit
            } else {
                1.0
            };
            used += *q as f64 * m;
        }
        used += quantity as f64 * mass_per_unit;
        if used > cargo_capacity_mass {
            return Err(DbError::Other("insufficient_cargo".into()));
        }
        (
            credits - cost,
            -cost,
            stock - quantity,
            cargo_qty + quantity,
        )
    } else {
        if cargo_qty < quantity {
            return Err(DbError::Other("insufficient_cargo".into()));
        }
        let gain = sell_price as i64 * quantity as i64;
        vol += gain;
        if vol > daily_cap {
            return Err(DbError::Other("daily_cap".into()));
        }
        (
            credits + gain,
            gain,
            stock + quantity,
            cargo_qty - quantity,
        )
    };

    sqlx::query(
        "UPDATE characters SET credits = $1, trade_volume_day = $2, trade_volume_day_date = $3 WHERE id = $4",
    )
    .bind(new_credits)
    .bind(vol)
    .bind(today)
    .bind(character_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE station_markets SET stock = $1, version = $2 WHERE station_id = $3 AND commodity_id = $4",
    )
    .bind(new_stock)
    .bind(version + 1)
    .bind(station_id)
    .bind(commodity_id)
    .execute(&mut *tx)
    .await?;

    if new_cargo_qty <= 0 {
        sqlx::query("DELETE FROM cargo_stacks WHERE ship_id = $1 AND commodity_id = $2")
            .bind(ship_id)
            .bind(commodity_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO cargo_stacks (ship_id, commodity_id, quantity)
            VALUES ($1, $2, $3)
            ON CONFLICT (ship_id, commodity_id) DO UPDATE SET quantity = EXCLUDED.quantity
            "#,
        )
        .bind(ship_id)
        .bind(commodity_id)
        .bind(new_cargo_qty)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO economy_ledger (character_id, kind, delta_credits, payload)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(character_id)
    .bind(if side_buy { "trade_buy" } else { "trade_sell" })
    .bind(delta_credits)
    .bind(serde_json::json!({
        "station_id": station_id,
        "commodity_id": commodity_id,
        "quantity": quantity,
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let cargo = list_cargo(pool, ship_id).await?;
    let mut mass = 0.0;
    for c in &cargo {
        let m = if c.commodity_id == commodity_id {
            mass_per_unit
        } else {
            1.0
        };
        mass += c.quantity as f64 * m;
    }
    Ok((new_credits, cargo, mass))
}

pub async fn refuel(
    pool: &PgPool,
    character_id: Uuid,
    ship_id: Uuid,
    price_per_unit: u32,
    units: u32,
    new_fuel: u32,
) -> Result<(i64, u32), DbError> {
    let cost = price_per_unit as i64 * units as i64;
    let mut tx = pool.begin().await?;
    let credits: i64 =
        sqlx::query_scalar("SELECT credits FROM characters WHERE id = $1 FOR UPDATE")
            .bind(character_id)
            .fetch_one(&mut *tx)
            .await?;
    if credits < cost {
        return Err(DbError::Other("insufficient_funds".into()));
    }
    let new_credits = credits - cost;
    sqlx::query("UPDATE characters SET credits = $1 WHERE id = $2")
        .bind(new_credits)
        .bind(character_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE ships SET fuel = $1, updated_at = now() WHERE id = $2")
        .bind(new_fuel as i32)
        .bind(ship_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO economy_ledger (character_id, kind, delta_credits, payload)
        VALUES ($1, 'refuel', $2, $3)
        "#,
    )
    .bind(character_id)
    .bind(-cost)
    .bind(serde_json::json!({ "units": units, "fuel": new_fuel }))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((new_credits, new_fuel))
}

pub async fn apply_death_cargo_loss(pool: &PgPool, ship_id: Uuid) -> Result<(), DbError> {
    // Lose 50% of each stack (floor).
    let stacks = list_cargo(pool, ship_id).await?;
    let mut tx = pool.begin().await?;
    for s in stacks {
        let keep = s.quantity / 2;
        if keep <= 0 {
            sqlx::query("DELETE FROM cargo_stacks WHERE ship_id = $1 AND commodity_id = $2")
                .bind(ship_id)
                .bind(&s.commodity_id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(
                "UPDATE cargo_stacks SET quantity = $1 WHERE ship_id = $2 AND commodity_id = $3",
            )
            .bind(keep)
            .bind(ship_id)
            .bind(&s.commodity_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn purchase_ship(
    pool: &PgPool,
    character_id: Uuid,
    def_id: &str,
    price: i64,
    system_id: &str,
    station_id: &str,
    starter_fuel: i32,
    shield: f32,
    armor: f32,
    energy: f32,
) -> Result<Uuid, DbError> {
    let mut tx = pool.begin().await?;
    let credits: i64 =
        sqlx::query_scalar("SELECT credits FROM characters WHERE id = $1 FOR UPDATE")
            .bind(character_id)
            .fetch_one(&mut *tx)
            .await?;
    if credits < price {
        return Err(DbError::Other("insufficient_funds".into()));
    }
    let ship_id = Uuid::now_v7();
    sqlx::query("UPDATE characters SET credits = $1 WHERE id = $2")
        .bind(credits - price)
        .bind(character_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO ships (
            id, character_id, def_id, name, system_id,
            pos_x, pos_y, rot, shield, armor, energy, fuel, loadout,
            docked_station, last_docked_station
        ) VALUES (
            $1, $2, $3, $3, $4,
            0, 0, 0, $5, $6, $7, $8, '{}'::jsonb,
            $9, $9
        )
        "#,
    )
    .bind(ship_id)
    .bind(character_id)
    .bind(def_id)
    .bind(system_id)
    .bind(shield)
    .bind(armor)
    .bind(energy)
    .bind(starter_fuel)
    .bind(station_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE characters SET active_ship_id = $1 WHERE id = $2")
        .bind(ship_id)
        .bind(character_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO economy_ledger (character_id, kind, delta_credits, payload)
        VALUES ($1, 'ship_purchase', $2, $3)
        "#,
    )
    .bind(character_id)
    .bind(-price)
    .bind(serde_json::json!({ "def_id": def_id, "ship_id": ship_id }))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ship_id)
}

pub async fn persist_ship_state(
    pool: &PgPool,
    ship_id: Uuid,
    system_id: &str,
    x: f64,
    y: f64,
    rot: f32,
    shield: f32,
    armor: f32,
    energy: f32,
    fuel: u32,
    docked: Option<&str>,
    last_docked: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query(
        r#"
        UPDATE ships SET
            system_id = $2, pos_x = $3, pos_y = $4, rot = $5,
            shield = $6, armor = $7, energy = $8, fuel = $9,
            docked_station = $10, last_docked_station = COALESCE($11, last_docked_station),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(ship_id)
    .bind(system_id)
    .bind(x)
    .bind(y)
    .bind(rot)
    .bind(shield)
    .bind(armor)
    .bind(energy)
    .bind(fuel as i32)
    .bind(docked)
    .bind(last_docked)
    .execute(pool)
    .await?;
    Ok(())
}

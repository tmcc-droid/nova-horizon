-- Seed static markets for sys.sol (two stations). Spreads are intentional content.
-- buy_price = what station charges player to buy; sell_price = what station pays player.

INSERT INTO station_markets (station_id, commodity_id, stock, buy_price, sell_price, version)
VALUES
    -- st.earth_orbit: food plentiful, ore scarcer
    ('st.earth_orbit', 'commodity.food', 500, 40, 30, 0),
    ('st.earth_orbit', 'commodity.ore',  120, 90, 70, 0),
    -- st.mars_depot: ore plentiful, food scarcer (arbitrage loop)
    ('st.mars_depot',  'commodity.food', 150, 55, 42, 0),
    ('st.mars_depot',  'commodity.ore',  400, 70, 55, 0)
ON CONFLICT (station_id, commodity_id) DO NOTHING;

-- Alpha Centauri station market seed
INSERT INTO station_markets (station_id, commodity_id, stock, buy_price, sell_price, version)
VALUES
    ('st.proxima_port', 'commodity.food', 80, 70, 50, 0),
    ('st.proxima_port', 'commodity.ore', 300, 65, 50, 0)
ON CONFLICT (station_id, commodity_id) DO NOTHING;
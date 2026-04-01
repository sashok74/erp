--! upsert_balance
--@ repo: balances
--@ kind: exec
--@ dec: balance
INSERT INTO test.balances (tenant_id, item_id, balance)
VALUES (:tenant_id, :item_id, :balance::TEXT::NUMERIC)
ON CONFLICT (tenant_id, item_id) DO UPDATE SET balance = EXCLUDED.balance;

--! get_balance
--@ repo: balances
--@ kind: opt
--@ dto: BalanceRow
--@ dec: balance
SELECT item_id, balance::TEXT
FROM test.balances
WHERE tenant_id = :tenant_id AND item_id = :item_id;

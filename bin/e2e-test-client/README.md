# Fuel Core E2E Test Client

The test client provides a suite of idempotent tests intended to validate remote environments.

To customize the suite parameters for an existing node deployment, set the `FUEL_CORE_E2E_CONFIG` environment variable with a path to a valid configuration toml.

When `FUEL_CORE_E2E_CONFIG` is unset a default configuration is used which is suitable for local environment testing. The default configuration is:

```toml
endpoint = "http://localhost:4000"
wallet_sync_timeout = "10s"
full_test = false

[wallet_a]
secret = "de97d8624a438121b86a1956544bd72ed68cd69f2c99555b08b1e8c51ffd511c"

[wallet_b]
secret = "37fa81c84ccd547c30c176b118d5cb892bdb113e8e80141f266519422ef9eefd"
```

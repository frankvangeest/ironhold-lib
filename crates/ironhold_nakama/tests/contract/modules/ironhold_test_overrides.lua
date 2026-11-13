-- Test-only overrides for the contract test, loaded by ironhold.lua with pcall(require, ...). Never copied to a real server.
-- A short lobby expiry lets the expiry cases take seconds instead of a minute (ironhold.lua clamps it to 1..60 and logs a warning).
return { lobby_expiry_secs = 6 }

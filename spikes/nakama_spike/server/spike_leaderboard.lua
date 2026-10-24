-- THROWAWAY spike module (planning/features/nakama_backend_spike.md, E3). Nakama only lets server code create a leaderboard;
-- clients can then submit scores to it and list the top records through the normal API. Lua needs no build step.
-- Deploy: copy this file into the modules directory bind-mounted into the Nakama container, then restart the container.
local nk = require("nakama")

-- id, authoritative, sort order, operator, reset schedule (nil = never), metadata
local ok, err = pcall(nk.leaderboard_create, "spike_best", false, "desc", "best", nil, {})
if ok then
  nk.logger_info("spike_leaderboard: leaderboard spike_best is ready")
else
  nk.logger_error(string.format("spike_leaderboard: create failed: %s", tostring(err)))
end

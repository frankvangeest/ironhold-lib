-- Test-only RPCs for the contract test of `game_rpc` in ironhold.lua. Never copied to a real server.
local nk = require("nakama")
local ih = require("ironhold")

-- Through the guard: reaches the function only with a game session, and gets the session's game id.
ih.game_rpc("ironhold_lobby_probe", function(_, _, game_id)
  return nk.json_encode({ game_id = game_id })
end)

-- The guard must refuse to register ids outside the allowlist; the result is reported by the probe below.
local refused = {}
for _, id in ipairs({ "ironhold_other_probe", "Ironhold_Lobby_Probe", "ironhold_lobby_", "ironhold_lobby_Probe", "healthcheck", 7 }) do
  refused[#refused + 1] = (pcall(ih.game_rpc, id, function() return "x" end)) == false
end
nk.register_rpc(function(_, _)
  return nk.json_encode({ refused = refused })
end, "test_registration_report")

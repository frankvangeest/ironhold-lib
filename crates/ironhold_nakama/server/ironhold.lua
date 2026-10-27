-- Ironhold generic Nakama module (planning/features/backend_abstraction.md, v2). No build step; the games it serves are listed
-- in the generated ironhold_games.lua (`ironhold backend config <project>...`), which must sit next to this file.
--
-- It does three things:
--   1. creates every declared leaderboard (clients cannot create leaderboards), and
--   2. enforces the game namespace on the server: a session belongs to exactly one game (the `game_id` session variable,
--      taken from the device id `{game_id}:{uuid}` at sign-in), and every leaderboard id and storage collection a request
--      names must start with `{game_id}.`. Client-side prefixing only stops honest bugs; anyone holding the shared server
--      key could otherwise read or write another game's data.
--
--   3. closes every API the engine does not use (matches, chat, groups, friends, tournaments, account linking), because those
--      share ids across games. v3 (lobbies, matchmaker) opens its own APIs again, each with a guard.
--
-- Pinned and tested against Nakama 3.41.0.
local nk = require("nakama")
local games = require("ironhold_games").games

local PERMISSION_DENIED = 7

local function reject(message)
  error({ message, PERMISSION_DENIED })
end

-- The leaderboard ids the config declares: a board dropped from backend.ron stays on the server but is no longer reachable.
local declared = {}
for _, game in pairs(games) do
  for _, lb in ipairs(game.leaderboards) do
    declared[lb.id] = true
  end
end

-- 1. leaderboards ------------------------------------------------------------------------------------------------------
for game_id, game in pairs(games) do
  for _, lb in ipairs(game.leaderboards) do
    -- id, authoritative, sort order, operator, reset schedule (nil = never), metadata
    local ok, err = pcall(nk.leaderboard_create, lb.id, false, lb.sort, lb.operator, nil, {})
    if ok then
      nk.logger_info(string.format("ironhold: leaderboard %s is ready", lb.id))
    else
      nk.logger_error(string.format("ironhold: leaderboard %s (game %s) could not be created: %s", lb.id, game_id, tostring(err)))
    end
  end
end

-- 2a. a session belongs to one game ------------------------------------------------------------------------------------
local function before_authenticate_device(_, payload)
  local id = payload.account and payload.account.id or ""
  local game_id = string.match(id, "^([a-z0-9_]+):")
  if not game_id or not games[game_id] then
    reject("unknown game")
  end
  -- Overwrites anything the client sent: the game comes from the device id and nowhere else.
  payload.account.vars = { game_id = game_id }
  return payload
end
nk.register_req_before(before_authenticate_device, "AuthenticateDevice")

-- Every other way of getting a session would carry no game, so none is allowed.
local function deny(_, _)
  reject("this sign-in method is not available")
end
for _, rpc in ipairs({
  "AuthenticateEmail", "AuthenticateCustom", "AuthenticateApple", "AuthenticateFacebook",
  "AuthenticateFacebookInstantGame", "AuthenticateGameCenter", "AuthenticateGoogle", "AuthenticateSteam",
}) do
  nk.register_req_before(deny, rpc)
end

-- A refresh must not let the client choose new session variables.
local function before_session_refresh(_, payload)
  payload.vars = nil
  return payload
end
nk.register_req_before(before_session_refresh, "SessionRefresh")

-- 2b. every id a request names must belong to the session's game --------------------------------------------------------
local function owns(context, id)
  local game_id = context.vars and context.vars.game_id
  if not game_id or not games[game_id] or type(id) ~= "string" then
    return false
  end
  return string.sub(id, 1, #game_id + 1) == game_id .. "."
end

local function leaderboard_guard(context, payload)
  if not owns(context, payload.leaderboard_id) then
    reject("that leaderboard belongs to another game")
  end
  if not declared[payload.leaderboard_id] then
    reject("that leaderboard is not declared")
  end
  -- The board's `keep` rule (best/latest/total) is the server's call: a client may not pick its own operator per write.
  if payload.record then
    payload.record.operator = 0
  end
  return payload
end
for _, rpc in ipairs({
  "WriteLeaderboardRecord", "ListLeaderboardRecords", "ListLeaderboardRecordsAroundOwner", "DeleteLeaderboardRecord",
}) do
  nk.register_req_before(leaderboard_guard, rpc)
end

local function collections_guard(list_field)
  return function(context, payload)
    if list_field then
      for _, o in ipairs(payload[list_field] or {}) do
        if not owns(context, o.collection) then
          reject("that collection belongs to another game")
        end
      end
    elseif not owns(context, payload.collection) then
      reject("that collection belongs to another game")
    end
    return payload
  end
end
nk.register_req_before(collections_guard("objects"), "WriteStorageObjects")
nk.register_req_before(collections_guard("object_ids"), "ReadStorageObjects")
nk.register_req_before(collections_guard("object_ids"), "DeleteStorageObjects")
nk.register_req_before(collections_guard(nil), "ListStorageObjects")

-- 3. everything the engine does not use is closed ------------------------------------------------------------------------
-- Nothing below is needed by sign-in and leaderboards, and each of these takes ids or names that are shared across games
-- (matches, chat rooms, groups, friends, tournaments) or would give one account two games (LinkDevice). Open an API again
-- only together with its own guard. Registered with pcall so an API name this Nakama version does not know cannot stop the
-- module from loading.
local function closed(_, _)
  reject("this API is not available")
end
for _, rpc in ipairs({
  -- REST
  "LinkDevice", "UnlinkDevice", "LinkEmail", "LinkCustom", "LinkApple", "LinkFacebook", "LinkFacebookInstantGame",
  "LinkGameCenter", "LinkGoogle", "LinkSteam",
  "ListMatches", "ListTournaments", "JoinTournament", "ListTournamentRecords", "WriteTournamentRecord",
  "ListTournamentRecordsAroundOwner", "CreateGroup", "ListGroups", "JoinGroup", "AddGroupUsers", "ListUserGroups",
  "ListGroupUsers", "AddFriends", "ListFriends", "DeleteFriends", "BlockFriends", "ImportFacebookFriends",
  "ImportSteamFriends", "ListNotifications", "DeleteNotifications", "ListChannelMessages", "GetUsers", "Event",
  "RpcFunc",
}) do
  local ok, err = pcall(nk.register_req_before, closed, rpc)
  if not ok then
    nk.logger_warn(string.format("ironhold: could not close %s: %s", rpc, tostring(err)))
  end
end
for _, rpc in ipairs({
  -- realtime socket
  "ChannelJoin", "ChannelLeave", "ChannelMessageSend", "ChannelMessageUpdate", "ChannelMessageRemove", "MatchCreate",
  "MatchDataSend", "MatchJoin", "MatchLeave", "MatchmakerAdd", "MatchmakerRemove", "PartyCreate", "PartyJoin",
  "PartyLeave", "PartyPromote", "PartyAccept", "PartyRemove", "PartyClose", "PartyJoinRequestList", "PartyMatchmakerAdd",
  "PartyMatchmakerRemove", "PartyDataSend", "StatusFollow", "StatusUnfollow", "StatusUpdate", "Rpc",
}) do
  local ok, err = pcall(nk.register_rt_before, closed, rpc)
  if not ok then
    nk.logger_warn(string.format("ironhold: could not close realtime %s: %s", rpc, tostring(err)))
  end
end

-- Ironhold generic Nakama module (planning/features/backend_abstraction.md, v2). No build step; the games it serves are listed
-- in the generated ironhold_games.lua (`ironhold backend config <project>...`), which must sit next to this file.
--
-- It does three things:
--   1. creates every declared leaderboard (clients cannot create leaderboards), and
--   2. enforces the game namespace on the server: a session belongs to exactly one game (the `game_id` session variable,
--      taken from the device id `{game_id}:{hash}` at sign-in, where hash is 32 lowercase hex characters the client derives from the
--      game and its stored secret, see `ironhold_nakama::ns`), and every leaderboard id and storage collection a request
--      names must start with `{game_id}.`. Client-side prefixing only stops honest bugs; anyone holding the shared server
--      key could otherwise read or write another game's data.
--
--   3. closes every API the engine does not use (matches, chat, groups, friends, tournaments, account linking, account updates),
--      because those share ids across games or let a player choose their display name. v3 (lobbies, matchmaker) opens its own
--      APIs again, each with a guard: v3a registers the six `ironhold_lobby_*` RPCs, only through `game_rpc` (section 2c), and keeps
--      their state in three server-owned collections per game (section 2d); v3b will reopen the matchmaker.
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
  local game_id, rest = string.match(id, "^([a-z0-9_]+):(.*)$")
  if not game_id or not games[game_id] then
    reject("unknown game")
  end
  -- The game is checked first, so an unknown game still says so. A suffix that is not 32 lowercase hex characters is the
  -- old `{game_id}:{uuid}` format (a stale page or an old native build): refuse it, so it cannot write a raw uuid into the
  -- database again, and say what to do.
  if #rest ~= 32 or string.find(rest, "[^0-9a-f]") then
    reject("out of date: reload the page or update the game")
  end
  -- Overwrites anything the client sent: the game comes from the device id and nowhere else.
  payload.account.vars = { game_id = game_id }
  -- The display name is made by the server, never chosen by the player (it shows in lobby lists and on the leaderboards): drop a
  -- `?username=` so Nakama generates one when the account is created.
  payload.username = nil
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

-- The three lobby collections are the server's own (section 2d): a client never reads, writes, deletes or lists them, so a player
-- cannot read other players' ids or forge a lobby. Only the RPCs touch them, and those run as the server.
local RESERVED_COLLECTIONS = { lobbies = true, lobby_hb = true, lobby_of = true }

local function check_collection(context, collection)
  if not owns(context, collection) then
    reject("that collection belongs to another game")
  end
  if RESERVED_COLLECTIONS[string.sub(collection, #context.vars.game_id + 2)] then
    reject("that collection is reserved for the server")
  end
end

local function collections_guard(list_field)
  return function(context, payload)
    if list_field then
      for _, o in ipairs(payload[list_field] or {}) do
        check_collection(context, o.collection)
      end
    else
      check_collection(context, payload.collection)
    end
    return payload
  end
end
nk.register_req_before(collections_guard("objects"), "WriteStorageObjects")
nk.register_req_before(collections_guard("object_ids"), "ReadStorageObjects")
nk.register_req_before(collections_guard("object_ids"), "DeleteStorageObjects")
nk.register_req_before(collections_guard(nil), "ListStorageObjects")

-- 2c. RPCs: a guarded registration, only for the lobby family ----------------------------------------------------------
-- Nakama has no before-hook for REST RPC calls (verified against 3.41.0: a hook on "RpcFunc" is accepted and never runs), so
-- the door cannot be closed from outside. Instead every RPC of this module is registered through `game_rpc`, which only accepts
-- ids of the form `ironhold_lobby_<name>` (lowercase letters and underscores) and wraps the function so that it never runs
-- without a game session (a call made with the HTTP key alone has none) and hands it the session's game id. The function must
-- namespace everything it touches with that game id, like the storage guard above.
-- What this does NOT cover: an RPC that some other module on the same server registers with `nk.register_rpc` answers any
-- signed-in session of any game. The operator decides which modules are installed; `ironhold.lua` is meant to be the only one.
local function game_rpc(id, fn)
  if type(id) ~= "string" or not string.match(id, "^ironhold_lobby_[a-z_]+$") then
    error("ironhold: RPC id " .. tostring(id) .. " is outside the ironhold_lobby_* allowlist")
  end
  nk.register_rpc(function(context, payload)
    local game_id = context.vars and context.vars.game_id
    if not game_id or not games[game_id] then
      reject("this API needs a signed-in game session")
    end
    local ok, result = pcall(fn, context, payload, game_id)
    if ok then
      return result
    end
    -- A deliberate refusal (`reject`, an error table) goes to the client as it is. Anything else is a bug or a storage failure:
    -- Nakama would send its text, with the module's file path, to the caller. Log it here, answer with a fixed message.
    if type(result) == "table" then
      error(result)
    end
    nk.logger_error(string.format("ironhold: RPC %s (game %s) failed: %s", id, game_id, tostring(result)))
    reject("the request failed")
  end, id)
end
-- Realtime `Rpc` stays closed below (v3a is REST only). If v3b reopens it, no second guard is needed: this wrapper guards the
-- function itself, and a socket session carries the same `vars`.

-- 2d. lobbies ----------------------------------------------------------------------------------------------------------
-- planning/features/backend_lobbies_server.md. Six RPCs behind `game_rpc`; all state is server-owned storage (system user, no client
-- permission) and every write is versioned (a conflict means "re-read and retry", never "overwrite").
--
--   {game}.lobbies   lobby_id -> {v, name, max, host, members = {{uid, name}, ...}, started, closed = nil|"host_left", created, ended}
--   {game}.lobby_hb  user_id  -> {lobby, at}        the member's last check-in (server clock, seconds)
--   {game}.lobby_of  user_id  -> {lobby = id | ""}  which lobby the user is in; "" is a tombstone (a storage_write cannot delete)
--
-- Rules the code below keeps: a reader treats a `lobby_of` that points at a lobby the user is not a member of, or that is closed,
-- started or missing, as "in no lobby"; the sweep never writes another user's `lobby_of`; a `lobby_hb` row is deleted only if its
-- `lobby` is the lobby being swept, left or deleted; the cleanup delete of the caller's own `lobby_of` never retries.
--
-- Constants shared with the engine (schema/backend.rs; a Rust unit test compares the numbers) and server-only ones.
local HEARTBEAT_SECS = 10 -- LOBBY_HEARTBEAT_SECS: core drives the heartbeat, the server only needs EXPIRY_SECS; kept so both are checked
local EXPIRY_SECS = 60 -- LOBBY_EXPIRY_SECS
local MIN_PLAYERS = 2 -- MIN_LOBBY_PLAYERS
local MAX_PLAYERS = 8 -- MAX_LOBBY_PLAYERS
local LIST_LIMIT = 10 -- MAX_LOBBY_ROWS
local MAX_NAME = 32 -- MAX_LOBBY_NAME_LEN
local MAX_LOBBIES_PER_GAME = 50 -- server only
local PAYLOAD_MAX_BYTES = 512 -- server only

local SYSTEM_USER = "00000000-0000-0000-0000-000000000000"
local PAGE_SIZE, MAX_PAGES, READ_CHUNK = 100, 3, 50
local CLEANUP_PER_CALL = 5
local RETRIES = 3

-- Test-only: tests/contract/modules/ironhold_test_overrides.lua may shorten the expiry so the expiry cases take seconds. Never
-- written by the generator and never present on a real server; clamped to 1..EXPIRY_SECS and announced loudly.
local expiry_secs = EXPIRY_SECS
do
  local ok, overrides = pcall(require, "ironhold_test_overrides")
  if ok and type(overrides) == "table" and type(overrides.lobby_expiry_secs) == "number" then
    expiry_secs = math.max(1, math.min(EXPIRY_SECS, math.floor(overrides.lobby_expiry_secs)))
    nk.logger_warn(string.format("ironhold: TEST OVERRIDE in effect: lobby expiry is %d s instead of %d s", expiry_secs, EXPIRY_SECS))
  end
end

-- The lobby names each game declared (a closed list: a player cannot put free text on other players' screens).
local lobby_name_set = {}
for game_id, game in pairs(games) do
  local set = {}
  for _, name in ipairs(game.lobby_names or {}) do
    set[name] = true
  end
  lobby_name_set[game_id] = set
end

-- Collection names come only from here. `lobbies`, `lobby_hb` and `lobby_of` are reserved (a future cloud-save slot validator must
-- refuse them as slot names).
local function lobby_collection(game_id, kind)
  assert(kind == "lobbies" or kind == "lobby_hb" or kind == "lobby_of", "unknown lobby collection kind")
  return game_id .. "." .. kind
end

local function lobbies_enabled(game_id)
  local game = games[game_id]
  return game ~= nil and type(game.features) == "table" and game.features.lobbies == true
end

-- nk.time() is milliseconds: store and compare whole seconds everywhere.
local function now_secs()
  return math.floor(nk.time() / 1000)
end

-- A table from a JSON object payload, or nil for anything malformed, oversized or not an object. An empty payload is `{}`.
local function decode_payload(payload, max_bytes)
  if payload == nil or payload == "" then
    return {}
  end
  if type(payload) ~= "string" or #payload > max_bytes then
    return nil
  end
  local ok, decoded = pcall(nk.json_decode, payload)
  if not ok or type(decoded) ~= "table" or decoded[1] ~= nil then
    return nil
  end
  return decoded
end

local function valid_lobby_id(id)
  return type(id) == "string" and #id == 32 and not string.find(id, "[^0-9a-f]")
end

local function reply(t)
  return nk.json_encode(t)
end

local function fail(reason)
  return nk.json_encode({ ok = false, reason = reason })
end

-- storage ------------------------------------------------------------------------------------------------------------------
-- key -> {value, version} for the keys that exist.
local function sys_read(collection, keys)
  local found = {}
  if #keys == 0 then
    return found
  end
  local request = {}
  for i, key in ipairs(keys) do
    request[i] = { collection = collection, key = key, user_id = SYSTEM_USER }
  end
  for _, object in ipairs(nk.storage_read(request)) do
    found[object.key] = { value = object.value, version = object.version }
  end
  return found
end

local function read_one(collection, key)
  return sys_read(collection, { key })[key]
end

-- A versioned write entry. `version` is what the writer read; "*" (also used when the row was absent) means "must not exist".
local function wobj(collection, key, value, version)
  return {
    collection = collection, key = key, user_id = SYSTEM_USER, value = value, version = version or "*",
    permission_read = 0, permission_write = 0,
  }
end

-- All entries or none. Returns ok, acks. A conflict is routine (someone else wrote first): the caller re-reads and retries.
local function try_write(objects)
  local ok, acks = pcall(nk.storage_write, objects)
  return ok, ok and acks or nil
end

local function try_delete(collection, key, version)
  return (pcall(nk.storage_delete, { { collection = collection, key = key, user_id = SYSTEM_USER, version = version } }))
end

local function ack_version(acks, collection, key)
  for _, ack in ipairs(acks or {}) do
    if ack.collection == collection and ack.key == key then
      return ack.version
    end
  end
  return nil
end

-- lobby helpers -------------------------------------------------------------------------------------------------------------
local function member_index(lobby, uid)
  for i, member in ipairs(lobby.members) do
    if member.uid == uid then
      return i
    end
  end
  return nil
end

local function copy_lobby(lobby)
  local members = {}
  for i, member in ipairs(lobby.members) do
    members[i] = { uid = member.uid, name = member.name }
  end
  return {
    v = lobby.v, name = lobby.name, max = lobby.max, host = lobby.host, members = members, started = lobby.started,
    closed = lobby.closed, created = lobby.created, ended = lobby.ended,
  }
end

-- The lobby after `uid` leaves it: the host leaving closes it, anyone else is removed from the list.
local function left_by(lobby, uid, now)
  local after = copy_lobby(lobby)
  after.v = lobby.v + 1
  if lobby.host == uid then
    after.closed = "host_left"
    after.ended = now
  else
    local members = {}
    for _, member in ipairs(after.members) do
      if member.uid ~= uid then
        members[#members + 1] = member
      end
    end
    after.members = members
  end
  return after
end

local function hb_live(hb, lobby_id, now)
  return hb ~= nil and hb.lobby == lobby_id and type(hb.at) == "number" and (now - hb.at) <= expiry_secs
end

local function host_name(lobby)
  for _, member in ipairs(lobby.members) do
    if member.uid == lobby.host then
      return member.name
    end
  end
  return ""
end

local function lobby_reply(id, lobby)
  local names = {}
  for i, member in ipairs(lobby.members) do
    names[i] = member.name
  end
  return reply({
    ok = true,
    lobby = {
      id = id, version = lobby.v, name = lobby.name, max = lobby.max, started = lobby.started == true, host = host_name(lobby),
      players = #lobby.members, members = names,
    },
  })
end

-- The caller's index row and, when it names an open lobby the caller is a member of, that lobby. Anything else is "in no lobby".
local function open_membership(game_id, uid)
  local of = read_one(lobby_collection(game_id, "lobby_of"), uid)
  local id = of and of.value.lobby or ""
  if id == "" then
    return of, nil
  end
  local row = read_one(lobby_collection(game_id, "lobbies"), id)
  if row and member_index(row.value, uid) and not row.value.closed and not row.value.started then
    return of, { id = id, lobby = row.value, version = row.version }
  end
  return of, nil
end

-- One write that moves the caller into `new_id`: the new lobby, the old lobby without the caller, the caller's index row and
-- check-in. A lost race leaves the caller in the old lobby, never in none.
local function replace_objects(game_id, uid, new_id, new_lobby, new_version, old, of, hb, now)
  local lobbies = lobby_collection(game_id, "lobbies")
  local objects = { wobj(lobbies, new_id, new_lobby, new_version) }
  if old then
    objects[#objects + 1] = wobj(lobbies, old.id, left_by(old.lobby, uid, now), old.version)
  end
  objects[#objects + 1] = wobj(lobby_collection(game_id, "lobby_of"), uid, { lobby = new_id }, of and of.version)
  objects[#objects + 1] = wobj(lobby_collection(game_id, "lobby_hb"), uid, { lobby = new_id, at = now }, hb and hb.version)
  return objects
end

-- Deletes a lobby and the check-in rows of its members that still point at it. Returns whether the lobby object was deleted.
local function delete_lobby(game_id, candidate)
  if not try_delete(lobby_collection(game_id, "lobbies"), candidate.id, candidate.version) then
    return false
  end
  local hb_collection = lobby_collection(game_id, "lobby_hb")
  local uids = {}
  for _, member in ipairs(candidate.lobby.members) do
    uids[#uids + 1] = member.uid
  end
  for uid, hb in pairs(sys_read(hb_collection, uids)) do
    if hb.value.lobby == candidate.id then
      try_delete(hb_collection, uid, hb.version)
    end
  end
  return true
end

-- Up to MAX_PAGES pages of the game's lobby objects, and whether more were left.
local function scan_lobbies(game_id)
  local all, cursor, truncated = {}, nil, false
  for page = 1, MAX_PAGES do
    local objects, next_cursor = nk.storage_list(SYSTEM_USER, lobby_collection(game_id, "lobbies"), PAGE_SIZE, cursor)
    for _, object in ipairs(objects or {}) do
      all[#all + 1] = { id = object.key, lobby = object.value, version = object.version }
    end
    cursor = next_cursor
    if cursor == nil or cursor == "" then
      break
    end
    if page == MAX_PAGES then
      truncated = true
    end
  end
  return all, truncated
end

-- The open lobbies whose host is live, plus whether the scan hit its page bound. Lazily cleans up to CLEANUP_PER_CALL dead lobbies
-- per call: a lobby whose host is gone is closed (members learn `host_left` on their next heartbeat), a closed or started lobby is
-- deleted once its marker has expired.
local function live_lobbies(game_id, now)
  local all, truncated = scan_lobbies(game_id)
  local host_keys = {}
  for _, candidate in ipairs(all) do
    if not candidate.lobby.closed and not candidate.lobby.started then
      host_keys[#host_keys + 1] = candidate.lobby.host
    end
  end
  local hb_collection = lobby_collection(game_id, "lobby_hb")
  local host_hb = {}
  for first = 1, #host_keys, READ_CHUNK do
    local chunk = {}
    for i = first, math.min(first + READ_CHUNK - 1, #host_keys) do
      chunk[#chunk + 1] = host_keys[i]
    end
    for key, hb in pairs(sys_read(hb_collection, chunk)) do
      host_hb[key] = hb
    end
  end
  local live, budget = {}, CLEANUP_PER_CALL
  for _, candidate in ipairs(all) do
    local lobby = candidate.lobby
    if lobby.closed or lobby.started then
      if budget > 0 and type(lobby.ended) == "number" and (now - lobby.ended) > expiry_secs then
        if delete_lobby(game_id, candidate) then
          budget = budget - 1
        end
      end
    else
      local hb = host_hb[lobby.host]
      if hb_live(hb and hb.value, candidate.id, now) then
        live[#live + 1] = candidate
      elseif budget > 0 then
        local closed = copy_lobby(lobby)
        closed.v = lobby.v + 1
        closed.closed = "host_left"
        closed.ended = now
        if try_write({ wobj(lobby_collection(game_id, "lobbies"), candidate.id, closed, candidate.version) }) then
          budget = budget - 1
        end
      end
    end
  end
  return live, truncated
end

-- Removes members whose check-in is missing or old (a dead host closes the lobby). The caller just checked in, so it is live. A lost
-- write is swallowed: the next heartbeat sweeps again. Returns the lobby as it is afterwards.
local function sweep(game_id, id, row, caller, now)
  local lobby = row.value
  if lobby.started then
    return lobby
  end
  local uids = {}
  for _, member in ipairs(lobby.members) do
    if member.uid ~= caller then
      uids[#uids + 1] = member.uid
    end
  end
  local hb_collection = lobby_collection(game_id, "lobby_hb")
  local hbs = sys_read(hb_collection, uids)
  local dead, host_dead, any_dead = {}, false, false
  for _, member in ipairs(lobby.members) do
    if member.uid ~= caller then
      local hb = hbs[member.uid]
      if not hb_live(hb and hb.value, id, now) then
        dead[member.uid] = true
        any_dead = true
        if member.uid == lobby.host then
          host_dead = true
        end
      end
    end
  end
  if not any_dead then
    return lobby
  end
  local after = copy_lobby(lobby)
  after.v = lobby.v + 1
  if host_dead then
    after.closed = "host_left"
    after.ended = now
  else
    local members = {}
    for _, member in ipairs(after.members) do
      if not dead[member.uid] then
        members[#members + 1] = member
      end
    end
    after.members = members
  end
  if try_write({ wobj(lobby_collection(game_id, "lobbies"), id, after, row.version) }) then
    for uid in pairs(dead) do
      local hb = hbs[uid]
      if hb and hb.value.lobby == id then
        try_delete(hb_collection, uid, hb.version)
      end
    end
    return after
  end
  local again = read_one(lobby_collection(game_id, "lobbies"), id)
  return again and again.value or lobby
end

-- the six RPCs -----------------------------------------------------------------------------------------------------------------
local function rpc_list(context, payload, game_id)
  if not lobbies_enabled(game_id) then
    return fail("not_enabled")
  end
  if not decode_payload(payload, PAYLOAD_MAX_BYTES) then
    return fail("server")
  end
  local live = live_lobbies(game_id, now_secs())
  table.sort(live, function(a, b)
    if a.lobby.created ~= b.lobby.created then
      return a.lobby.created > b.lobby.created
    end
    return a.id < b.id
  end)
  local rows = {}
  for i = 1, math.min(#live, LIST_LIMIT) do
    local lobby = live[i].lobby
    rows[i] = { id = live[i].id, name = lobby.name, players = #lobby.members, max = lobby.max, host = host_name(lobby) }
  end
  return reply({ ok = true, rows = rows })
end

local function rpc_create(context, payload, game_id)
  if not lobbies_enabled(game_id) then
    return fail("not_enabled")
  end
  local request = decode_payload(payload, PAYLOAD_MAX_BYTES)
  if not request then
    return fail("server")
  end
  local name, max = request.name, request.max_players
  if type(name) ~= "string" or #name > MAX_NAME or not lobby_name_set[game_id][name] then
    nk.logger_warn(string.format("ironhold: game %s refused the lobby name %q (not in its declared lobby_names)", game_id, tostring(name)))
    return fail("server")
  end
  if type(max) ~= "number" or max ~= math.floor(max) or max < MIN_PLAYERS or max > MAX_PLAYERS then
    return fail("server")
  end
  local uid, now = context.user_id, now_secs()
  local live, truncated = live_lobbies(game_id, now)
  if #live >= MAX_LOBBIES_PER_GAME or truncated then
    return fail("limit_reached")
  end
  for _ = 1, RETRIES do
    local of, old = open_membership(game_id, uid)
    local hb = read_one(lobby_collection(game_id, "lobby_hb"), uid)
    local id = (string.gsub(nk.uuid_v4(), "-", ""))
    local lobby = {
      v = 1, name = name, max = max, host = uid, members = { { uid = uid, name = context.username } }, started = false, created = now,
    }
    if try_write(replace_objects(game_id, uid, id, lobby, "*", old, of, hb, now)) then
      return lobby_reply(id, lobby)
    end
  end
  return fail("server")
end

local function rpc_join(context, payload, game_id)
  if not lobbies_enabled(game_id) then
    return fail("not_enabled")
  end
  local request = decode_payload(payload, PAYLOAD_MAX_BYTES)
  local id = request and request.lobby_id
  if not valid_lobby_id(id) then
    return fail("server")
  end
  local uid, now = context.user_id, now_secs()
  local lobbies, hb_collection = lobby_collection(game_id, "lobbies"), lobby_collection(game_id, "lobby_hb")
  for _ = 1, RETRIES do
    local row = read_one(lobbies, id)
    if not row or row.value.closed then
      return fail("gone")
    end
    local lobby = row.value
    local hb = read_one(hb_collection, uid)
    if member_index(lobby, uid) then
      -- Already a member (a reload, then a join of the same lobby): the lobby is unchanged, only the check-in is refreshed.
      if try_write({ wobj(hb_collection, uid, { lobby = id, at = now }, hb and hb.version) }) then
        return lobby_reply(id, lobby)
      end
    else
      if lobby.started then
        return fail("started")
      end
      if #lobby.members >= lobby.max then
        return fail("full")
      end
      local of, old = open_membership(game_id, uid)
      local after = copy_lobby(lobby)
      after.v = lobby.v + 1
      after.members[#after.members + 1] = { uid = uid, name = context.username }
      if try_write(replace_objects(game_id, uid, id, after, row.version, old, of, hb, now)) then
        return lobby_reply(id, after)
      end
    end
  end
  return fail("server")
end

local function rpc_leave(context, payload, game_id)
  if not lobbies_enabled(game_id) then
    return fail("not_enabled")
  end
  local request = decode_payload(payload, PAYLOAD_MAX_BYTES)
  if not request or (request.lobby_id ~= nil and not valid_lobby_id(request.lobby_id)) then
    return fail("server")
  end
  local uid, now = context.user_id, now_secs()
  local lobbies, of_collection = lobby_collection(game_id, "lobbies"), lobby_collection(game_id, "lobby_of")
  local hb_collection = lobby_collection(game_id, "lobby_hb")
  for _ = 1, RETRIES do
    local of = read_one(of_collection, uid)
    local id = of and of.value.lobby or ""
    -- "Leave whatever I am in", except that a leave naming another lobby than the current one (a late leave from before a
    -- re-join) must not undo the newer membership.
    if id == "" or (request.lobby_id ~= nil and request.lobby_id ~= id) then
      return reply({ ok = true })
    end
    local row = read_one(lobbies, id)
    local objects
    if row and member_index(row.value, uid) and not row.value.closed and not row.value.started then
      objects = { wobj(lobbies, id, left_by(row.value, uid, now), row.version), wobj(of_collection, uid, { lobby = "" }, of.version) }
    else
      -- A started, closed or missing lobby is left alone: only the caller's own index row is cleared.
      objects = { wobj(of_collection, uid, { lobby = "" }, of.version) }
    end
    local ok, acks = try_write(objects)
    if ok then
      -- Cleanup, best effort and never retried: the caller's check-in row (only while it still points at this lobby) and the
      -- tombstone (only at the version just written: a newer row means a concurrent join, keep it).
      local hb = read_one(hb_collection, uid)
      if hb and hb.value.lobby == id then
        try_delete(hb_collection, uid, hb.version)
      end
      local tombstone = ack_version(acks, of_collection, uid)
      if tombstone then
        try_delete(of_collection, uid, tombstone)
      end
      return reply({ ok = true })
    end
  end
  return fail("server")
end

local function rpc_heartbeat(context, payload, game_id)
  if not lobbies_enabled(game_id) then
    return fail("not_enabled")
  end
  local request = decode_payload(payload, PAYLOAD_MAX_BYTES)
  local id = request and request.lobby_id
  if not valid_lobby_id(id) then
    return fail("server")
  end
  local uid, now = context.user_id, now_secs()
  local hb_collection = lobby_collection(game_id, "lobby_hb")
  for _ = 1, RETRIES do
    -- Checks first, check-in second: a heartbeat that raced a leave must not bring the leaver back (its versioned write
    -- conflicts once the leave deleted the row, and the retry then finds the membership gone).
    local of = read_one(lobby_collection(game_id, "lobby_of"), uid)
    if not of or of.value.lobby ~= id then
      return fail("removed")
    end
    local row = read_one(lobby_collection(game_id, "lobbies"), id)
    if not row or not member_index(row.value, uid) then
      return fail("removed")
    end
    if row.value.closed then
      -- The members are told the host left; a host who was swept (lost contact for a minute) and comes back is told it was removed.
      return fail(row.value.host == uid and "removed" or "host_left")
    end
    local hb = read_one(hb_collection, uid)
    if try_write({ wobj(hb_collection, uid, { lobby = id, at = now }, hb and hb.version) }) then
      local lobby = sweep(game_id, id, row, uid, now)
      if lobby.closed then
        return fail("host_left")
      end
      return lobby_reply(id, lobby)
    end
  end
  return fail("server")
end

local function rpc_start(context, payload, game_id)
  if not lobbies_enabled(game_id) then
    return fail("not_enabled")
  end
  local request = decode_payload(payload, PAYLOAD_MAX_BYTES)
  local id = request and request.lobby_id
  if not valid_lobby_id(id) then
    return fail("server")
  end
  local uid, now = context.user_id, now_secs()
  local lobbies = lobby_collection(game_id, "lobbies")
  for _ = 1, RETRIES do
    local of = read_one(lobby_collection(game_id, "lobby_of"), uid)
    local row = read_one(lobbies, id)
    if not of or of.value.lobby ~= id or not row or row.value.closed or not member_index(row.value, uid) then
      return fail("not_in_lobby")
    end
    local lobby = row.value
    if lobby.host ~= uid then
      return fail("not_host")
    end
    if lobby.started then
      return fail("started")
    end
    if #lobby.members < MIN_PLAYERS then
      return fail("too_few_players")
    end
    local after = copy_lobby(lobby)
    after.v = lobby.v + 1
    after.started = true
    after.ended = now
    if try_write({ wobj(lobbies, id, after, row.version) }) then
      return lobby_reply(id, after)
    end
  end
  return fail("server")
end

for id, fn in pairs({
  ironhold_lobby_list = rpc_list, ironhold_lobby_create = rpc_create, ironhold_lobby_join = rpc_join,
  ironhold_lobby_leave = rpc_leave, ironhold_lobby_heartbeat = rpc_heartbeat, ironhold_lobby_start = rpc_start,
}) do
  game_rpc(id, fn)
end

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
  "ListGroupUsers", "AddFriends", "ListFriends", "DeleteFriends", "BlockFriends", "ImportFacebookFriends", "UpdateAccount",
  "ImportSteamFriends", "ListNotifications", "DeleteNotifications", "ListChannelMessages", "GetUsers", "Event",
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

-- Exposed so the contract test fixture can register through the same guard.
return { game_rpc = game_rpc }

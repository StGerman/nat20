local PRONE = "nat20_core::effect.condition.prone"
-- Kept apart from PRONE so a Prone attacker's own Disadvantage isn't overwritten
local PRONE_TARGET = "nat20_core::effect.condition.prone.target"

--- Advantage for an attacker within 5 feet, Disadvantage for one farther away
---@type AttackedHookFn
local function attacked_hook(engine_state, effect, victim, attacker, check)
    if not check.kind:attack_roll() then
        return
    end

    local distance = engine_state:distance_ft(victim, attacker)
    if distance == nil then
        return
    end

    if distance <= 5 then
        check:add_advantage("advantage", PRONE_TARGET)
    else
        check:add_advantage("disadvantage", PRONE_TARGET)
    end
end

--- Stand up at the start of the turn, spending half the Speed (rounded down)
--- of movement; a creature at 0 Hit Points or with a Speed of 0 stays down
---@type TurnStartHookFn
local function turn_start_hook(engine_state, entity)
    if engine_state:hp_current(entity) <= 0 then
        return
    end

    -- Speed is stored in meters, so round away the conversion error first
    local speed = math.floor(engine_state:speed_ft(entity) + 0.5)
    if speed <= 0 then
        return
    end

    engine_state:spend_movement(entity, math.floor(speed / 2))
    engine_state:remove_effect(entity, PRONE)
end

return {
    attacked_hook = attacked_hook,
    turn_start_hook = turn_start_hook,
}

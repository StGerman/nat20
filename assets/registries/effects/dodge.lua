local DODGE = "nat20_core::effect.dodge"
local INCAPACITATED = "nat20_core::effect.condition.incapacitated"

--- Dodging needs a creature that can act and move
local function dodging(engine_state, entity)
    if engine_state:has_effect(entity, INCAPACITATED) then
        return false
    end
    -- Speed is stored in meters, so round away the conversion error first
    return math.floor(engine_state:speed_ft(entity) + 0.5) > 0
end

--- Disadvantage on attack rolls against the dodging creature
---@type AttackedHookFn
local function attacked_hook(engine_state, effect, victim, attacker, check)
    if check.kind:attack_roll() and dodging(engine_state, victim) then
        check:add_advantage("disadvantage", DODGE)
    end
end

--- Advantage on Dexterity saving throws
---@type D20CheckHookFn
local function d20_check_hook(engine_state, entity, d20_check)
    if d20_check.kind:saving_throw() and dodging(engine_state, entity) then
        d20_check:add_advantage("advantage", DODGE)
    end
end

return {
    attacked_hook = attacked_hook,
    d20_check_hook = d20_check_hook,
}

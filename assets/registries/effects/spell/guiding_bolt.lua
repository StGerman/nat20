local GUIDING_BOLT = "nat20_core::effect.spell.guiding_bolt"

--- Whoever makes the next attack roll against the target has Advantage, the caster included
---@type AttackedHookFn
local function attacked_hook(engine_state, effect, victim, attacker, check)
    if check.kind:attack_roll() then
        check:add_advantage("advantage", GUIDING_BOLT)
    end
end

--- Spent by the first attack roll against the target that got the Advantage
---@type EventFilterFn
local function event_filter(event, applier, target)
    local actor, result, dc = event:as_d20_check_performed()
    if actor and result and dc and result.kind:attack_roll() then
        return dc.target == target and result:get_advantage(GUIDING_BOLT) ~= nil
    end

    return false
end

return {
    attacked_hook = attacked_hook,
    event_filter = event_filter
}

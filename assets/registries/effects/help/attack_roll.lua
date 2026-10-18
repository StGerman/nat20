local HELP_ATTACK_ROLL = "nat20_core::effect.help.attack_roll"

--- The Advantage is for the helper's allies, not the helper
---@type AttackedHookFn
local function attacked_hook(engine_state, effect, victim, attacker, check)
    if not check.kind:attack_roll() then
        return
    end

    local helper = effect.applier
    if not helper or attacker == helper then
        return
    end

    if engine_state:attitude(helper, attacker) == "friendly" then
        check:add_advantage("advantage", HELP_ATTACK_ROLL)
    end
end

--- Spent by the first attack roll against the target that got the Advantage
---@type EventFilterFn
local function event_filter(event, applier, target)
    local actor, result, dc = event:as_d20_check_performed()
    if actor and result and dc and result.kind:attack_roll() then
        return dc.target == target and result:get_advantage(HELP_ATTACK_ROLL) ~= nil
    end

    return false
end

return {
    attacked_hook = attacked_hook,
    event_filter = event_filter
}

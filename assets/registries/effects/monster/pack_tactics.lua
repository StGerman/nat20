local PACK_TACTICS = "nat20_core::effect.monster.pack_tactics"
local INCAPACITATED = "nat20_core::effect.condition.incapacitated"

--- Advantage when an ally that can act stands within 5 feet of the target
---@type D20CheckHookFn
local function d20_check_hook(engine_state, entity, check)
    local target = check.target
    if target == nil or not check.kind:attack_roll() then
        return
    end

    for _, creature in ipairs(engine_state:creatures_within(target, 5)) do
        if creature ~= entity
            and engine_state:attitude(entity, creature) == "friendly"
            and engine_state:hp_current(creature) > 0
            and not engine_state:has_effect(creature, INCAPACITATED)
        then
            check:add_advantage("advantage", PACK_TACTICS)
            return
        end
    end
end

return {
    d20_check_hook = d20_check_hook,
}

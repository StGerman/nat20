---@type HealingHookFn
local function healing_hook(engine_state, healer, target, action, healing)
    if action.action_id ~= "nat20_core::action.cleric.life_domain.preserve_life" then
        return
    end

    local pool = 5 * engine_state:class_level(healer, "nat20_core::class.cleric")
    local share = math.floor(pool / math.max(1, action:creature_target_count()))
    local room = math.max(0, math.floor(engine_state:hp_max(target) / 2) - engine_state:hp_current(target))
    healing:add_modifier("nat20_core::effect.cleric.life_domain.preserve_life", tostring(math.min(share, room)))
end

return {
    healing_hook = healing_hook,
}

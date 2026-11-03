---@type HealingHookFn
local function healing_hook(engine_state, healer, target, action, healing)
    local spell_level = action.action_context:spell_level()
    if spell_level and spell_level >= 1 then
        healing:add_modifier("nat20_core::effect.cleric.life_domain.disciple_of_life", tostring(2 + spell_level))
    end
end

return {
    healing_hook = healing_hook,
}

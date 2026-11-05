local SPELL_SLOTS = {
    "nat20_core::resource.spell_slot",
    "nat20_core::resource.warlock.pact_magic_spell_slot",
}

-- Only a spell cast with a spell slot: a granted spell cast for free gets no bonus
local function uses_a_spell_slot(action)
    for _, slot in ipairs(SPELL_SLOTS) do
        if action:costs_resource(slot) then
            return true
        end
    end
    return false
end

---@type HealingHookFn
local function healing_hook(engine_state, healer, target, action, healing)
    local spell_level = action.action_context:spell_level()
    if spell_level and spell_level >= 1 and uses_a_spell_slot(action) then
        healing:add_modifier("nat20_core::effect.cleric.life_domain.disciple_of_life", tostring(2 + spell_level))
    end
end

return {
    healing_hook = healing_hook,
}

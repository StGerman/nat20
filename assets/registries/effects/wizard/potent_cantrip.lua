--- A cantrip that misses or is saved against still deals half its damage
---@type DamageOnFailureHookFn
local function damage_on_failure_hook(engine_state, actor, action)
    return action.action_context:spell_level() == 0
end

return {
    damage_on_failure_hook = damage_on_failure_hook,
}

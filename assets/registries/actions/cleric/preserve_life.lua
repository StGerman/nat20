--- Only a Bloodied creature: one with half its Hit Points or fewer left
---@type TargetUsabilityFn
local function target_usability(engine_state, entity, target, action_id, context)
    if engine_state:hp_current(target) > engine_state:hp_max(target) / 2 then
        return "Preserve Life can only heal a Bloodied creature"
    end
end

return {
    target_usability = target_usability,
}

local KNOCK_PRONE = "nat20_core::effect.monster.knock_prone_on_bite"
local PRONE = "nat20_core::effect.condition.prone"

-- Sizes a Bite can knock down
local SMALL_ENOUGH = {
    tiny = true,
    small = true,
    medium = true,
}

--- A melee weapon hit on a Medium or smaller target knocks it Prone
---@type ActionResultHookFn
local function action_result_hook(engine_state, action, result)
    local resolution = result:resolution()
    if not resolution or not resolution:is_attack_roll() or not resolution:is_success() then
        return
    end

    local context = action.action_context
    if not context:is_weapon_attack() or not context:is_melee_attack() then
        return
    end

    local target = result.target
    if not SMALL_ENOUGH[engine_state:size(target)] or engine_state:has_effect(target, PRONE) then
        return
    end

    engine_state:apply_effect(action.actor, target, PRONE, KNOCK_PRONE, context)
end

return {
    action_result_hook = action_result_hook,
}
